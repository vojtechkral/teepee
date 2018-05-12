// extern crate vte;   // TODO: remove
#[macro_use] extern crate error_chain;

extern crate tp_pty;
extern crate tp_term;

use std::fmt;
use std::io::{self, Read, Write};
use std::thread;
use std::process::Command;

use tp_pty::{Process, TermSize};
use tp_term::Term;


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


pub struct Session {
    ps: Process,
    // parser: vte::Parser,
    buffer: Vec<u8>,
    term: Term,
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

    pub fn pk(&mut self) {
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
        while let Ok(read) = self.read_ready() {
            if i == 50 {
                break;
            } else {
                thread::sleep_ms(50);
                i += 1;
            }
        }
    }

    pub fn read_ready(&mut self) -> Result<usize> {
        // TODO: TermState might as well accept Reads
        let avail = self.ps.bytes_available()?;
        if avail > 0 {
            let actually_read = self.ps.read(&mut self.buffer)?;
            // for b in &self.buffer[0..actually_read] {
            //     // self.parser.advance(&mut self.state, *b);
            // }
            self.term.input(&self.buffer[0..actually_read]);
            Ok(actually_read)
        } else {
            Ok(0)
        }
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
