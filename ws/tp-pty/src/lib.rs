extern crate libc;
// #[macro_use] extern crate nix;   // TODO: remove?
#[macro_use] extern crate error_chain;

use std::{mem, ptr, io};
use std::fs::File;
use std::process::{Command, Child, Stdio};
use std::os::unix::process::CommandExt;
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd};
// use nix::pty;


#[allow(unused_macros)]
macro_rules! try_c {
	($what:expr) => (
		match unsafe { $what } {
			e if e < 0 => return Err(::std::io::Error::last_os_error().into()),
			res => res,
		}
	)
}

#[allow(unused_macros)]
macro_rules! try_intr {
	($what:expr) => (
		loop {
			match unsafe { $what } {
				e if e < 0 => {
					let err = ::std::io::Error::last_os_error();
					match err.kind() {
						::std::io::ErrorKind::Interrupted => continue,
						_ => return Err(err.into()),
					}
				},
				res => break res,
			}
		}
	)
}


mod err {
    error_chain! {
        foreign_links {
            // Nix(::nix::Error);
            Io(::std::io::Error);
        }
    }
}
pub use err::*;


fn make_controlling_tty(fd: RawFd) -> io::Result<()> {
    try_c!(libc::ioctl(fd, libc::TIOCSCTTY as _, 0));
    Ok(())
}

// pub struct TermSize {
//     rows: u16,
//     cols: u16,
// }

// impl TermSize {
//     pub fn new(rows: u16, cols: u16) -> TermSize { TermSize { rows, cols } }
// }

// impl From<libc::winsize> for TermSize {
//     fn from(size: libc::winsize) -> TermSize {
//         TermSize {
//             rows: size.ws_row,
//             cols: size.ws_col,
//         }
//     }
// }

// impl From<TermSize> for libc::winsize {
//     fn from(size: TermSize) -> libc::winsize {
//         libc::winsize {
//             ws_row: size.rows,
//             ws_col: size.cols,
//             ws_xpixel: 0,
//             ws_ypixel: 0,
//         }
//     }
// }

#[derive(Debug)]
pub struct Process {
    fd: RawFd,
    child: Child,
}

impl Process {
    // pub fn new() -> Result<Process> {
    pub fn new(mut program: Command) -> Result<Process> {
        if ! cfg!(target_os = "linux") {
            unimplemented!();
        }

        let mut master: RawFd = 0;
        let mut slave: RawFd = 0;
        try_c!(libc::openpty(&mut master, &mut slave, ptr::null_mut(), ptr::null(), ptr::null()));
        let (master, slave) = (master, slave);

        println!("PTY open: ({}, {})", master, slave);

        // let mut process = Command::new("/usr/bin/bash");
        // let mut process = Command::new(program);

        // if let Some(args) = args {
        //     process.args(args);
        // }

        // if let Some(startdir) = startdir {
        //     process.current_dir(startdir);
        // }

        program.env("TERM", "xterm-256color");

        unsafe {
            program.stdin(Stdio::from_raw_fd(slave));
            program.stderr(Stdio::from_raw_fd(slave));
            program.stdout(Stdio::from_raw_fd(slave));
        }

        program.before_exec(move || {
            try_c!(libc::setsid());
            make_controlling_tty(slave)?;

            unsafe {
                libc::close(master);
                libc::close(slave);
            }

            Ok(())
        });

        let child = program.spawn()?;

        Ok(Process { fd: master, child })
    }

    pub fn bytes_available(&self) -> io::Result<usize> {
        let mut res = 0usize;
        try_c!(libc::ioctl(self.fd, libc::TIOCINQ as _, &mut res));
        // TODO: On BSD & OS X that would be TIOCOUTQ
        Ok(res)
    }

    // pub fn resize<T: Into<TermSize>>(&mut self, size: T) -> io::Result<()> {
    //     let size: libc::winsize = size.into().into();
    //     try_c!(libc::ioctl(self.fd, libc::TIOCSWINSZ as _, &size));
    //     Ok(())
    // }
    pub fn set_winsize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        let winsize = libc::winsize {
            ws_row: rows as _,
            ws_col: cols as _,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        try_c!(libc::ioctl(self.fd, libc::TIOCSWINSZ as _, &winsize));
        Ok(())
    }
}

impl io::Read for Process {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut file = unsafe { File::from_raw_fd(self.fd) };
        let res = file.read(buf);
        mem::forget(file);
        res
    }
}

impl io::Write for Process {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = unsafe { File::from_raw_fd(self.fd) };
        let res = file.write(buf);
        mem::forget(file);
        res
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = unsafe { File::from_raw_fd(self.fd) };
        let res = file.flush();
        mem::forget(file);
        res
    }
}

impl AsRawFd for Process {
    fn as_raw_fd(&self) -> RawFd { self.fd }
}



#[cfg(test)]
mod tests {

use super::*;

use std::io::{Read, Write};
use std::process::Command;


fn make_test_process() -> Process {
    Process::new(Command::new("sh")).unwrap()
}

#[test]
fn process_create() {
    make_test_process();
}

#[test]
fn process_io() {
    let mut ps = make_test_process();
    let mut byte = [0u8];

    ps.write(b"\n").unwrap();
    ps.flush().unwrap();
    ps.bytes_available().unwrap();
    ps.resize(TermSize::new(25, 80)).unwrap();
    ps.read(&mut byte).unwrap();    // Surely there's at least 1 byte to read ...
}

fn termsize() {
    let mut ps = make_test_process();
    ps.resize(TermSize::new(25, 80)).unwrap();
}

}
