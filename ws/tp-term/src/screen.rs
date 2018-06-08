use std::{mem, ops};
use std::str::Chars;
use std::collections::VecDeque;
use unicode_width::UnicodeWidthChar;

use ::smallstring::*;
use ::vt::*;


#[derive(Debug, Default, Clone, Copy)]
pub struct Style {
    col_fg: VTColor,
    col_bg: VTColor,
    rendition: VTRendition,
}

#[derive(Debug, Clone)]
/// Character as part of the screen's grid, has associated `Style`
///
/// May actually consist of more than one unicode characters if combining marks are present.
pub struct Cell {
    chars: SmallString<[u8 ; 4]>,
    style: Style,
}

impl Cell {
    pub fn new(ch: char, style: Style) -> Cell {
        let mut res = Cell {
            chars: SmallString::new(),
            style
        };
        res.push(ch);
        res
    }

    pub fn with_style(style: Style) -> Cell {   // You gotta do stuff with_style!
        let mut ch = Cell::default();
        ch.style = style;
        ch
        // Yeah, I know it was a bad joke...
    }

    // pub fn combine(&mut self, what: char) {
    //     // First try to compose the char into the first char that we already have
    //     // Ie. for example 'a' + '´' becomes 'á'. This isn't possible in all cases.
    //     if let Some(combined) = unicode::compose(self.chars[0], what) {
    //         self.chars[0] = combined;
    //     } else {
    //         // `what` could not be combined, append it into the list if there's a free spot.
    //         // If there's no free spot, it gets discarded.
    //         for c in &mut self.chars[1..] {
    //             if *c == '\0' {
    //                 *c = what;
    //                 break;
    //             }
    //         }
    //     }
    // }

    pub fn dirty(&self) -> bool {
        self.style.rendition.contains(VTRendition::Dirty)
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.style.rendition.set(VTRendition::Dirty, dirty);
    }

    pub fn push(&mut self, ch: char) {
        self.chars.push(ch);
        self.set_dirty(true);
    }

    // TODO: &str
    pub fn as_str(&self) -> &str {
        self.chars.as_ref()
    }

    pub fn col_fg(&self) -> VTColor { self.style.col_fg }
    pub fn col_bg(&self) -> VTColor { self.style.col_bg }
    pub fn rendition(&self) -> VTRendition { self.style.rendition }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            chars: SmallString::from_str(" "),
            style: Style::default(),
        }
    }
}


pub const GRAPHICS: [char ; 32] = [
    '\u{0020}', '\u{25c6}', '\u{2592}', '\u{2409}', '\u{240c}', '\u{240d}', '\u{240a}', '\u{00b0}',   // _ through f
    '\u{00b1}', '\u{2424}', '\u{240b}', '\u{2518}', '\u{2510}', '\u{250c}', '\u{2514}', '\u{253c}',   // g through n
    '\u{23ba}', '\u{23bb}', '\u{2014}', '\u{23bd}', '\u{23af}', '\u{251c}', '\u{2524}', '\u{2534}',   // o through v
    '\u{252c}', '\u{2502}', '\u{2264}', '\u{2265}', '\u{03c0}', '\u{2260}', '\u{00a3}', '\u{00b7}',   // w through ~
];

#[derive(Debug, Clone)]
pub struct Line {
    chars: Vec<Cell>,
}

impl Line {
    pub fn new(ch: Cell, width: u32) -> Line {
        Line {
            chars: vec![ch ; width as usize],
        }
    }

    fn fill(&mut self, start: usize, end: usize, value: Cell) {
        self.chars.iter_mut()
            .skip(start)
            .take(end.saturating_sub(start))
            .for_each(|c| *c = value.clone());
    }
}

impl ops::Deref for Line {
    type Target = Vec<Cell>;
    fn deref(&self) -> &Self::Target { &self.chars }
}

impl ops::DerefMut for Line {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.chars }
}


#[derive(Debug, Default, Clone)]
pub struct Cursor {
    /// Horizontal position on the screen
    x: u32,
    /// Vertical position on the screen
    y: u32,
    /// Current format of this cursor
    style: Style,
    /// Currently used charset
    charset: VTCharset,
    /// Charset slots (four by specification)
    charsets: [VTCharset ; 4],
    /// Whether VTMode::Origin is active, only used for cursor save & restore
    mode_origin: bool,
}

impl Cursor {
    // pub fn clamp(&mut self, to_size: (u32, u32)) {
    //     if self.x > to_size.0 { self.x = to_size.0; }
    //     if self.y > to_size.1 { self.y = to_size.1; }
    // }

    pub fn charset_use(&mut self, slot: u32) {
        if let Some(c) = self.charsets.get(slot as usize) {
            self.charset = *c;
        }
    }

    fn charset_designate(&mut self, slot: u32, charset: VTCharset) {
        if let Some(c) = self.charsets.get_mut(slot as usize) {
            *c = charset;
        }
    }
}

pub const SCREEN_SIZE_MIN: (u32, u32) = (10, 5);
pub const SCREEN_SIZE_DEFAULT: (u32, u32) = (80, 40);

#[derive(Debug)]
pub struct Screen {
    /// Size: width, height, in number of characters
    size: (u32, u32),
    /// Current cursor data
    cursor: Cursor,
    /// Saved cursor data (for the VT cursor save & restore functionality)
    cursor_saved: Cursor,
    /// Mode
    mode: VTMode,
    /// Scrolling region span: top, bottom; spans the whole screen by default
    /// Warning: `scroll_rg` is 0-indexed, while `set_scroll_region()` has 1-indexed arguments (with `0` being "the default").
    scroll_rg: (u32, u32),
    /// Tab stops
    tabs: Vec<bool>,
    /// The actual character data
    lines: VecDeque<Line>,
}

impl Screen {
    pub fn new() -> Screen {
        // By default we initialize the screen to minimum size
        Screen::with_size(SCREEN_SIZE_MIN)
    }

    pub fn with_size(size: (u32, u32)) -> Screen {
        let mut lines = VecDeque::with_capacity(size.1 as usize);
        for _ in 0 .. size.1 {
            lines.push_back(Line::new(Cell::default(), size.0));
        }

        let tabs = (0 .. size.0).map(|i| i > 0 && i % 8 == 0).collect();

        Screen {
            size,
            cursor: Cursor::default(),
            cursor_saved: Cursor::default(),
            mode: VTMode::default(),
            scroll_rg: (0, size.1 - 1),
            tabs,
            lines,
        }
    }

    /// Iterate Lines
    pub fn line_iter(&mut self) -> impl ExactSizeIterator + Iterator<Item=&mut Line> {
        self.lines.iter_mut()
    }

    fn empty_char(&self) -> Cell {
        Cell::with_style(self.cursor.style)    // XXX: replace occurences
    }

    fn empty_line(&self) -> Line {
        Line::new(self.empty_char(), self.size.0)
    }

    fn x(&self) -> usize { self.cursor.x as usize }
    fn y(&self) -> usize { self.cursor.y as usize }

    fn clamp_x(&self, x: u32) -> u32 { if x >= self.size.0 { self.size.0 - 1 } else { x } }
    fn clamp_y(&self, y: u32) -> u32 { if y >= self.size.1 { self.size.1 - 1 } else { y } }

    fn sr_set(&self) -> bool { self.scroll_rg != (0, self.size.1 - 1) }

    fn cursor_in_sr(&self) -> bool {
        self.cursor.y >= self.scroll_rg.0 && self.cursor.y <= self.scroll_rg.1
    }

    fn cursor_set_pos(&mut self, x: u32, y: u32) {
        self.cursor.x = self.clamp_x(x);
        self.cursor.y = if self.mode.contains(VTMode::Origin) {
            y.min(self.scroll_rg.1).max(self.scroll_rg.0)
        } else {
            self.clamp_y(y)
        };
    }

    fn current_char(&mut self) -> &mut Cell {
        let x = self.x();
        self.lines.get_mut(self.cursor.y as usize).and_then(|line| line.get_mut(x)).expect("Cursor position out of bounds")
    }

    fn current_line(&mut self) -> &mut Line {
        self.lines.get_mut(self.cursor.y as usize).expect("Cursor position out of bounds")
    }

    /// Scroll lines in the range (top, bottom), inserting blank lines and popping to scrollback if appropriate
    fn scroll_generic(&mut self, range: (u32, u32), num: i32) {
        // FIXME: dirty marking

        let range = (range.0 as i32, range.1 as i32);

        if num > 0 {
            // Scroll up

            for i in range.0 .. range.0 - num + 1 {
                self.lines.swap(i as usize, (i + num) as usize);
            }

            for i in range.0 - num + 1 .. range.0 + 1 {
                let mut empty = self.empty_line();
                mem::swap(&mut empty, self.lines.get_mut(i as usize).expect("Lines index out of bounds"));
                if range.0 == 0 {
                    // TODO: Scrollback
                }
            }

        } else {
            // Scroll down

            for i in (range.0 + num .. range.0).rev() {
                self.lines.swap(i as usize, (i - num) as usize);
            }

            for i in range.0 .. range.0 + num {
                let mut empty = self.empty_line();
                *self.lines.get_mut(i as usize).expect("Lines index out of bounds") = empty;
            }
        }
    }
}

impl Default for Screen {
    fn default() -> Screen {
        Screen::with_size(SCREEN_SIZE_DEFAULT)
    }
}

impl VTScreen for Screen {
    fn put_char(&mut self, ch: char) {
        println!("put_char: {} @ ({}, {})", ch, self.x(), self.y());

        let ch = match (self.cursor.charset, ch as usize) {
            (VTCharset::Graphics, ord @ 0x5f ... 0x7e) => GRAPHICS[ord],
            _ => ch,
        };

        let width = ch.width().expect("Unexpected control character");

        if width == 0 {
            self.current_char().push(ch);
        } else {
            let x_last_valid = match width {
                1 => self.size.0 - 1,
                2 => self.size.0 - 2,
                _ => panic!("Unexpected character width: {}", width),
            };


            if self.cursor.x > x_last_valid {
                if self.mode.contains(VTMode::Wrap) && self.cursor_in_sr() {
                    self.newline();
                    self.cursor.x = 0;
                } else {
                    self.cursor.x = x_last_valid;
                }
            }

            let (x, y) = (self.x(), self.y());
            let line = &mut self.lines[y];

            if x > 0 {
                let prev = &mut line[x];
                if prev.style.rendition.contains(VTRendition::Wide) {
                    *prev = Cell::new(' ', prev.style);
                    prev.style.rendition.remove(VTRendition::Wide);
                    prev.set_dirty(true);
                }
            }

            if self.mode.contains(VTMode::Insert) {
                for c in line[x + 1 ..].iter_mut() {
                    *c = Cell::with_style(self.cursor.style);
                }
            }

            let mut ch = Cell::new(ch, self.cursor.style);
            if width == 2 {
                ch.style.rendition.insert(VTRendition::Wide);
                line[x + 1] = Cell::with_style(self.cursor.style);
            }

            line[x] = ch;
            self.cursor.x += width as u32;
        }
    }

    fn put_chars(&mut self, num: u32) {
        let x = self.x();
        let empty_char = self.empty_char();
        let line = self.current_line();
        for c in line[x..].iter_mut() {
            *c = empty_char.clone();
        }
    }

    fn newline(&mut self) {
        self.index(true);
        if self.mode.contains(VTMode::NewLine) {
            self.cursor.x = 0;
        }
    }

    fn index(&mut self, forward: bool) {
        // If the cursor is in the middle of the screen, just move it in the appropriate direction.
        // If it is at the edge of the scrolling region, peform a scroll up/down instead.
        match (forward, self.cursor.y) {
            (true, y) if y == self.scroll_rg.1 => self.scroll(1),
            (true, _) => self.cursor.y += 1,
            (false, y) if y == self.scroll_rg.0 => self.scroll(-1),
            (false, _) => self.cursor.y -= 1,
        }

        // XXX: Does this do the right thing if cursor is outside the scroll_rg?
    }

    fn next_line(&mut self) {
        self.index(true);
        self.cursor.x = 0;
    }

    fn erase(&mut self, erase: VTErase) {
        use VTErase::*;

        let (x, y) = (self.x(), self.y());
        let w = self.size.0 as usize;
        let h = self.size.1 as usize;
        let empty_char = self.empty_char();

        match erase {
            All => {
                while let Some(_line) = self.lines.pop_front() {
                    // TODO: Scrollback
                }
                let empty_line = self.empty_line();
                self.lines.resize(h, empty_line);
            },
            Above => {
                self.erase(LineLeft);
                let empty_line = self.empty_line();
                self.lines.iter_mut()
                    .take(y.saturating_sub(1))
                    .for_each(|l| *l = empty_line.clone());
            },
            Below => {
                self.erase(LineRight);
                let empty_line = self.empty_line();
                self.lines.iter_mut()
                    .skip(y)
                    .take(h)
                    .for_each(|l| *l = empty_line.clone());
            },
            Line => { self.current_line().fill(0, w, empty_char); },
            LineLeft => { self.current_line().fill(0, x + 1, empty_char); },
            LineRight => { self.current_line().fill(x, w, empty_char); },
            NumChars(num) => { self.current_line().fill(x, num as usize, empty_char); },
        }
    }

    fn tab(&mut self, mut tabs: i32) {
        let sgn = tabs.signum();
        let mut i = self.cursor.x as i32 + sgn;

        while tabs != 0 && i >= 0 && i < self.size.0 as i32 {
            if self.tabs[i as usize] { tabs -= 1; }
            if tabs == 0 { self.cursor.x = i as u32; }
            i += sgn;
        }
    }

    fn tab_set(&mut self, tab: bool) {
        let x = self.x();
        self.tabs[x] = tab;
    }

    fn tabs_clear(&mut self) {
        self.tabs.resize(0, false);
        self.tabs.resize(self.size.0 as usize, false);
    }

    fn resize(&mut self, cols: u32, rows: u32) {
        let cols = cols.max(SCREEN_SIZE_MIN.0);
        let rows = rows.max(SCREEN_SIZE_MIN.1);

        // Resize each line
        if cols > self.size.0 {
            for line in self.lines.iter_mut() {
                let ch = Cell::with_style(line.last().unwrap().style);
                line.resize(cols as usize, ch);
            }
        } else if cols < self.size.0 {
            self.cursor.x = self.cursor.x.min(cols - 1);
        }

        // Resize lines
        if rows < self.size.1 {
            let diff = self.size.1 - rows;

            // FIXME: try to remove empty lines from back first

            for _ in 0 .. diff {
                let _line = self.lines.pop_front();
                // TODO: Scrollback
            }

            self.cursor.y = self.cursor.y.saturating_sub(diff);
        } else if rows > self.size.1 {
            for _ in self.size.1 .. rows {
                self.lines.push_back(Line::new(Cell::with_style(self.cursor.style), cols));
            }
        }

        // Resize tabs
        if cols > self.size.0 {
            (self.size.0 .. cols)
                .map(|i| i % 8 == 0)
                .for_each(|tab| self.tabs.push(tab));
        }

        self.size = (cols, rows);
    }

    fn scroll(&mut self, mut num: i32) {
        // This is a nightmare in terms of off-by-one errors
        // Note that scroll region interval is inclusive

        // First make sure no more than the number of lines in scroll region is scrolled
        let srsize = (self.scroll_rg.1 - self.scroll_rg.0 + 1) as i32;
        if num == 0 { num = 1; }
        let num = num.min(srsize).max(-srsize);

        if !self.sr_set() {
            // Scroll the whole screen; this is the common case

            for _ in 0 .. num {
                // Scroll up
                let _line = self.lines.pop_front();
                // TODO: Scrollback
                let empty = self.empty_line();
                self.lines.push_back(empty);
            }

            for _ in num .. 0 {
                // Scroll down
                self.lines.pop_back();
                let empty = self.empty_line();
                self.lines.push_front(empty);
            }

            // FIXME: scrollup rendering metadata
        } else {
            // Scroll the scrolling region

            let scroll_rg = self.scroll_rg;
            self.scroll_generic(scroll_rg, num);

            // let (rg_0, rg_1) = (self.scroll_rg.0 as i32, self.scroll_rg.1 as i32);

            // if num > 0 {
            //     // Scroll up
            //     for i in rg_0 .. rg_1 - num + 1 {
            //         self.lines.swap(i as usize, (i + num) as usize);
            //     }
            //     for i in rg_1 - num + 1 .. rg_1 + 1 {
            //         let mut empty = self.empty_line();
            //         mem::swap(&mut empty, self.lines.get_mut(i as usize).expect("Lines index out of bounds"));
            //         if rg_0 == 0 {
            //             // TODO: Scrollback
            //         }
            //     }
            // } else {
            //     // Scroll down
            //     for i in (rg_0 + num .. rg_1).rev() {
            //         self.lines.swap(i as usize, (i - num) as usize);
            //     }
            //     for i in rg_0 .. rg_0 + num {
            //         let mut empty = self.empty_line();
            //         *self.lines.get_mut(i as usize).expect("Lines index out of bounds") = empty;
            //     }
            // }
        }
    }

    fn scroll_at_cursor(&mut self, mut num: i32) {
        if !self.cursor_in_sr() { return; }

        let x = self.cursor.x;
        let scroll_rg_bottom = self.scroll_rg.1;
        let num_lines = (scroll_rg_bottom - x + 1) as i32;
        if num == 0 { num = 1; }
        num = num.min(num_lines).max(-num_lines);
        self.scroll_generic((x, scroll_rg_bottom), num);
    }

    fn set_scroll_region(&mut self, mut top: u32, mut bottom: u32) {
        if bottom == 0 { bottom = self.size.1 - 1; }
        bottom = bottom.min(self.size.1 - 1);
        top = top.min(bottom - 1);

        self.scroll_rg = (top, bottom);
        self.cursor.x = 0;
        self.cursor.y = 0;
    }

    fn set_mode(&mut self, mode: VTMode, enable: bool) {
        self.mode.set(mode, enable);
    }

    fn set_rendition(&mut self, rend: VTRendition, enable: bool) {
        self.cursor.style.rendition.set(rend, enable);
    }

    fn set_fg(&mut self, color: VTColor) { self.cursor.style.col_fg = color; }
    fn set_bg(&mut self, color: VTColor) { self.cursor.style.col_bg = color; }

    fn charset_use(&mut self, slot: u32) { self.cursor.charset_use(slot); }
    fn charset_designate(&mut self, slot: u32, charset: VTCharset) { self.cursor.charset_designate(slot, charset); }

    fn reset(&mut self) { unimplemented!() }

    fn cursor_set(&mut self, x: Option<u32>, y: Option<u32>) {
        if let Some(y) = y {
            let cx = self.cursor.x;
            self.cursor_set_pos(x.map_or(cx, |x| x - 1), y - 1);
        } else if let Some(x) = x {
            self.cursor.x = self.clamp_x(x - 1);
        }
    }

    fn cursor_move(&mut self, x: i32, y: i32) {
        let cx = self.cursor.x as i32 + x;
        let cy = self.cursor.y as i32 + y;
        self.cursor_set_pos(cx as u32, cy as u32);
    }

    fn cursor_save(&mut self) {
        self.cursor_saved = self.cursor.clone();
        self.cursor_saved.mode_origin = self.mode.contains(VTMode::Origin);
    }

    fn cursor_load(&mut self) {
        self.cursor = self.cursor_saved.clone();
        self.mode.set(VTMode::Origin, self.cursor_saved.mode_origin);
        self.cursor.x = self.clamp_x(self.cursor.x);
        self.cursor.y = self.clamp_y(self.cursor.y);
    }

    fn alignment_test(&mut self) {
        let eeeeee = Line::new(Cell::new('E', self.cursor.style), self.size.0);
        for line in self.lines.iter_mut() {
            *line = eeeeee.clone();
        }
    }
}




#[cfg(test)]
mod tests {
use super::*;

#[test]
fn screen_scroll() {
    let mut screen = Screen::with_size((1, 10));
}

}
