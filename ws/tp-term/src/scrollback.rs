/// Scrollback support
///
/// Currently there is just one scrollback implementation, the `MemScrollback`

use std::mem;
use std::rc::Rc;
use std::cell::UnsafeCell;
use std::iter;
use std::collections::{VecDeque, vec_deque};

use ::{Line, Cell, Style, VTColor};


bitflags! {
    /// Based on VTRendition, except without stuff that we don't need for scrollback
    /// and additionally with flags for color storage.
    struct SBRendition: u8 {
        const NONE       = 0;
        const BOLD       = 1 << 0;
        const UNDERLINED = 1 << 1;
        const INVERSE    = 1 << 2;
        const BLINKING   = 1 << 3;
        const INVISIBLE  = 1 << 4;

        // MemScrollback bookkeeping:
        const HAS_FG     = 1 << 5;
        const HAS_BG     = 1 << 6;
        const LAST       = 1 << 7;
    }
}

impl Default for SBRendition {
    fn default() -> SBRendition { SBRendition::NONE }
}

impl From<Style> for SBRendition {
    fn from(style: Style) -> SBRendition {
        let mut res = SBRendition::from_bits_truncate(style.rendition.bits() & 0x1f);
        if style.col_fg != VTColor::DefaultFg { res |= SBRendition::HAS_FG; }
        if style.col_bg != VTColor::DefaultBg { res |= SBRendition::HAS_BG; }
        res
    }
}

impl SBRendition {
    fn is_last(&self) -> bool {
        self.contains(SBRendition::LAST)
    }

    fn has_fg(&self) -> bool { self.contains(SBRendition::HAS_FG) }
    fn has_bg(&self) -> bool { self.contains(SBRendition::HAS_BG) }

    fn header_size(&self) -> usize {
        2  // flags & size
        + if self.has_fg() { 4 } else { 0 }
        + if self.has_bg() { 4 } else { 0 }
    }
}

impl VTColor {
    fn memsb_encode(&self) -> [u8 ; 4] {
        unsafe { mem::transmute(*self) }
    }

    fn memsb_decode(data: &[u8]) -> VTColor {
        let mut color = [0u8 ; 4];
        color.copy_from_slice(&data[..4]);
        unsafe { mem::transmute(color) }
    }
}


const CHUNK_SIZE: usize = 32 * 1024;
const CHUNK_OVERHEAD: usize = 2 * mem::size_of::<usize>() /* = Rc overhead */ + mem::size_of::<MemSBLine>();

/// A line stored in the `MemScrollback`
#[derive(Debug)]
pub struct MemSBLine {
    chunk: Rc<UnsafeCell<Vec<u8>>>,
    offset: usize,
}

impl MemSBLine {
    fn new(line: &Line) -> MemSBLine {
        let mut res = MemSBLine {
            chunk: Rc::new(UnsafeCell::new(Vec::with_capacity(CHUNK_SIZE))),
            offset: 0,
        };
        res.encode_line(line);
        res
    }

    fn from_previous(line: &Line, prev: &MemSBLine) -> Option<MemSBLine> {
        let line_size = Self::line_size(line);
        if line_size <= prev.chunk().capacity() - prev.chunk().len() {
            let mut res = MemSBLine {
                chunk: Rc::clone(&prev.chunk),
                offset: prev.chunk().len(),
            };
            res.encode_line(line);
            Some(res)
        } else {
            None
        }
    }

    fn chunk(&self) -> &Vec<u8> { unsafe { &*self.chunk.get() } }
    fn chunk_mut(&mut self) -> &mut Vec<u8> { unsafe { &mut *self.chunk.get() } }

    fn encode_piece_header(&mut self, style: Style) {
        let rend: SBRendition = style.into();

        // Encode flags and size
        self.chunk_mut().push(rend.bits());
        self.chunk_mut().push(0);   // Placeholder, will be modified as needed

        // Encode colors if needed
        if rend.has_fg() {
            self.chunk_mut().extend(&style.col_fg.memsb_encode());
        }
        if rend.has_bg() {
            self.chunk_mut().extend(&style.col_bg.memsb_encode());
        }
    }

    fn trim_count(line: &Line) -> usize {
        let default_cell = Cell::default();
        line.iter().rev().take_while(|cell| **cell == default_cell).count()
    }

    fn encode_line(&mut self, line: &Line) {
        let num_cells = line.len() - Self::trim_count(line);

        let mut style = line.get(0).map_or(Style::default(), |cell| cell.style);
        let mut piece_size = 0u32;

        // Start writing the first piece
        let mut piece_start = self.chunk().len();
        self.encode_piece_header(style);

        for cell in &line[..num_cells] {
            if cell.style != style || piece_size == 255 {
                // Need to finalize the current piece and start a new one
                self.chunk_mut()[piece_start + 1] = piece_size as u8;    // Write the final size of the current piece
                style = cell.style;
                piece_size = 1;
                piece_start = self.chunk().len();
                self.encode_piece_header(style);
            } else {
                piece_size += 1;
            }

            self.chunk_mut().extend(cell.as_str().as_bytes());
        }

        // Finalize the last piece
        self.chunk_mut()[piece_start] |= SBRendition::LAST.bits();
        self.chunk_mut()[piece_start + 1] = piece_size as u8;
    }

    fn line_size(line: &Line) -> usize {
        let num_cells = line.len() - Self::trim_count(line);

        let style = match line.get(0) {
            Some(cell) => cell.style,
            None => return SBRendition::default().header_size(),
        };
        let rend: SBRendition = style.into();

        let mut piece_size = 0u32;

        line[..num_cells].iter().fold((rend.header_size(), style), |(mut size, style), cell| {
            if cell.style != style || piece_size == 255 {
                let rend: SBRendition = cell.style.into();
                size += rend.header_size();
                piece_size = 1;
            } else {
                piece_size += 1;
            }

            (size + cell.as_str().len(), cell.style)
        }).0
    }

    /// Obtain a piece iterator of this line. A piece is a sub-string of the line with contiguous `Style`.
    /// See `MemScrollback` documentation for more information.
    pub fn iter(&self) -> PieceIter {
        PieceIter::new(self)
    }
}


/// Holds a reference to a substring of a scrollback line in which all characters have contiguously the same `Style`
#[derive(Debug, PartialEq, Eq)]
pub struct Piece<'a> {
    /// The string data of this piece
    pub string: &'a str,
    /// The `Style` used to render this line substring
    pub style: Style,
}

/// Iterates `MemSBLine` substrings. See `MemScrollback` or `Piece` documentation for more information.
#[derive(Debug)]
pub struct PieceIter<'a> {
    chunk: &'a Vec<u8>,
    offset: usize,
    last_seen: bool,
}

impl<'a> PieceIter<'a> {
    fn new(line: &'a MemSBLine) -> PieceIter<'a> {
        PieceIter {
            chunk: line.chunk(),
            offset: line.offset,
            last_seen: false,
        }
    }
}

impl<'a> iter::Iterator for PieceIter<'a> {
    type Item = Piece<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.last_seen {
            return None;
        }

        let rend = SBRendition::from_bits_truncate(self.chunk[self.offset]);
        let size = self.chunk[self.offset + 1] as usize;
        if rend.is_last() || size == 0 {
            self.last_seen = true;
        }

        let mut style = Style::default();
        let mut data_offset = self.offset + 2;
        if rend.has_fg() {
            style.col_fg = VTColor::memsb_decode(&self.chunk[data_offset .. data_offset + 4]);
            data_offset += 4;
        }
        if rend.has_bg() {
            style.col_bg = VTColor::memsb_decode(&self.chunk[data_offset .. data_offset + 4]);
            data_offset += 4;
        }

        let str_slice = &self.chunk[data_offset .. data_offset + size];
        let string = unsafe { ::std::str::from_utf8_unchecked(str_slice) };

        // Advance state
        self.offset = data_offset + string.len();

        Some(Piece {
            string,
            style,
        })
    }
}

pub type LineIter<'a> = vec_deque::Iter<'a, MemSBLine>;


/// An efficient memory-backed scrollback implementation
///
/// The MemScrollback stores shell screen lines in a compressed way in a queue-like data structure.
/// Each line is right-trimmed removing default (empty) cells and then broken down into "pieces" - substrings in which
/// characters share the same rendering `Style`. Pieces are then stored in memory along with the Style data.
/// Therefore, `Style` data is only stored when it changes along the line.
/// Additionally, allocating memory by larger chunks is used instead of allocation smaller pieces for each line
/// (currently a chunk size of 32k is used).
///
/// Each piece is layed out in memory as follows:
///
/// `flags: u8 | length: u8 | [fg_color: u32] | [bg_color: u32] | UTF-8 string data ...`
///
/// The foreground and/or background color is only stored when it differs from the default.
#[derive(Debug)]
pub struct MemScrollback {
    lines: VecDeque<MemSBLine>,
    data_size: usize,
    mem_cap: usize,
}

impl MemScrollback {
    /// Constructs a new `MemScrollback` with that will consume a maximum of `mem_cap` bytes of memory
    pub fn new(mem_cap: usize) -> MemScrollback {
        MemScrollback {
            lines: VecDeque::new(),
            data_size: 0,
            mem_cap,
        }
    }

    fn mem_size(&self) -> usize {
        self.lines.capacity() * mem::size_of::<MemSBLine>() + self.data_size
    }

    fn pop_over_cap(&mut self) {
        let mut half_lines = self.lines.len() / 2;
        while self.mem_size() > self.mem_cap {
            let front = match self.lines.pop_front() {
                Some(front) => front,
                None => return,
            };

            while self.lines.front().map_or(false, |line| Rc::ptr_eq(&line.chunk, &front.chunk)) {
                self.lines.pop_front();
            }
            assert_eq!(Rc::strong_count(&front.chunk), 1, "MemScrollback: Internal error: Memory leak");

            let dec = front.chunk().capacity() + CHUNK_OVERHEAD;
            self.data_size -= dec;

            if self.lines.len() <= half_lines {
                self.lines.shrink_to_fit();
                half_lines = self.lines.len() / 2;
            }
        }
    }

    /// Push a line into the scrollback. This is typically only used by a screen data structure.
    pub fn push(&mut self, line: Line) {
        // Encode the line into either the previous chunk or a new one, get back MemSBLine
        let line = match self.lines.back()
            .and_then(|prev| MemSBLine::from_previous(&line, prev)) {
            Some(line) => line,
            None => {
                let line = MemSBLine::new(&line);
                let inc = line.chunk().capacity() + CHUNK_OVERHEAD;
                self.data_size += inc;
                line
            },
        };

        // Append the MemSBLine into our deque
        // self.pop_over_cap_index();
        self.lines.push_back(line);
        self.pop_over_cap();
    }

    /// Nuber of lines in the scollback.
    pub fn len(&self) -> usize { self.lines.len() }

    /// Set the memory cap (in bytes) of the scrollback in-memory data storage.
    /// Note that due to internal implementation details the actual comsumed size may be somewhat larger,
    /// although not by a very significant ammount.
    pub fn set_mem_cap(&mut self, mem_cap: usize) {
        self.mem_cap = mem_cap;
        self.pop_over_cap();
    }

    /// Obtain a line iterator, it iterates in the older-to-newer direction
    pub fn iter(&self) -> LineIter {
        self.lines.iter()
    }

    pub fn iter_at(&self, at: usize) -> LineIter {
        let mut iter = self.lines.iter();
        if at > 0 {
            iter.nth(at - 1);
        }
        iter
    }
}

impl Default for MemScrollback {
    fn default() -> MemScrollback {
        MemScrollback::new(20 * 1024 * 1024)
    }
}




#[cfg(test)]
mod tests {
use super::*;

const MEM_CAP: usize = 1 * 1024 * 1024;
static LONG_STR: [u8; 200] = [b'a'; 200];

fn test_line() -> (Line, Vec<Piece<'static>>) {
    let style_blue = Style::with_fg(VTColor::Indexed(::VTCOLOR_BLUE));
    let style_red = Style::with_fg(VTColor::Indexed(::VTCOLOR_RED));
    let mut line = Line::new();

    line.push(Cell::new('a', Style::default()));
    line.push(Cell::new('b', style_blue));
    line.push(Cell::new('c', style_red));

    let mut pieces = Vec::new();
    pieces.push(Piece { string: "a", style: Style::default() });
    pieces.push(Piece { string: "b", style: style_blue });
    pieces.push(Piece { string: "c", style: style_red });

    (line, pieces)
}

fn wide_line() -> (Line, Vec<Piece<'static>>) {
    let (mut test_line, test_pieces) = test_line();
    let mut line = Line::with_size(Cell::new('a', Style::default()), 150);
    let mut line2 = line.clone();

    line.append(&mut test_line);
    line.append(&mut line2);

    let string = unsafe { ::std::str::from_utf8_unchecked(&LONG_STR[..]) };
    let mut pieces = Vec::new();
    pieces.push(Piece { string, style: Style::default() });

    (line, pieces)
}

#[test]
fn memscrollback_basic() {
    let (mut line, pieces) = test_line();

    let mut scrollback = MemScrollback::new(MEM_CAP);
    for i in 0..5 { line.push(Cell::default()); }   // Ensure trimming works
    scrollback.push(line.clone());

    let line_iter = scrollback.iter().next().unwrap();
    assert_eq!(line_iter.iter().count(), pieces.iter().count());
    for (p1, p2) in line_iter.iter().zip(pieces.iter()) {
        assert_eq!(p1, *p2);
    }
}

#[test]
fn memscrollback_line_size() {
    let lines = vec![
        test_line().0,
        wide_line().0,
    ];

    for line in lines {
        let size = MemSBLine::line_size(&line);
        let sbline = MemSBLine::new(&line);
        let sbline2 = MemSBLine::from_previous(&line, &sbline).unwrap();
        assert_eq!(size, sbline2.offset);
    }
}

#[test]
fn memscrollback_mem_cap() {
    let tests = vec![
        (2*CHUNK_SIZE, CHUNK_SIZE, test_line().0),
        (256 * 1024, 2000, wide_line().0),
    ];

    for (cap, num_lines, line) in tests.iter() {
        let line_size = MemSBLine::line_size(&line);

        let mut scrollback = MemScrollback::new(*cap);
        for _ in 0..*num_lines {
            scrollback.push(line.clone());
            let mem_size = scrollback.mem_size();
            assert!(mem_size <= *cap, "mem_size: {}, cap: {}", mem_size, *cap);
        }
    }
}

#[test]
fn memscrollback_iter_at() {
    let (mut line, pieces) = test_line();

    let mut scrollback = MemScrollback::new(MEM_CAP);
    let num_lines = 10;
    for i in 0..num_lines {
        scrollback.push(line.clone());
    }

    let at = 3;
    let line_iter = scrollback.iter_at(3);
    assert_eq!(line_iter.count(), num_lines - at);
}


}
