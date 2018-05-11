//! A streaming (byte-by-byte) UTF-8 parser

use std::mem;


#[derive(Debug, Clone, Copy)]
enum State {
    /// The default state
    Ground,
    /// Expect continuation bytes.
    /// First number is bytes expected originally, second one is bytes to go.
    Continues(u8, u8),
}

use self::State::*;

pub const REPLACE_CHAR: char = '\u{FFFD}';

/// A UTF-8 streaming parser.
#[derive(Debug)]
pub struct Parser {
    state: State,
    char_wip: u32,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            state: Ground,
            char_wip: 0,
        }
    }

    fn expect(&mut self, data: u8, conts: u8) {
        self.char_wip = data as u32;
        self.state = Continues(conts, conts);
    }

    fn ground(&mut self, byte: u8) -> Option<Result<char, ()>> {
        match byte {
            0x00 ... 0x7f => Some(Ok(char::from(byte))),      // ASCII char
            0x80 ... 0xbf => Some(Err(())),                   // A bogus continuation byte
            0xc0 ... 0xdf => { self.expect(byte & 0x1f, 1); None }   // 1 continuation byte
            // ^ Note: We could discard 0xc0 & 0xc1 here as they are invalid, but it would produce extra errors
            0xe0 ... 0xef => { self.expect(byte & 0xf,  2); None }   // 2 continuation bytes
            0xf0 ... 0xf4 => { self.expect(byte & 0x7,  3); None }   // 3 continuation bytes
            _             => Some(Err(())),                   // Other values are not valid
        }
    }

    fn push_cont(&mut self, byte: u8) {
        self.char_wip <<= 6;
        self.char_wip |= byte as u32 & 0x3f;
    }

    fn continues(&mut self, byte: u8, orig: u8, mut to_go: u8) -> Option<Result<char, ()>> {
        to_go -= 1;
        if to_go > 0 {
            match byte {
                0x80 ... 0xbf => {
                    self.push_cont(byte);
                    self.state = Continues(orig, to_go);
                    None
                },
                _ => {
                    // Expected continuation byte, got something else
                    self.state = Ground;
                    Some(Err(()))
                },
            }
        } else {
            // If we're here, we're supposed to have received the last expected continuation byte.
            // Lets check that and try to form a charater out of what we've got so far.

            // Next state will be Ground no matter what
            self.state = Ground;

            if byte & 0xc0 != 0x80 {
                // Not a continuation byte
                return Some(Err(()));
            }

            // Got a continuation byte
            self.push_cont(byte);
            let c = match (orig, self.char_wip) {
                (1, c @ 0x80    ... 0x7ff)    => c,
                (2, c @ 0x800   ... 0xffff)   => c,
                (3, c @ 0x10000 ... 0x10ffff) => c,
                _ => {
                    // Overlong sequences and other invalid values fall through to here
                    return Some(Err(()));
                },
            };

            // Since we already did the bounds checking, this should be ok:
            Some(Ok(unsafe { ::std::char::from_u32_unchecked(c) }))
        }
    }

    pub fn input(&mut self, byte: u8) -> Option<Result<char, ()>> {
        match self.state {
            Ground => self.ground(byte),
            Continues(co, cg) => self.continues(byte, co, cg),
        }
    }

    /// Resets the state of the parser.
    /// Result indicates if there was an incomplete character.
    pub fn reset(&mut self) -> Result<(), ()> {
        let prev_state = mem::replace(&mut self.state, Ground);
        match prev_state {
            Ground => Ok(()),
            Continues(_, _) => Err(()),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<String, ()> {
        let mut parser = Parser::new();
        let mut res = String::with_capacity(bytes.len());

        for b in bytes {
            if let Some(c) = parser.input(*b) {
                res.push(c?);
            }
        }

        res.shrink_to_fit();
        Ok(res)
    }

    pub fn parse_lossy(bytes: &[u8]) -> String {
        let mut parser = Parser::new();
        let mut res = String::with_capacity(bytes.len());

        for c in bytes.iter().filter_map(|b| parser.input(*b).map(|r| r.unwrap_or(REPLACE_CHAR))) {
            res.push(c);
        }

        res.shrink_to_fit();
        res
    }
}




#[cfg(test)]
mod tests {

use std::iter::FromIterator;
use super::*;

#[test]
fn utf8_smoke() {
    assert_eq!(Parser::parse_lossy(b"Ahoj, sv\xc4\x9bte!"), "Ahoj, světe!", "Hello, World!");
    assert_eq!(Parser::parse_lossy(
        b"\xf0\x9d\x94\x98\xf0\x9d\x94\xab\xf0\x9d\x94\xa6\xf0\x9d\x94\xa0\xf0\x9d\x94\xac\xf0\x9d\x94\xa1\xf0\x9d\x94\xa2"),
        "𝔘𝔫𝔦𝔠𝔬𝔡𝔢", "\"Unicode\" in Mathematical Fraktur"
    );
}

#[test]
fn utf8_boundary() {
    assert_eq!(Parser::parse_lossy(b"\x00"), "\u{00}", "1-byte lower boundary");
    assert_eq!(Parser::parse_lossy(b"\x7f"), "\u{7f}", "1-byte upper boundary");
    assert_eq!(Parser::parse_lossy(b"\xc2\x80"), "\u{80}", "2-byte lower boundary");
    assert_eq!(Parser::parse_lossy(b"\xdf\xbf"), "\u{7ff}", "2-byte upper boundary");
    assert_eq!(Parser::parse_lossy(b"\xe8\x80\x80"), "耀", "3-byte lower boundary");
    assert_eq!(Parser::parse_lossy(b"\xef\xbf\xbf"), "\u{ffff}", "3-byte upper boundary");
    assert_eq!(Parser::parse_lossy(b"\xf0\x90\x80\x80"), "\u{10000}", "4-byte lower boundary");
    assert_eq!(Parser::parse_lossy(b"\xf4\x8f\xbf\xbf"), "\u{10ffff}", "4-byte upper boundary");
}

#[test]
fn utf8_overlongs() {
    assert!(Parser::parse(b"\xc1\xbf").is_err(), "2-byte overlong");
    assert!(Parser::parse(b"\xe0\x80\xbf").is_err(), "3-byte overlong");
    assert!(Parser::parse(b"\xe0\x9f\xbf").is_err(), "3-byte overlong");
    assert!(Parser::parse(b"\xf0\x80\x81\xbf").is_err(), "4-byte overlong");
    assert!(Parser::parse(b"\xf0\x80\x9f\xbf").is_err(), "4-byte overlong");
    assert!(Parser::parse(b"\xf0\x8f\xbf\xbf").is_err(), "4-byte overlong");
}

#[test]
fn utf8_continuations() {
    assert_eq!(Parser::parse_lossy(b"TE\xa0ST"), "TE�ST", "Unexpected continuation byte");
    assert_eq!(Parser::parse_lossy(b"\xc4TEST"), "�EST", "Insufficient continuation bytes - 2-bytes - 1/2");
    assert_eq!(Parser::parse_lossy(b"\xe2TEST"), "�EST", "Insufficient continuation bytes - 3-bytes - 1/3");
    assert_eq!(Parser::parse_lossy(b"\xe2\xb0TEST"), "�EST", "Insufficient continuation bytes - 3-bytes - 2/3");
    assert_eq!(Parser::parse_lossy(b"\xf0TEST"), "�EST", "Insufficient continuation bytes - 4-bytes - 1/4");
    assert_eq!(Parser::parse_lossy(b"\xf0\x9dTEST"), "�EST", "Insufficient continuation bytes - 4-bytes - 2/4");
    assert_eq!(Parser::parse_lossy(b"\xf0\x9d\x94TEST"), "�EST", "Insufficient continuation bytes - 4-bytes - 3/4");
}

#[test]
fn utf8_out_of_range() {
    assert_eq!(Parser::parse_lossy(b"\xF4\x90\x80\x80TEST"), "�TEST", "Code point over U+10FFFF");
    assert_eq!(Parser::parse_lossy(b"\xF9\x80\x80\x80\x81TEST"), "�����TEST", "5-byte utf-8");
    assert_eq!(Parser::parse_lossy(b"\xFD\x80\x80\x80\x80\x81TEST"), "������TEST", "6-byte utf-8");
}

#[test]
fn utf8_parser_reset() {
    let mut parser = Parser::new();
    parser.input(b'\xc2'); // Expects a continuation byte
    parser.reset();
    assert_eq!(parser.input(b'a'), Some(Ok('a')), "Parser reset");
}


}
