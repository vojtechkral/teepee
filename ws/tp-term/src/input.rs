use std::io::{self, Write};

use ::vt::*;
use ::screen::Screen;


/// Input Key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
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
}

bitflags! {
    pub struct Modifier: u8 {
        const NONE    = 0;
        const SHIFT   = 1 << 0;
        const ALT     = 1 << 1;
        const CONTROL = 1 << 2;
    }
}

impl Default for Modifier {
    fn default() -> Modifier {
        Modifier::NONE
    }
}

impl Modifier {
    pub fn is_none(&self) -> bool {
        self.bits == 0
    }

    pub fn escape_arg(&self) -> u8 {
        if self.bits == 0 {
            0x30
        } else {
            self.bits + 1 + 0x30     // +1 for VT encoding, +0x30 to make ascii number char
        }
    }

    pub fn encode_into(&self, separate: bool, buffer: &mut &mut [u8]) -> io::Result<usize> {
        let arg = self.escape_arg();

        if self.is_none() {
            return Ok(0);
        }

        if separate {
            buffer.write(b";")?;
            buffer.write(&[arg])?;
            Ok(2)
        } else {
            buffer.write(&[arg])
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputData<'a> {
    Key(Key, Modifier),
    FKey(u8, Modifier),
    Char(char, Modifier),
    Str(&'a str),
    Empty,
}

#[derive(Debug)]
pub struct VTInput;

impl VTInput {
    fn input_esc(byte: u8, modifier: Modifier, mut buffer: &mut [u8]) -> io::Result<usize> {
        if modifier.contains(Modifier::ALT) {
            buffer.write(b"\x1b")?;
            buffer.write(&[byte])?;
            Ok(2)
        } else {
            buffer.write(&[byte])
        }
    }

    fn input_ss3(cmd: u8, modifier: Modifier, mut buffer: &mut [u8]) -> io::Result<usize> {
        let mut size = buffer.write(b"\x1bO")?;
        size += modifier.encode_into(false, &mut buffer)?;
        size += buffer.write(&[cmd])?;
        Ok(size)
    }

    fn input_csi(cmd: u8, arg: Option<&[u8]>, modifier: Modifier, mut buffer: &mut [u8]) -> io::Result<usize> {
        let mut size = buffer.write(b"\x1b[")?;

        match arg {
            Some(arg) => {
                size += buffer.write(arg)?;
                size += modifier.encode_into(true, &mut buffer)?;
            },
            None => size += modifier.encode_into(false, &mut buffer)?,
        }

        size += buffer.write(&[cmd])?;
        Ok(size)
    }

    fn input_key(key: Key, modifier: Modifier, mode: VTMode, mut buffer: &mut [u8]) -> io::Result<usize> {
        use Key::*;

        let mode_nl = mode.contains(VTMode::NEWLINE);
        let mode_appkeys = mode.contains(VTMode::APP_CURSOR_KEYS);

        match key {
            Return if mode_nl && modifier.is_none() => buffer.write(b"\r\n"),

            Up     if mode_appkeys => Self::input_ss3(b'A', modifier, buffer),
            Down   if mode_appkeys => Self::input_ss3(b'B', modifier, buffer),
            Right  if mode_appkeys => Self::input_ss3(b'C', modifier, buffer),
            Left   if mode_appkeys => Self::input_ss3(b'D', modifier, buffer),

            Return    => Self::input_esc(b'\r', modifier, buffer),
            Tab       => Self::input_esc(b'\t', modifier, buffer),
            Backspace => Self::input_esc(0x7f, modifier, buffer),
            Up        => Self::input_csi(b'A', None, modifier, buffer),
            Down      => Self::input_csi(b'B', None, modifier, buffer),
            Right     => Self::input_csi(b'C', None, modifier, buffer),
            Left      => Self::input_csi(b'D', None, modifier, buffer),
            PageUp    => Self::input_csi(b'~', Some(b"5"), modifier, buffer),
            PageDown  => Self::input_csi(b'~', Some(b"6"), modifier, buffer),
            Home      => Self::input_csi(b'H', None, modifier, buffer),
            End       => Self::input_csi(b'F', None, modifier, buffer),
            Insert    => Self::input_csi(b'~', Some(b"2"), modifier, buffer),
            Delete    => Self::input_csi(b'~', Some(b"3"), modifier, buffer),
        }
    }

    fn input_fkey(fkey: u8, modifier: Modifier, mut buffer: &mut [u8]) -> io::Result<usize> {
        match fkey {
            1  => Self::input_ss3(b'P', modifier, buffer),
            2  => Self::input_ss3(b'Q', modifier, buffer),
            3  => Self::input_ss3(b'R', modifier, buffer),
            4  => Self::input_ss3(b'S', modifier, buffer),
            5  => Self::input_csi(b'~', Some(b"15"), modifier, buffer),
            6  => Self::input_csi(b'~', Some(b"17"), modifier, buffer),
            7  => Self::input_csi(b'~', Some(b"18"), modifier, buffer),
            8  => Self::input_csi(b'~', Some(b"19"), modifier, buffer),
            9  => Self::input_csi(b'~', Some(b"20"), modifier, buffer),
            10 => Self::input_csi(b'~', Some(b"21"), modifier, buffer),
            11 => Self::input_csi(b'~', Some(b"23"), modifier, buffer),
            12 => Self::input_csi(b'~', Some(b"24"), modifier, buffer),
            13 => Self::input_csi(b'~', Some(b"25"), modifier, buffer),
            14 => Self::input_csi(b'~', Some(b"26"), modifier, buffer),
            15 => Self::input_csi(b'~', Some(b"28"), modifier, buffer),
            16 => Self::input_csi(b'~', Some(b"29"), modifier, buffer),
            17 => Self::input_csi(b'~', Some(b"31"), modifier, buffer),
            18 => Self::input_csi(b'~', Some(b"32"), modifier, buffer),
            19 => Self::input_csi(b'~', Some(b"33"), modifier, buffer),
            20 => Self::input_csi(b'~', Some(b"34"), modifier, buffer),
            _  => Ok(0),
        }
    }

    fn input_ascii(ch: u8, modifier: Modifier, mut buffer: &mut [u8]) -> io::Result<usize> {
        let alt = modifier.contains(Modifier::ALT);
        let control = modifier.contains(Modifier::CONTROL);
        let mut size = 0;

        if alt {
            size += buffer.write(b"\x1b")?;
        }

        size += match ch {
            0x40 ... 0x5f if control => buffer.write(&[ch - 0x40])?,
            0x60 ... 0x7f if control => buffer.write(&[ch - 0x60])?,
            _ => buffer.write(&[ch])?,
        };

        Ok(size)
    }

    fn input_char(ch: char, modifier: Modifier, mut buffer: &mut [u8]) -> Result<usize, ()> {
        let ch = if ch <= '\x7f' {
            ch as u8
        } else {
            return if ch.len_utf8() <= buffer.len() {
                Ok(ch.encode_utf8(buffer).len())
            } else {
                Err(())
            };
        };

        let alt = modifier.contains(Modifier::ALT);
        let control = modifier.contains(Modifier::CONTROL);
        let mut size = 0;

        if alt {
            size += buffer.write(b"\x1b").map_err(|_| ())?;
        }

        size += match ch {
            0x40 ... 0x5f if control => buffer.write(&[ch - 0x40]),
            0x60 ... 0x7f if control => buffer.write(&[ch - 0x60]),
            _ => buffer.write(&[ch]),
        }.map_err(|_| ())?;

        Ok(size)
    }

    pub fn input(&self, input: InputData, mode: VTMode, mut buffer: &mut [u8]) -> Result<usize, ()> {
        use InputData::*;

        match input {
            Key(key, modifier) => Self::input_key(key, modifier, mode, buffer).map_err(|_| ()),
            FKey(num, modifier) => Self::input_fkey(num, modifier, buffer).map_err(|_| ()),
            Char(ch, modifier) => Self::input_char(ch, modifier, buffer),
            Str(s) => buffer.write(s.as_bytes()).map_err(|_| ()),
            Empty => Ok(0),
        }
    }

    pub fn report_answer(&self, screen: &Screen, report: VTReport, mut buffer: &mut [u8]) -> Result<usize, ()> {
        use VTReport::*;

        let cursor = screen.cursor();
        let cursor = format!("\x1b[{};{}R", cursor.0, cursor.1);
        buffer.write(match report {
            AnswerBack => b"TeePee",
            PrimaryAttrs => b"\x1b[?1;2c",
            SecondaryAttrs => b"\x1b>0;0;0c",    // TODO: version number?
            DeviceStatus => b"\x1b[0n",
            CursorPos => cursor.as_bytes(),
            TermParams0 => b"\x1b[2;1;1;120;120;1;0;x",     // Made-up numbers
            TermParams1 => b"\x1b[3;1;1;120;120;1;0;x",     // Made-up numbers
        }).map_err(|_| ())
    }
}
