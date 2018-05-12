use std::ops;
use std::collections::VecDeque;
// use unicode_normalization::char as unicode;
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
pub struct Char {
    chars: SmallString<[u8 ; 4]>,
    style: Style,
}

impl Char {
    pub fn new(ch: char, style: Style) -> Char {
        let mut res = Char {
            chars: SmallString::new(),
            style
        };
        res.push(ch);
        res
    }

    pub fn with_style(style: Style) -> Char {   // You gotta do stuff with_style!
        let mut ch = Char::default();
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
}

impl Default for Char {
    fn default() -> Char {
        Char {
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
    // TODO
    chars: Vec<Char>,
}

impl Line {
    pub fn new(ch: Char, width: u32) -> Line {
        Line {
            chars: vec![ch ; width as usize],
        }
    }
}

impl ops::Deref for Line {
    type Target = Vec<Char>;
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

const SCREEN_SIZE_MIN: (u32, u32) = (10, 5);
const SCREEN_SIZE_DEFALT: (u32, u32) = (80, 40);

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
    scroll_rg: (u32, u32),
    /// Tab stops
    tabs: Vec<bool>,
    /// The actual character data
    lines: VecDeque<Line>,
}

impl Screen {
    pub fn new() -> Screen {
        // By default we initialize the screen to minimum size
        let size = SCREEN_SIZE_MIN;

        let mut lines = VecDeque::with_capacity(size.1 as usize);
        for _ in 0 .. size.1 {
            lines.push_back(Line::new(Char::default(), size.0));
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

    fn empty_line(&self) -> Line {
        Line::new(Char::with_style(self.cursor.style), self.size.0)
    }

    fn x(&self) -> usize { self.cursor.x as usize }
    fn y(&self) -> usize { self.cursor.y as usize }

    fn clamp_x(&self, x: u32) -> u32 { if x >= self.size.0 { self.size.0 - 1 } else { x } }
    fn clamp_y(&self, y: u32) -> u32 { if y >= self.size.1 { self.size.1 - 1 } else { y } }

    fn sr_set(&self) -> bool { self.scroll_rg != (0, self.size.1 - 1) }

    fn curosr_in_sr(&self) -> bool {
        self.cursor.y >= self.scroll_rg.0 && self.cursor.y <= self.scroll_rg.1
    }

    fn current_char(&mut self) -> &mut Char {
        let x = self.x();
        self.lines.get_mut(self.cursor.y as usize).and_then(|line| line.get_mut(x)).expect("Cursor position out of bounds")
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

        // TODO: TADY

        if width == 0 {
            self.current_char().push(ch);
        } else {
            let x_last_valid = match width {
                1 => self.size.0 - 1,
                2 => self.size.0 - 2,
                _ => panic!("Unexpected character width: {}", width),
            };


            if self.cursor.x > x_last_valid {
                if self.mode.contains(VTMode::Wrap) && self.curosr_in_sr() {
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
                    *prev = Char::new(' ', prev.style);
                    prev.style.rendition.remove(VTRendition::Wide);
                    prev.set_dirty(true);
                }
            }

            if self.mode.contains(VTMode::Insert) {
                for c in line[x + 1 ..].iter_mut() {
                    *c = Char::with_style(self.cursor.style);
                }
            }

            let mut ch = Char::new(ch, self.cursor.style);
            if width == 2 {
                ch.style.rendition.insert(VTRendition::Wide);
                line[x + 1] = Char::with_style(self.cursor.style);
            }

            line[x] = ch;
            self.cursor.x += width as u32;
        }
    }

    fn put_chars(&mut self, num: u32) {
        let (x, y) = (self.x(), self.y());
        let line = &mut self.lines[y];
        for c in line[x..].iter_mut() {
            *c = Char::with_style(self.cursor.style);
        }
    }

    fn newline(&mut self) {
        self.index(true);
        if self.mode.contains(VTMode::NewLine) {
            self.cursor.x = 0;
        }
    }

    fn bell(&mut self) {
        unimplemented!()
        // TODO: push an event, same with report requests
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

    fn next_line(&mut self) { unimplemented!() }
    fn erase(&mut self, erase: VTErase) { unimplemented!() }

    fn tab(&mut self, tabs: i32) { unimplemented!() }
    fn tab_set(&mut self, tab: bool) { unimplemented!() }
    fn tabs_clear(&mut self) { unimplemented!() }

    fn resize(&mut self, cols: u32, rows: u32) {
        let cols = cols.min(SCREEN_SIZE_MIN.0);
        let rows = rows.min(SCREEN_SIZE_MIN.1);

        if cols > self.size.0 {
            for line in self.lines.iter_mut() {
                let ch = Char::with_style(line.last().unwrap().style);
                line.resize(cols as usize, ch);
            }
        } else if cols < self.size.0 {
            self.cursor.x = self.cursor.x.min(cols - 1);
        }

        if rows < self.size.1 {
            let diff = self.size.1 - rows;

            for _ in 0 .. diff {
                let _line = self.lines.pop_front();   // TODO: Scrollback
            }

            self.cursor.y = self.cursor.y.saturating_sub(diff);
        } else if rows > self.size.1 {
            for _ in self.size.1 .. rows {
                self.lines.push_back(Line::new(Char::with_style(self.cursor.style), cols));
            }
        }

        self.size = (cols, rows);
    }

    fn scroll(&mut self, num: i32) {
        if ! self.sr_set() {
            // Scroll the whole screen

            // FIXME: clamp num

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
        } else {
            // Scroll the scrolling region
            unimplemented!();
        }
    }

    fn scroll_at_cursor(&mut self, num: i32) { unimplemented!() }

    fn set_scroll_region(&mut self, top: u32, bottom: u32) { unimplemented!() }

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
        if let Some(x) = x { self.cursor.x = self.clamp_x(x - 1); }
        if let Some(y) = y { self.cursor.y = self.clamp_y(y - 1); }
    }

    fn cursor_move(&mut self, x: i32, y: i32) { unimplemented!() }
    fn cursor_save(&mut self) { unimplemented!() }
    fn cursor_load(&mut self) { unimplemented!() }

    fn alignment_test(&mut self) { unimplemented!() }
}
