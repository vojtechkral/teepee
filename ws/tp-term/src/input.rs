use std::io::Write;

use ::vt::*;
use ::screen::Screen;


/// Input Key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Return,
    Tab,
    Backspace,
    Up,
    Down,
    Right,
    Left,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    F(u8),
    Char(char),
}

/// Modifier key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// No modifier being pressed
    None,
    /// A.k.a the `Alt` key
    Mod3,
    /// A.k.a the `Control` key
    Mod5,
}

pub type KeyMod = (Key, Modifier);

#[derive(Debug)]
pub struct VTInput;

macro_rules! esc {
    ($cmd:expr) => ((concat!("\x1b[", $cmd), concat!("\x1b[1;5", $cmd), concat!("\x1b[1;3", $cmd)));
    (~ $arg:expr) => ((concat!("\x1b[", $arg, "~"), concat!("\x1b[", $arg, ";5~"), concat!("\x1b[", $arg, ";3~")));
    (ss3 $cmd:expr) => ((concat!("\x1bO", $cmd), concat!("\x1bO5", $cmd), concat!("\x1bO3", $cmd)));
}

impl VTInput {
    pub fn input(&self, screen: &Screen, keymod: KeyMod, mut buffer: &mut [u8]) -> Result<usize, ()> {
        use Key::*;

        let (key, modifier) = keymod;

        let mode_newline = screen.mode().contains(VTMode::NEWLINE);
        let triple = match key {
            Return if mode_newline => ("\r\n", "\r\n", "\x1b\r\n"),
            Return    => ("\r", "\r", "\x1b\r"),
            Tab       => ("\t", "\t", "\t"),
            Backspace => ("\x7f", "\x7f", "\x7f"),
            Up        => esc!('A'),
            Down      => esc!('B'),
            Right     => esc!('C'),
            Left      => esc!('D'),
            PageUp    => esc!(~ '5'),
            PageDown  => esc!(~ '5'),
            Home      => esc!('H'),
            End       => esc!('F'),
            Insert    => esc!(~ '2'),
            Delete    => esc!(~ '3'),
            F(1)      => esc!(ss3 'P'),
            F(2)      => esc!(ss3 'Q'),
            F(3)      => esc!(ss3 'R'),
            F(4)      => esc!(ss3 'S'),
            F(5)      => esc!(~ "15"),
            F(6)      => esc!(~ "17"),    // WARN: Irregularity on purpose
            F(7)      => esc!(~ "18"),
            F(8)      => esc!(~ "19"),
            F(9)      => esc!(~ "20"),
            F(10)     => esc!(~ "21"),
            F(11)     => esc!(~ "23"),
            F(12)     => esc!(~ "24"),    // WARN: Ditto
            F(_)      => return Ok(0),
            Char(c) => {
                let mut utf8_buffer = [0u8; 8];

                let len = match modifier {
                    Modifier::Mod3 => {
                        // Write out an escaped character
                        buffer[0] = 0x1b;
                        c.encode_utf8(&mut utf8_buffer[1..]);
                        c.len_utf8() + 1
                    },
                    Modifier::Mod5 if c.is_ascii_alphabetic() => {
                        // Write out a control character
                        let c_num = c.to_ascii_uppercase() as u8;
                        utf8_buffer[0] = c_num - 0x40;
                        1
                    },
                    Modifier::Mod5 | Modifier::None => {
                        // Regular character, just write out its utf-8 representation
                        c.encode_utf8(utf8_buffer.as_mut());
                        c.len_utf8()
                    },
                };

                return buffer.write(&utf8_buffer[0..len]).map_err(|_| ());
            },
        };

        buffer.write(match modifier {
            Modifier::None => triple.0.as_bytes(),
            Modifier::Mod3 => triple.1.as_bytes(),
            Modifier::Mod5 => triple.2.as_bytes(),
        }).map_err(|_| ())
    }

    pub fn report_answer(&self, screen: &Screen, report: VTReport, mut buffer: &mut [u8]) -> Result<usize, ()> {
        use VTReport::*;

        let cursor_pos = screen.cursor_pos();
        let cursor_pos = format!("\x1b[{};{}R", cursor_pos.0, cursor_pos.1);
        buffer.write(match report {
            AnswerBack => b"TeePee",
            PrimaryAttrs => b"\x1b[?1;2c",
            SecondaryAttrs => b"\x1b>0;0;0c",    // TODO: version number?
            DeviceStatus => b"\x1b[0n",
            CursorPos => cursor_pos.as_bytes(),
            TermParams0 => b"\x1b[2;1;1;120;120;1;0;x",     // Made-up numbers
            TermParams1 => b"\x1b[3;1;1;120;120;1;0;x",     // Made-up numbers
        }).map_err(|_| ())
    }
}
