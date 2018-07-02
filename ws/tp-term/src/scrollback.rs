/// Scrollback implmenetation: TODO
/// None if this works yet

use std::mem;
use std::ops;
use std::collections::VecDeque;

use ::{Line, Cell, Style, VTColor, VTRendition};


pub trait VTScrollback {
    fn push(&mut self, line: Line);
}


// bitflags! {
//     pub struct PieceFlags: u8 {
//         const NONE = 0;
//         const COL_FG    = 1 << 0;
//         const COL_BG    = 1 << 0;
//         const RENDITION = 1 << 0;
//         const LAST      = 1 << 0;
//     }
// }

// impl Default for PieceFlags {
//     fn default() -> PieceFlags { PieceFlags::NONE }
// }

// impl PieceFlags {
//     fn with_style(style: Style) -> PieceFlags {
//         let mut res = PieceFlags::NONE;
//         if style.col_fg != VTColor::DefaultFg { res |= PieceFlags::COL_FG; }
//         if style.col_bg != VTColor::DefaultBg { res |= PieceFlags::COL_BG; }
//         if style.rendition != VTRendition::default() { res |= PieceFlags::RENDITION; }

//         res
//     }

//     fn is_last(&self) -> bool {
//         self.contains(PieceFlags::LAST)
//     }
// }

// #[derive(Debug, Default)]
// #[repr(C)]
// struct PieceHeader {
//     len: u16,
//     prev: u8,
//     flags: PieceFlags,
// }

// const HEADER_SIZE: usize = mem::size_of::<PieceHeader>();
// const MAX_PIECE_SIZE: usize = 1024 - HEADER_SIZE;

// impl PieceHeader {
//     fn new(len: u16, prev: u8, flags: PieceFlags) -> PieceHeader {
//         PieceHeader { len, prev, flags }
//     }

//     fn prev(&self) -> usize {
//         self.prev * 4 + 2
//     }

//     fn set_prev(&mut self, mut prev: usize) {
//         prev -= 2;
//         assert_eq!(prev & 3, 0, "MemScrollback: Piece header not aligned to 4 bytes");
//         prev /= 4;
//         self.prev = prev as u8;
//     }
// }

// const PAGE_SIZE: usize = 4096;

// #[derive(Debug)]
// struct Page(Vec<u8>);

// impl Page {
//     fn new() -> Page {
//         let mut page = Vec::with_capacity(PAGE_SIZE);
//         // Initialize line number to 0 (2 bytes) and add an empty PieceHeader
//         page.resize(2 + HEADER_SIZE, 0u8);

//         Page(page)
//     }

//     fn num_lines(&self) -> &u16 { unsafe { mem::transmute(self.0.as_ptr()) } }
//     fn num_lines_mut(&mut self) -> &mut u16 { unsafe { mem::transmute(self.0.as_mut_ptr()) } }

//     fn tail_header_mut(&mut self) -> (&mut PieceHeader, usize) {
//         let pos = self.0.len() - HEADER_SIZE;
//         let res = unsafe { mem::transmute(self.0.as_mut_ptr().offset(pos as isize)) };
//         (res, pos)
//     }

//     fn push(&mut self, line: Line) -> Result<(), Line> {
//         if line.is_empty() {
//             let (header, header_pos) = self.tail_header_mut();
//             // header.
//         }

//         // TODO

//         unimplemented!()
//     }
// }

bitflags! {
    /// Based on VTRendition, except without stuff that we either don't implement (BLINKING, INVISIBLE) or don't need for scrollback (WIDE)
    /// and additionally with flags for color storage.
    pub struct SBRendition: u8 {
        const NONE = 0;
        const BOLD       = 1 << 0;
        const UNDERLINED = 1 << 1;
        const INVERSE    = 1 << 3;

        // MemScrollback bookkeeping:
        const HAS_FG     = 1 << 4;
        const HAS_BG     = 1 << 5;
        const LAST       = 1 << 6;
    }
}

impl Default for SBRendition {
    fn default() -> SBRendition { SBRendition::NONE }
}

impl From<Style> for SBRendition {
    fn from(style: Style) -> SBRendition {
        let mut res = SBRendition::from_bits_truncate(style.rendition.bits() & 7);
        if style.col_fg != VTColor::DefaultFg { res |= SBRendition::HAS_FG; }
        if style.col_bg != VTColor::DefaultBg { res |= SBRendition::HAS_BG; }
        res
    }
}

impl SBRendition {
    fn is_last(&self) -> bool {
        self.contains(SBRendition::LAST)
    }

    fn header_size(&self) -> usize {
        1
        + if self.contains(SBRendition::HAS_BG) { 4 } else { 0 }
        + if self.contains(SBRendition::HAS_FG) { 4 } else { 0 }
    }
}

impl VTColor {
    fn memsb_encode(&self) -> [u8 ; 4] {
        unsafe { mem::transmute(*self) }
    }
}

impl Line {
    fn memsb_size(&self) -> usize {
        let first_rend: SBRendition = match self.get(0) {
            Some(cell) => cell.style,
            None => return 1,  // An empty line requires 1 LAST marker
        }.into();

        self.iter().skip(1).fold((first_rend.header_size(), first_rend), |(mut size, prev_rend), cell| {
            let rend: SBRendition = cell.style.into();
            if rend != prev_rend { size += rend.header_size(); }
            (size, rend)
        }).0 + 1   // + 1 for the LAST marker
    }
}

const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
struct Page(Vec<u8>);

impl Page {
    fn new() -> Page {
        let page = Vec::with_capacity(PAGE_SIZE);
        Page(page)
    }

    fn free_space(&self) -> usize {
        self.capacity() - self.len()
    }
}

impl ops::Deref for Page {
    type Target = Vec<u8>;
    fn deref(&self) -> &Vec<u8> { &self.0 }
}

impl ops::DerefMut for Page {
    fn deref_mut(&mut self) -> &mut Vec<u8> { &mut self.0 }
}

/// MemScrollback Line bookkeeping data
#[derive(Debug)]
struct LineMeta(usize, usize);

#[derive(Debug)]
pub struct MemScrollback {
    directory: VecDeque<LineMeta>,
    pages: VecDeque<Page>,
    pages_popped: usize,
    mem_cap: usize,
}

impl MemScrollback {
    pub fn new(mem_cap: usize) -> MemScrollback {
        let mut pages = VecDeque::new();
        pages.push_back(Page::new());

        MemScrollback {
            directory: VecDeque::new(),
            pages,
            pages_popped: 0,
            mem_cap,
        }
    }

    pub fn lines(&self) -> usize {
        self.directory.len()
    }
}

impl VTScrollback for MemScrollback {
    fn push(&mut self, mut line: Line) {
        // First, trim default cells from the right
        while line.last().map(|cell| *cell == Cell::default()).unwrap_or(false) {
            line.pop();
        }

        let encode_size = line.memsb_size();
        let mut page_num = (self.pages.len() - 1).wrapping_add(self.pages_popped);
        if self.pages.back().unwrap().free_space() < encode_size {
            self.pages.push_back(Page::new());
        }
        let page = self.pages.back_mut().unwrap();

        unimplemented!()
    }
}


#[derive(Debug, Default)]
struct NullScrollback;

impl VTScrollback for NullScrollback {
    fn push(&mut self, _line: Line) {}
}
