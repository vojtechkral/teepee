// extern crate vte;   // TODO: remove
#[macro_use] extern crate error_chain;

extern crate tp_pty;
extern crate tp_term;

use std::fmt;
use std::io::{self, Read, Write};
use std::thread;
use std::process::Command;
use std::os::unix::io::{RawFd, AsRawFd};

pub mod term { pub use tp_term::*; }
pub mod pty { pub use tp_pty::*; }

use pty::Process;
use term::{Term, InputData};

mod colors;
pub use colors::*;


mod err {
    error_chain! {
        links {
            Pty(::tp_pty::Error, ::tp_pty::ErrorKind);
        }

        foreign_links {
            Io(::std::io::Error);
        }
    }
}
pub use err::*;


pub struct Session {
    ps: Process,
    buffer: Vec<u8>,
    pub term: Term,
    pub colors: ColorScheme,
}

impl Session {
    pub fn new(program: Command) -> Result<Session> {
        Ok(Session {
            ps: Process::new(program)?,
            buffer: vec![0; 4096],
            term: Term::new(),
            colors: ColorScheme::default(),
        })
    }

    pub fn notify_read(&mut self) -> Result<usize> {
        let avail = self.ps.bytes_available()?;
        if avail > 0 {
            let actually_read = self.ps.read(&mut self.buffer)?;
            if actually_read > 0 {
                self.term.write(&self.buffer[0..actually_read]);
            }
            Ok(actually_read)
        } else {
            Ok(0)
        }
    }

    pub fn input(&mut self, input: InputData) -> Result<usize> {
        if let InputData::Str(string) = input {
            self.ps.write(string.as_bytes())
        } else {
            let size = self.term.input(input, &mut self.buffer).expect("Input buffer not large enough");
            self.ps.write(&self.buffer[0..size])
        }.map_err(io::Error::into)
    }

    pub fn screen_resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.ps.set_winsize(cols, rows)?;
        self.term.screen_resize(cols, rows);
        Ok(())
    }
}

impl AsRawFd for Session {
    fn as_raw_fd(&self) -> RawFd {
        self.ps.as_raw_fd()
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Term")
           .field("ps", &self.ps)
           .field("parser", &"vte::Parser { ... }")
           .field("buffer", &format!("[_; {}]", self.buffer.len()))
           .finish()
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
