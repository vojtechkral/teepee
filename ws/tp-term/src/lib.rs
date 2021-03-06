#[macro_use] extern crate bitflags;
extern crate smallvec;
extern crate unicode_width;

use std::mem;
use std::ops;

use smallvec::{SmallVec, Drain};

pub mod utf8;
mod smallstring;
mod vt;
pub mod scrollback;
mod screen;
mod input;
pub use smallstring::*;
pub use vt::*;
pub use scrollback::MemScrollback;
pub use screen::*;
pub use input::*;


pub type ReportRequests = SmallVec<[VTReport; 4]>;

#[derive(Debug)]
pub struct TermState {
    mode: VTMode,
    screen_current: VTScreenChoice,
    screen_primary: Screen,
    screen_alternate: Screen,
    bell: bool,
    report_requests: ReportRequests,
}

impl TermState {
    pub fn new() -> TermState {
        let scrollback = MemScrollback::default();

        TermState {
            mode: VTMode::default(),
            screen_current: VTScreenChoice::default(),
            screen_primary: Screen::default().with_scrollback(scrollback),
            screen_alternate: Screen::default(),
            bell: false,
            report_requests: ReportRequests::new(),
        }
    }

    pub fn screen_resize(&mut self, cols: u16, rows: u16) {
        self.screen_primary.resize(cols, rows);
        self.screen_alternate.resize(cols, rows);
    }

    pub fn reset_bell(&mut self) -> bool {
        mem::replace(&mut self.bell, false)
    }

    pub fn reset_report_requests(&mut self) -> Drain<VTReport> {
        self.report_requests.drain()
    }
}

impl VTDispatch for TermState {
    type Screen = Screen;

    fn screen(&self) -> &Self::Screen {
        match self.screen_current {
            VTScreenChoice::Primary => &self.screen_primary,
            VTScreenChoice::Alternate => &self.screen_alternate,
        }
    }

    fn screen_mut(&mut self) -> &mut Self::Screen {
        match self.screen_current {
            VTScreenChoice::Primary => &mut self.screen_primary,
            VTScreenChoice::Alternate => &mut self.screen_alternate,
        }
    }

    fn screen_primary(&mut self) -> &mut Self::Screen { &mut self.screen_primary }
    fn screen_alternate(&mut self) -> &mut Self::Screen { &mut self.screen_alternate }

    fn switch_screen(&mut self, screen: VTScreenChoice) {
        if screen != self.screen_current {
            self.screen_current = screen;
            self.screen_mut().set_dirty();
        }
    }

    fn set_mode(&mut self, mode: VTMode, enable: bool) {
        self.mode.set(mode, enable);
        // Also copy the mode to the screens for easier access
        self.screen_primary.set_mode(mode, enable);
        self.screen_alternate.set_mode(mode, enable);
    }

    fn report_request(&mut self, report: VTReport) {
        self.report_requests.push(report);
    }

    fn bell(&mut self) {
        self.bell = true;
    }

    // TP extensions:
    // TODO
}

#[derive(Debug)]
pub struct Term {
    parser: VTParser,
    state: TermState,
    input: VTInput,
}

impl Term {
    pub fn new() -> Term {
        Term {
            parser: VTParser::new(),
            state: TermState::new(),
            input: VTInput,
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.parser.input(data, &mut self.state);
    }

    pub fn input(&self, input: InputData, buffer: &mut [u8]) -> Result<usize, ()> {
        self.input.input(input, self.mode, buffer)
    }

    pub fn report_answer(&self, report: VTReport, buffer: &mut [u8]) -> Result<usize, ()> {
        self.input.report_answer(self.screen(), report, buffer)
    }
}

impl ops::Deref for Term {
    type Target = TermState;

    fn deref(&self) -> &TermState {
        &self.state
    }
}

impl ops::DerefMut for Term {
    fn deref_mut(&mut self) -> &mut TermState {
        &mut self.state
    }
}



#[cfg(test)]
mod tests {
// XXX

#[test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

}
