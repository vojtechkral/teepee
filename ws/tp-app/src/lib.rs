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

use pty::{Process, TermSize};
use term::{Term, InputData};


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


// impl vte::Perform for TermState {
//     fn print(&mut self, _: char) { unimplemented!() }
//     fn execute(&mut self, byte: u8) { unimplemented!() }
//     fn hook(&mut self, params: &[i64], intermediates: &[u8], ignore: bool) { unimplemented!() }
//     fn put(&mut self, byte: u8) { unimplemented!() }
//     fn unhook(&mut self) { unimplemented!() }
//     fn osc_dispatch(&mut self, params: &[&[u8]]) { unimplemented!() }
//     fn csi_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, _: char) { unimplemented!() }
//     fn esc_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: u8) { unimplemented!() }
// }


// #[derive(Debug)]
// pub enum Input<'a> {
//     Key(InputKey),
//     Str(&'a str),
// }

pub struct Session {
    ps: Process,
    buffer: Vec<u8>,
    pub term: Term,
}

impl Session {
    pub fn new(program: Command) -> Result<Session> {
        Ok(Session {
            ps: Process::new(program)?,
            // parser: vte::Parser::new(),
            buffer: vec![0; 4096],
            term: Term::new(),
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

    pub fn pk(&mut self) {    // XXX
        let mut buffer = vec![0; 4096];

        self.ps.resize(TermSize::new(25, 80)).unwrap();

        self.ps.write(b"\n").unwrap();
        self.ps.flush().unwrap();

        let mut i = 0;
        // loop {
        //     let avail = self.ps.bytes_available().unwrap();
        //     if avail > 0 {
        //         let actually_read = self.ps.read(&mut buffer).unwrap();
        //         io::stdout().write(&buffer[0..actually_read]).unwrap();
        //     } else if i == 50 {
        //         break;
        //     } else {
        //         thread::sleep_ms(50);
        //         i += 1;
        //     }
        // }
        while let Ok(read) = self.notify_read() {
            if i == 50 {
                break;
            } else {
                thread::sleep_ms(50);
                i += 1;
            }
        }
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
