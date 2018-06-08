use std::ops;
use std::slice::Iter;

use utf8;

// TODO: comment
// https://vt100.net/emu/dec_ansi_parser

// XXX: OSC may be terminated by bell (xterm extension?)

bitflags! {
    pub struct VTRendition: u8 {
        const Bold       = 1 << 0;
        const Underlined = 1 << 1;
        /// Blinking is sometimes implemented as synonimous to Bold.
        const Blinking   = 1 << 2;
        const Inverse    = 1 << 3;
        const Invisible  = 1 << 4;
        const All = 0x1f;

        /// Marks a wide unicode character
        const Wide       = 1 << 5;
        /// For rendering purposes
        const Dirty      = 1 << 6;
    }
}

impl Default for VTRendition {
    fn default() -> VTRendition {
        VTRendition::Dirty    // A sensible default, methinks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTColor {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl VTColor {
    fn parse<I: Iterator<Item=i32>>(p: i32, it: &mut I) -> Result<VTColor, ()> {
        use VTColor::*;

        match p {
            39 | 49 => return Ok(Default),

            // Basic colors
            30 ... 37 => return Ok(Indexed(p as u8 - 30)),
            40 ... 47 => return Ok(Indexed(p as u8 - 40)),

            // Bright colors
            90 ... 97 => return Ok(Indexed(p as u8 - 90 + 8)),
            100 ... 107 => return Ok(Indexed(p as u8 - 100 + 8)),

            // Extended color palettes, fall through to parsing below
            38 | 48 => {},

            _ => return Err(()),
        }

        // We need at least 2 params
        let op = it.next().ok_or(())?;
        let p1 = it.next().ok_or(())?;

        // Color cube format
        match (p, op, p1) {
            (38, 5, 0 ... 255) => return Ok(Indexed(p1 as u8)),
            (48, 5, 0 ... 255) => return Ok(Indexed(p1 as u8)),
            _ => {},
        }

        // For RGB format, we need 2 more params
        let p2 = it.next().ok_or(())?;
        let p3 = it.next().ok_or(())?;

        match (p, op, p1, p2, p3) {
            (38, 2, 0 ... 255, 0 ... 255, 0 ... 255) => Ok(Rgb(p1 as u8, p2 as u8, p3 as u8)),
            (48, 2, 0 ... 255, 0 ... 255, 0 ... 255) => Ok(Rgb(p1 as u8, p2 as u8, p3 as u8)),
            _ => Err(()),
        }
    }
}

impl Default for VTColor {
    fn default() -> VTColor {
        VTColor::Default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTCharset {
    /// Encoded 'B', the "regular" charset. Actually means UTF-8.
    UsAscii,
    /// Encoded '0', glyphs used for drawing windows, typically in curses-based programs.
    Graphics,
}

impl VTCharset {
    fn decode(byte: u8) -> Option<VTCharset> {
        use VTCharset::*;
        match byte {
            b'0' => Some(Graphics),
            b'B' => Some(UsAscii),
            _ => None,
        }
    }
}

impl Default for VTCharset {
    fn default() -> VTCharset {
        VTCharset::UsAscii
    }
}

bitflags! {
    pub struct VTMode: u8 {
        const Wrap         = 1 << 0;
        const Origin       = 1 << 1;
        const NewLine      = 1 << 2;
        const Insert       = 1 << 3;
        const ReverseVideo = 1 << 4;
    }
}

impl Default for VTMode {
    fn default() -> VTMode {
        VTMode::Wrap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTErase {
    All,
    Above,
    Below,
    Line,
    LineLeft,
    LineRight,
    NumChars(u32),
}


#[derive(Debug)]
struct Params {
    ints: Vec<i32>,
    open: bool,
}

impl Params {
    fn new() -> Params {
        Params {
            ints: vec![],
            open: false,
        }
    }

    fn push_digit(&mut self, digit: u8) {
        if self.ints.len() > 16 { return; }

        if !self.open { self.ints.push(0); }

        let last = self.ints.last_mut().unwrap();
        if let Some(num) = last.checked_mul(10).and_then(|num| num.checked_add(digit as i32)) {
            *last = num;
        }

        self.open = true;
    }

    fn next(&mut self) {
        if self.open && self.ints.len() <= 16 {
            self.open = false;
        }
    }

    fn get(&self, index: usize, default: i32) -> i32 {
        match self.ints.get(index) {
            Some(0) | None => default,
            Some(p) => *p,
        }
    }

    fn clear(&mut self) {
        self.ints.clear();
        self.open = false;
    }
}

impl ops::Deref for Params {
    type Target = Vec<i32>;
    fn deref(&self) -> &Self::Target { &self.ints }
}

impl ops::DerefMut for Params {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.ints }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    // Text input states
    Ground,

    // Escape sequence states
    Escape,
    EscapeInterm,
    CsiEntry,
    CsiParam,
    CsiInterm,
    CsiIgnore,

    // Control string states
    ApcEntry,
    ApcInterm,
    ApcTp,
    CtrlStrIgnore,
}

use self::State::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTReport {
    AnswerBack,
    PrimaryAttrs,
    SecondaryAttrs,
    DeviceStatus,
    CursorPos,
    TermParams0,
    TermParams1,
    Bell,   // XXX: move
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTScreenChoice {
    Primary,
    Alternate,
}

impl Default for VTScreenChoice {
    fn default() -> VTScreenChoice {
        VTScreenChoice::Primary
    }
}

pub trait VTScreen {
    /// Insert a unicode charater
    fn put_char(&mut self, ch: char);
    /// Insert `num` default (empty) characters
    fn put_chars(&mut self, num: u32);
    fn newline(&mut self);
    // fn bell(&mut self);
    fn index(&mut self, forward: bool);
    fn next_line(&mut self);
    /// Performs an erase operation pertaining to current cursor location.
    /// See `VTErase`.
    fn erase(&mut self, erase: VTErase);

    /// Move cursor to the next tab stop
    fn tab(&mut self, tabs: i32);
    fn tab_set(&mut self, tab: bool);
    fn tabs_clear(&mut self);

    fn reset(&mut self);
    fn resize(&mut self, cols: u32, rows: u32);   // XXX: this probably shouldn't be part of the trait

    /// Scroll screen or the scrolling region if any.
    /// Positive `num` is for scrolling up, negative for scrolling down.
    fn scroll(&mut self, num: i32);

    /// Scroll between current line and the bottom of the screen or the scrolling region if any.
    /// If the current line is outside of the scrolling region, there is no effect.
    /// Positive `num` is for scrolling up, negative for scrolling down. In VT jargon this is known as
    /// Delete Lines and Insert Lines, respectively.
    fn scroll_at_cursor(&mut self, num: i32);

    /// Set scrolling region.
    /// `top` should be at least `1`, `bottom` should be strictly larger than `top`,
    /// and `top` should be within screen's height.
    /// If those conditions are not met, apply the default action - reseting the scroll region to the whole screen.
    /// Note that `1` means "the first line" (ie. 1-indexing).
    fn set_scroll_region(&mut self, top: u32, bottom: u32);

    fn set_mode(&mut self, mode: VTMode, enable: bool);
    fn set_rendition(&mut self, rend: VTRendition, enable: bool);
    fn set_fg(&mut self, color: VTColor);
    fn set_bg(&mut self, color: VTColor);

    fn charset_use(&mut self, slot: u32);
    fn charset_designate(&mut self, slot: u32, charset: VTCharset);

    /// Set cursor absolute position
    /// Note that the coordinates are 1-indexed.
    fn cursor_set(&mut self, x: Option<u32>, y: Option<u32>);
    /// Set cursor relative position
    fn cursor_move(&mut self, x: i32, y: i32);
    /// Save current cursor data
    ///
    /// Saves current cursor's position as well as its style, current character set,
    /// and the four desginated character set slots into the saved cursor slot.
    /// Any previous data in the saved cursor slot is overwritten.
    fn cursor_save(&mut self);
    /// Restores the data saved with `cursor_save()`
    fn cursor_load(&mut self);

    /// "DEC Screen Alignment Test ", actually means the whole screen is filled with `E`s (with default style).
    fn alignment_test(&mut self);
}

pub trait VTDispatch {
    type Screen: VTScreen;

    /// Reference the current screen
    fn screen(&mut self) -> &mut Self::Screen;

    /// Reference the primary screen
    fn screen_primary(&mut self) -> &mut Self::Screen;

    /// Reference the alternate screen
    fn screen_alternate(&mut self) -> &mut Self::Screen;

    /// Set current screen
    fn switch_screen(&mut self, screen: VTScreenChoice);

    /// Set mode on both screens
    fn set_mode(&mut self, mode: VTMode, enable: bool);

    /// Queue up a terminal report request
    fn report_request(&mut self, report: VTReport);

    // TP extensions:
    // TODO
}

#[derive(Debug)]
pub struct VTParser {
    state: State,
    utf8: utf8::Parser,
    interm1: u8,
    interm2: u8,
    params: Params,
}

#[derive(Debug)]
struct Dispatcher<'s, 'd, D: VTDispatch + 'static> {
    p: &'s mut VTParser,
    d: &'d mut D,
}

impl<'s, 'd, D: VTDispatch + 'static> Dispatcher<'s, 'd, D> {
    fn screen(&mut self) -> &mut D::Screen {
        self.d.screen()
    }

    fn clear(&mut self) {
        // Clear the state, checking if for an incomplete UTF-8 parse in progess.
        if self.p.clear().is_err() {
            self.screen().put_char(utf8::REPLACE_CHAR);
        }
    }

    /// Checks for C0 bytes that need to be performed in all states
    fn check_anywhere(&mut self, byte: u8) -> Option<State> {
        // These ones work anywhere and interrupt ANY escape sequence
        match byte {
            0x18 | 0x1a => {
                // A previous escape sequence, if there was one, gets canceled
                self.clear();
                return Some(Ground);
            },
            0x1b => return Some(Escape),    // TODO: dispatch ApcTp here ???
            _ => {},
        }

        // The rest of C0 is interpreted in basic escapes and CSIs, but not in control string sequences (ie. DCS, APC et al.)
        // So we first check for that, then the C0 char executes, and then the escape sequence continues (if any).
        match self.p.state {
            ApcEntry | ApcInterm | ApcTp | CtrlStrIgnore => return None,
            _ => {},
        }

        match byte {
            0 ... 4 | 6 | 0x10 ... 0x17 | 0x19 | 0x1c ... 0x1f | 0x7f => {
                    // Ignored: NUL, SOH, STX, ETX, EOT, ACK, DLE, DC1, DC2, DC3, DC4, NAK, SYN, ETB1, FS, GS, RS, US
                },
            5 /* ENQ */ => self.d.report_request(VTReport::AnswerBack),
            // 7 /* BEL */ => self.screen().bell(),
            7 /* BEL */ => self.d.report_request(VTReport::Bell),
            8 /* BS */  => self.screen().cursor_move(-1, 0),
            9 /* HT */  => self.screen().tab(1),
            0xa ... 0xc /* LF, VT, FF */ => self.screen().newline(),
            0xd /* CR */  => self.screen().cursor_set(Some(1), None),
            0xe /* SO */  => self.screen().charset_use(1),
            0xf /* SI */  => self.screen().charset_use(0),
            _ => return None,
        }

        Some(self.p.state)
    }

    fn ground(&mut self, byte: u8) -> State {
        if let Some(res) = self.p.utf8.input(byte) {
            self.screen().put_char(res.unwrap_or(utf8::REPLACE_CHAR));
        }

        Ground
    }

    fn escape(&mut self, byte: u8) -> State {
        self.clear();   // Legal, because we'll transition to another state

        let d = &mut self.d;

        match byte {
            0x20 ... 0x2f => {
                self.p.interm1 = byte;
                return EscapeInterm;
            },

            b'D' => d.screen().index(true),
            b'E' => d.screen().next_line(),
            b'H' => d.screen().tab_set(true),
            b'M' => d.screen().index(false),
            b'Z' => d.report_request(VTReport::PrimaryAttrs),
            b'7' => d.screen().cursor_save(),
            b'8' => d.screen().cursor_load(),
            b'c' => d.screen().reset(),
            b'n' => d.screen().charset_use(2),
            b'o' => d.screen().charset_use(3),

            b'[' => return CsiEntry,
            b'_' => return ApcEntry,
            b'P' | b'X' | b']' | b'^' => return CtrlStrIgnore,

            _ => {
                // Other sequences ignored either by specification or because we don't implement them
            }
        }

        Ground
    }

    fn charset_designate(&mut self, slot: u32, param: u8) {
        if let Some(charset) = VTCharset::decode(param) {
            self.screen().charset_designate(slot, charset);
        }
    }

    fn escape_interm(&mut self, byte: u8) -> State {
        match (self.p.interm1, byte) {
            // XXX: Ok to go to ground for 0x20 ~ 0x2f ??? (ditto csi_interm) VT100.net says collect
            (b'#', b'8') => self.screen().alignment_test(),
            (b'(', p) => self.charset_designate(0, p),
            (b')', p) => self.charset_designate(1, p),
            (b'*', p) => self.charset_designate(2, p),
            (b'+', p) => self.charset_designate(3, p),
            _ => {},
        }

        Ground
    }

    fn csi_modes(&mut self, enable: bool) {
        for m in self.p.params.iter() {
            match *m {
                4 => self.d.set_mode(VTMode::Insert, enable),
                20 => self.d.set_mode(VTMode::NewLine, enable),
                _ => {},
            }
        }
    }

    fn csi_modes_dec(&mut self, enable: bool) {
        for m in self.p.params.iter() {
            match *m {
                5 => self.d.set_mode(VTMode::ReverseVideo, enable),
                6 => {
                    self.d.set_mode(VTMode::Origin, enable);
                    self.d.screen_primary().cursor_set(Some(1), Some(1));
                    self.d.screen_alternate().cursor_set(Some(1), Some(1));
                },
                20 => self.d.set_mode(VTMode::NewLine, enable),   // FIXME: also applies to input
                47 | 1047 if  enable => self.d.switch_screen(VTScreenChoice::Primary),
                47 | 1047 if !enable => self.d.switch_screen(VTScreenChoice::Alternate),
                1048 if  enable => self.d.screen().cursor_save(),
                1048 if !enable => self.d.screen().cursor_load(),
                1049 if  enable => {
                    self.d.screen_primary().cursor_save();
                    self.d.switch_screen(VTScreenChoice::Alternate);
                    self.d.screen_alternate().erase(VTErase::All);
                },
                1049 if !enable => {
                    self.d.switch_screen(VTScreenChoice::Primary);
                    self.d.screen_primary().cursor_load();
                },
                _ => {},
            }
        }
    }

    /// Character rendition setting. The one escape sequence people actually know to exist.
    fn csi_sgr(&mut self) {
        if self.p.params.len() == 0 {
            self.screen().set_rendition(VTRendition::All, false);
            return;
        }

        let screen = &mut self.d.screen();
        let mut it = self.p.params.iter().map(|p| *p);
        while let Some(p) = it.next() {
            match p {
                0 => screen.set_rendition(VTRendition::All, false),
                1 => screen.set_rendition(VTRendition::Bold, true),
                4 => screen.set_rendition(VTRendition::Underlined, true),
                5 => screen.set_rendition(VTRendition::Blinking, true),
                7 => screen.set_rendition(VTRendition::Inverse, true),
                8 => screen.set_rendition(VTRendition::Invisible, true),

                22 => screen.set_rendition(VTRendition::Bold, false),
                24 => screen.set_rendition(VTRendition::Underlined, false),
                25 => screen.set_rendition(VTRendition::Blinking, false),
                27 => screen.set_rendition(VTRendition::Inverse, false),
                28 => screen.set_rendition(VTRendition::Invisible, false),

                // Foreground color
                30 ... 39 | 90 ... 97 => {
                    match VTColor::parse(p, &mut it) {
                        Ok(c) => screen.set_fg(c),
                        Err(_) => return,
                    }
                },

                // Background color
                40 ... 49 | 100 ... 107 => {
                    match VTColor::parse(p, &mut it) {
                        Ok(c) => screen.set_bg(c),
                        Err(_) => return,
                    }
                },

                _ => {
                    // What's the right thing to do here?
                    // I'd say just bail to avoid interpreting params that might be bogus ...
                    return;
                },
            }
        }
    }

    fn csi_erase(&mut self, screen: bool) {
        match (screen, self.p.params.get(0, 0)) {
            (true, 0) => self.screen().erase(VTErase::Below),
            (true, 1) => self.screen().erase(VTErase::Above),
            (true, 2) => self.screen().erase(VTErase::All),
            (false, 0) => self.screen().erase(VTErase::LineRight),
            (false, 1) => self.screen().erase(VTErase::LineLeft),
            (false, 2) => self.screen().erase(VTErase::Line),
            _ => {},
        }
    }

    fn csi_tab(&mut self) {
        match self.p.params.get(0, 0) {
            0 => self.screen().tab_set(false),
            3 => self.screen().tabs_clear(),
            _ => {},
        }
    }

    fn csi_dsr(&mut self) {
        match self.p.params.get(0, 0) {
            5 => self.d.report_request(VTReport::DeviceStatus),
            6 => self.d.report_request(VTReport::CursorPos),
            _ => {},
        }
    }

    fn csi_tparm(&mut self) {
        match self.p.params.get(0, 0) {
            0 => self.d.report_request(VTReport::TermParams0),
            1 => self.d.report_request(VTReport::TermParams1),
            _ => {},
        }
    }

    fn csi_dispatch(&mut self, byte: u8) -> State {
        if self.p.interm1 != 0 {
            match (self.p.interm1, byte) {
                (b'>', b'c') => self.d.report_request(VTReport::SecondaryAttrs),
                (b'?', b'h') => self.csi_modes_dec(true),
                (b'?', b'l') => self.csi_modes_dec(false),

                // XXX: support these? xterm specific?
                // case csi("?r"): /* restore mode */ break;
                // case csi("?s"): /* save mode */ break;
                _ => {},
            }
            return Ground;
        }

        if self.p.interm2 != 0 {
            // We support no such CSIs
            return Ground;
        }

        match byte {
            b'J' => self.csi_erase(true),
            b'K' => self.csi_erase(false),
            b'g' => self.csi_tab(),
            b'h' => self.csi_modes(true),
            b'l' => self.csi_modes(false),
            b'm' => self.csi_sgr(),
            b'n' => self.csi_dsr(),
            b'x' => self.csi_tparm(),
            _ => {
                // The saga continues in the following match.
                // This is done to make borrowchecker happy because for the rest of CSIs
                // we want to borrow both the dispatcher as well as params.
            },
        }

        let d = &mut self.d;
        let params = &self.p.params;

        match byte {
            b'@' => d.screen().put_chars(params.get(0, 1) as u32),
            b'A' => d.screen().cursor_move(0, -params.get(0, 1)),
            b'B' => d.screen().cursor_move(0,  params.get(0, 1)),
            b'C' => d.screen().cursor_move( params.get(0, 1), 0),
            b'D' => d.screen().cursor_move(-params.get(0, 1), 0),
            b'G' => d.screen().cursor_set(Some(params.get(0, 1) as u32), None),
            b'f' | b'H' => d.screen().cursor_set(Some(params.get(1, 1) as u32), Some(params.get(0, 1) as u32)),
            b'I' => d.screen().tab(params.get(0, 1)),
            b'L' => d.screen().scroll_at_cursor(-params.get(0, 1)),
            b'M' => d.screen().scroll_at_cursor(params.get(0, 1)),
            b'P' => d.screen().erase(VTErase::NumChars(params.get(0, 1) as u32)),   // TODO: is this right?
            b'S' => d.screen().scroll( params.get(0, 1)),
            b'T' => d.screen().scroll(-params.get(0, 1)),
            b'X' => d.screen().erase(VTErase::NumChars(params.get(0, 1) as u32)),
            b'Z' => d.screen().tab(-params.get(0, 1)),
            b'c' => d.report_request(VTReport::PrimaryAttrs),
            b'd' => d.screen().cursor_set(None, Some(params.get(0, 1) as u32)),
            b'r' => d.screen().set_scroll_region(params.get(0, 0) as u32, params.get(1, 0) as u32),
            b's' => d.screen().cursor_save(),
            b'u' => d.screen().cursor_load(),
            _ => {
                // Other sequences ignored either by specification or because we don't implement them
            },
        }

        Ground
    }

    fn csi_entry(&mut self, byte: u8) -> State {
        self.clear();   // Legal, because we'll transition to another state

        match byte {
            0x20 ... 0x2f => {
                self.p.interm2 = byte;
                CsiInterm
            },
            b'<' | b'=' | b'>' | b'?' => {
                self.p.interm1 = byte;
                CsiParam
            },
            b'0' ... b'9' => {
                self.p.params.push_digit(byte - b'0');
                CsiParam
            },
            b';' => {
                self.p.params.next();
                CsiParam
            },
            b':' => CsiIgnore,
            0x40 ... 0x7e => self.csi_dispatch(byte),
            _ => Ground,
        }
    }

    fn csi_param(&mut self, byte: u8) -> State {
        match byte {
            0x20 ... 0x2f => {
                self.p.interm2 = byte;
                CsiInterm
            }
            b'0' ... b'9' => {
                self.p.params.push_digit(byte - b'0');
                CsiParam
            },
            b';' => {
                self.p.params.next();
                CsiParam
            },
            b':' | b'<' | b'=' | b'>' | b'?' => CsiIgnore,
            0x40 ... 0x7e => {
                self.csi_dispatch(byte);
                Ground
            },
            _ => CsiParam
        }
    }

    fn csi_interm(&mut self, byte: u8) -> State {
        match byte {
            // XXX: Ok to go to ground for 0x20 ~ 0x2f ???
            0x30 ... 0x3f => CsiIgnore,
            0x40 ... 0x7e => {
                self.csi_dispatch(byte);
                Ground
            },
            _ => Ground
        }
    }

    fn csi_ignore(&mut self, byte: u8) -> State {
        match byte {
            0x20 ... 0x3f => CsiIgnore,
            _ => Ground
        }
    }

    fn apc_entry(&mut self, byte: u8) -> State {
        self.clear();   // Legal, because we'll transition to another state

        match byte {
            b'T' => ApcInterm,
            _ => CtrlStrIgnore,
        }
    }

    fn apc_interm(&mut self, byte: u8) -> State {
        match byte {
            b'P' => ApcTp,
            _ => CtrlStrIgnore,
        }
    }

    fn apc_tp(&mut self, byte: u8) -> State {
        // TODO
        ApcTp
    }

    fn ctrl_str_ignore(&mut self, _byte: u8) -> State {
        // All input is just ignored here.
        // (Except for 0x18, 0x19, and 0x1b, but those are checked in `check_anywhere()`.)
        self.p.state
    }

    fn input(&mut self, byte: u8) {
        if let Some(state) = self.check_anywhere(byte) {
            self.p.state = state;
            return;
        }

        self.p.state = match self.p.state {
            Ground        => Self::ground,
            Escape        => Self::escape,
            EscapeInterm  => Self::escape_interm,
            CsiEntry      => Self::csi_entry,
            CsiParam      => Self::csi_param,
            CsiInterm     => Self::csi_interm,
            CsiIgnore     => Self::csi_ignore,
            ApcEntry      => Self::apc_entry,
            ApcInterm     => Self::apc_interm,
            ApcTp         => Self::apc_tp,
            CtrlStrIgnore => Self::ctrl_str_ignore,
        } (self, byte);
    }
}


impl VTParser {
    pub fn new() -> VTParser {
        VTParser {
            state: State::Ground,
            utf8: utf8::Parser::new(),
            interm1: 0,
            interm2: 0,
            params: Params::new(),
        }
    }

    fn clear(&mut self) -> Result<(), ()> {
        // TODO: clear parsing data
        self.params.clear();
        self.interm1 = 0;
        self.interm2 = 0;
        self.utf8.reset()
    }

    pub fn input<D: VTDispatch + 'static>(&mut self, data: &[u8], dispatch: &mut D) {
        // TODO

        let mut dispatcher = Dispatcher { p: self, d: dispatch };
        for byte in data {
            dispatcher.input(*byte);
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;


    type Call = (&'static str, Vec<String>);

    macro_rules! call {
        ($name:ident) => ((stringify!($name), vec![]));
        ($name:ident, $($arg:expr),*) => ((stringify!($name), vec![$(format!("{:?}", $arg)),*]));
    }

    #[derive(Debug, Default)]
    struct TestDispatch {
        calls: Vec<Call>,
    }

    impl TestDispatch {
        fn calls(&mut self) -> Vec<Call> { mem::replace(&mut self.calls, vec![]) }
    }

    macro_rules! dispatch_impl {
        ($name:ident) => {
            fn $name(&mut self) { self.calls.push((call!($name))) }
        };
        ($name:ident, $($arg:ident : $ty:ty),+) => {
            fn $name(&mut self, $($arg : $ty),*) { self.calls.push(call!($name, $($arg),*)) }
        };
    }

    impl VTScreen for TestDispatch {
        dispatch_impl!(put_char, ch: char);
        dispatch_impl!(put_chars, num: u32);
        dispatch_impl!(newline);
        dispatch_impl!(index, forward: bool);
        dispatch_impl!(next_line);
        dispatch_impl!(erase, erase: VTErase);
        dispatch_impl!(tab, tabs: i32);
        dispatch_impl!(tab_set, tab: bool);
        dispatch_impl!(tabs_clear);
        dispatch_impl!(reset);
        dispatch_impl!(resize, cols: u32, rows: u32);
        dispatch_impl!(scroll, num: i32);
        dispatch_impl!(scroll_at_cursor, num: i32);
        dispatch_impl!(set_scroll_region, top: u32, bottom: u32);
        dispatch_impl!(set_mode, mode: VTMode, enable: bool);
        dispatch_impl!(set_rendition, rend: VTRendition, enable: bool);
        dispatch_impl!(set_fg, color: VTColor);
        dispatch_impl!(set_bg, color: VTColor);
        dispatch_impl!(charset_use, slot: u32);
        dispatch_impl!(charset_designate, slot: u32, charset: VTCharset);
        dispatch_impl!(cursor_set, x: Option<u32>, y: Option<u32>);
        dispatch_impl!(cursor_move, x: i32, y: i32);
        dispatch_impl!(cursor_save);
        dispatch_impl!(cursor_load);
        dispatch_impl!(alignment_test);
    }

    impl VTDispatch for TestDispatch {
        type Screen = TestDispatch;

        fn screen(&mut self) -> &mut Self::Screen { self }
        fn screen_primary(&mut self) -> &mut Self::Screen { self }
        fn screen_alternate(&mut self) -> &mut Self::Screen { self }

        dispatch_impl!(switch_screen, screen: VTScreenChoice);
        dispatch_impl!(set_mode, mode: VTMode, enable: bool);
        dispatch_impl!(report_request, report: VTReport);
    }

    macro_rules! parse {
        ($input:expr) => {{
            let mut parser = VTParser::new();
            let mut dispatch = TestDispatch::default();
            parser.input($input, &mut dispatch);
            dispatch.calls()
        }};
    }

    #[test]
    fn put_char() {
        assert_eq!(parse!(b"Hello!"), vec![
            call!(put_char, 'H'),
            call!(put_char, 'e'),
            call!(put_char, 'l'),
            call!(put_char, 'l'),
            call!(put_char, 'o'),
            call!(put_char, '!'),
        ]);
    }

    #[test]
    fn cancelations() {
        assert_eq!(parse!(b"\x1b[1;30\x18\x1b[34m"), vec![ call!(set_fg, VTColor::Indexed(4)) ]);
        assert_eq!(parse!(b"\x1b[1;30\x1a\x1b[34m"), vec![ call!(set_fg, VTColor::Indexed(4)) ]);
    }

    #[test]
    fn interleave_csi_c0() {
        assert_eq!(parse!(b"\x1b[1;\x0530\x18\x1b[3\x0e4m"), vec![
            call!(report_request, VTReport::AnswerBack),
            call!(charset_use, 1),
            call!(set_fg, VTColor::Indexed(4)),
        ]);
    }

    #[test]
    fn alignment_test() {
        assert_eq!(parse!(b"\x1b#8"), vec![ call!(alignment_test) ]);
    }

    // TODO: more tests
}
