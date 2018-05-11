use std::ops;
use std::slice::Iter;

use utf8;

// TODO: comment
// https://vt100.net/emu/dec_ansi_parser


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
        VTRendition::from_bits_truncate(0)
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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


#[derive(Debug)]
struct IntParams {
    ints: Vec<i32>,
    open: bool,
}

impl IntParams {
    fn new() -> IntParams {
        IntParams {
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
        self.ints.get(index).map(|v| *v).unwrap_or(default)
    }

    fn clear(&mut self) {
        self.ints.clear();
        self.open = false;
    }
}

impl ops::Deref for IntParams {
    type Target = Vec<i32>;
    fn deref(&self) -> &Self::Target { &self.ints }
}

impl ops::DerefMut for IntParams {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.ints }
}


#[derive(Debug, Clone, Copy)]
enum State {
    // Text input states
    Ground,
    // TODO: UTF-8

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

// impl State {
//     fn executes_c0(&self) -> bool {
//         match *self {
//             Ground | Escape | EscapeInterm | CsiEntry | CsiIgnore | CsiParam | CsiInterm => true,
//             _ => false,
//         }
//     }
// }

#[derive(Debug, Clone, Copy)]
pub enum VTReport {
    // TODO: remove stuff we don't support
    AnswerBack,
    PrimaryAttrs,
    SecondaryAttrs,
    // DeviceStatus,
    // CursorPos,
    // VT52Identify,
    // TermParams0,
    // TermParams1,
}

pub trait VTScreen {
    fn putc(&mut self, c: char);

    fn newline(&mut self);
    fn tab(&mut self, tabs: i32);
    fn bell(&mut self);

    fn set_mode(&mut self, mode: VTMode, enable: bool);
    fn set_rendition(&mut self, rend: VTRendition, enable: bool);
    fn set_fg(&mut self, color: VTColor);
    fn set_bg(&mut self, color: VTColor);

    fn charset_use(&mut self, slot: u8);
    fn charset_designate(&mut self, slot: u8, charset: VTCharset);

    fn index(&mut self, forward: bool);
    fn next_line(&mut self);
    fn tab_set(&mut self, tab: bool);
    fn alignment_test(&mut self);
    fn reset(&mut self);

    fn cursor_set(&mut self, x: Option<i32>, y: Option<i32>);
    fn cursor_move(&mut self, x: i32, y: i32);
    fn cursor_save(&mut self);
    fn cursor_load(&mut self);
}

pub trait VTDispatch {
    type Screen: VTScreen;

    fn screen(&mut self) -> &mut Self::Screen;

    fn set_mode(&mut self, mode: VTMode, enable: bool);
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
    params: IntParams,
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
            self.screen().putc(utf8::REPLACE_CHAR);
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
            7 /* BEL */ => self.screen().bell(),
            8 /* BS */  => self.screen().cursor_move(-1, 0),
            9 /* HT */  => self.screen().tab(1),
            0xa ... 0xc /* LF, VT, FF */ => self.screen().newline(),
            0xd /* CR */  => self.screen().cursor_set(Some(0), None),
            0xe /* SO */  => self.screen().charset_use(1),
            0xf /* SI */  => self.screen().charset_use(0),
            _ => return None,
        }

        Some(self.p.state)
    }

    fn ground(&mut self, byte: u8) -> State {
        if let Some(res) = self.p.utf8.input(byte) {
            self.screen().putc(res.unwrap_or(utf8::REPLACE_CHAR));
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

    fn charset_designate(&mut self, slot: u8, param: u8) {
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

    fn csi_dispatch(&mut self, byte: u8) -> State {
        if self.p.interm1 != 0 {
            match (self.p.interm1, byte) {
                (b'>', b'c') => self.d.report_request(VTReport::SecondaryAttrs),
                // case csi("?h"): csi_dec_modes_set(); break;
                // case csi("?l"): if (csi_dec_modes_decanm()) fgoto vt52; break;
                // case csi("?r"): /* TODO: restore mode */ break; /* xterm specific ? */
                // case csi("?s"): /* TODO: save mode */ break;    /* xterm specific ? */
                _ => {},
            }
            return Ground;
        }

        if self.p.interm2 != 0 {
            // We support no such CSIs
            return Ground;
        }

        match byte {
            b'h' => self.csi_modes(true),
            b'l' => self.csi_modes(false),
            b'm' => { let _ = self.csi_sgr(); },
            b'n' => unimplemented!(), // csi_dsr(args_int->value(0, 0)),
            b'x' => unimplemented!(), // csi_tparm(args_int->value(0, 0)),
            _ => {
                // The saga continues in the following match.
                // This is done to make borrowchecker happy because for the rest of CSIs
                // we want to borrow both the dispatcher as well as params.
            },
        }

        let d = &mut self.d;
        let params = &self.p.params;

        match byte {
            b'@' => unimplemented!(), // d.screen().insertChars(params.get(0, 1)),
            b'A' => d.screen().cursor_move(0, -params.get(0, 1)),
            b'B' => d.screen().cursor_move(0,  params.get(0, 1)),
            b'C' => d.screen().cursor_move( params.get(0, 1), 0),
            b'D' => d.screen().cursor_move(-params.get(0, 1), 0),
            b'G' => d.screen().cursor_set(Some(params.get(0, 1) - 1), None),
            b'f' | b'H' => d.screen().cursor_set(Some(params.get(1, 1) - 1), Some(params.get(0, 1) - 1)),
            b'I' => d.screen().tab(params.get(0, 1)),
            b'J' => unimplemented!(), // d.screen().erase(static_cast<Erase>(args_int->value(0, 0))),
            b'K' => unimplemented!(), // d.screen().eraseInLine(static_cast<EraseInLine>(args_int->value(0, 0))),
            b'L' => unimplemented!(), // d.screen().insertLines(params.get(0, 1)),
            b'M' => unimplemented!(), // d.screen().deleteLines(params.get(0, 1)),
            b'P' => unimplemented!(), // d.screen().deleteChars(params.get(0, 1)),
            b'S' => unimplemented!(), // d.screen().scrollUp(params.get(0, 1)),
            b'T' => unimplemented!(), // d.screen().scrollDown(params.get(0, 1)),
            b'X' => unimplemented!(), // d.screen().eraseInLine(EraseInLine::NumChars, params.get(0, 1)),
            b'Z' => d.screen().tab(-params.get(0, 1)),
            b'c' => d.report_request(VTReport::PrimaryAttrs),
            b'd' => d.screen().cursor_set(None, Some(params.get(0, 1) - 1)),
            b'g' => unimplemented!(), // screen().tabSet(false, args_int->value(0, 0)),
            b'r' => unimplemented!(), // screen().setMargins(params.get(0, 1)-1, params.get(1, screen().height())-1),
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
            params: IntParams::new(),
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

