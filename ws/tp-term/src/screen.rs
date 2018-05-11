use std::collections::VecDeque;
// use unicode_normalization::char as unicode;

use ::smallstring::*;
use ::vt::*;


#[derive(Debug, Default, Clone)]
pub struct Style {
    col_fg: VTColor,
    col_bg: VTColor,
    rendition: VTRendition,
}

#[derive(Debug, Default, Clone)]
/// Character as part of the screen's grid, has associated `Style`
///
/// May actually consist of more than one unicode characters if combining marks are present.
pub struct Char {
    chars: SmallString<[u8 ; 4]>,
    style: Style,
}

impl Char {
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

    pub fn push(&mut self, ch: char) {
        self.chars.push(ch);
    }

    // TODO: &str
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
    pub fn new() -> Line {
        Line {
            chars: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Cursor {
    x: u32,
    y: u32,
    style: Style,
}

#[derive(Debug)]
pub struct Screen {
    // TODO
    /// Size: width, height
    size: (u32, u32),
    /// scrol region size: top, bottom
    scrol_region: Option<(u32, u32)>,
    /// Current cursor data
    cursor: Cursor,
    /// Saved cursor data (for the VT curosr save & restore functionality)
    cursor_saved: Cursor,
    /// VT Charset slots (four by specification)
    charsets: [VTCharset ; 4],
    /// The actual character data
    lines: VecDeque<Line>,
}

impl Screen {
    pub fn new() -> Screen {
        Screen {
            size: (80, 20),
            scrol_region: None,
            cursor: Cursor::default(),
            cursor_saved: Cursor::default(),
            charsets: [VTCharset::default() ; 4],
            lines: VecDeque::new(),
        }
    }
}

impl VTScreen for Screen {
    fn putc(&mut self, c: char) { unimplemented!() }

    fn newline(&mut self) { unimplemented!() }
    fn tab(&mut self, tabs: i32) { unimplemented!() }
    fn bell(&mut self) { unimplemented!() }

    fn set_mode(&mut self, mode: VTMode, enable: bool) { unimplemented!() }
    fn set_rendition(&mut self, rend: VTRendition, enable: bool) { unimplemented!() }
    fn set_fg(&mut self, color: VTColor) { unimplemented!() }
    fn set_bg(&mut self, color: VTColor) { unimplemented!() }

    fn charset_use(&mut self, slot: u8) { unimplemented!() }
    fn charset_designate(&mut self, slot: u8, charset: VTCharset) { unimplemented!() }

    fn index(&mut self, forward: bool) { unimplemented!() }
    fn next_line(&mut self) { unimplemented!() }
    fn tab_set(&mut self, tab: bool) { unimplemented!() }
    fn alignment_test(&mut self) { unimplemented!() }
    fn reset(&mut self) { unimplemented!() }

    fn cursor_set(&mut self, x: Option<i32>, y: Option<i32>) { unimplemented!() }
    fn cursor_move(&mut self, x: i32, y: i32) { unimplemented!() }
    fn cursor_save(&mut self) { unimplemented!() }
    fn cursor_load(&mut self) { unimplemented!() }
}
