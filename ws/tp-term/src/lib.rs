#[macro_use] extern crate bitflags;
extern crate smallvec;
extern crate unicode_width;

use std::mem;
use std::ops;

use smallvec::SmallVec;

pub mod utf8;
mod smallstring;
mod screen;
mod vt;
mod input;
pub use smallstring::*;
pub use screen::*;
pub use vt::*;
pub use input::*;


#[derive(Debug, Clone, Default)]
pub struct TermUpdate {
    // scrolled_up: usize,  // FIXME: move to screen
    screen_switched: bool,
    bell: bool,
    report_requests: SmallVec<[VTReport; 4]>,
}

#[derive(Debug)]
pub struct TermState {
    mode: VTMode,
    screen_current: VTScreenChoice,
    screen_primary: Screen,
    screen_alternate: Screen,
    // screen_choice_prev: VTScreenChoice,   // TODO: remove in favor of *Update
    update: TermUpdate,
    // TODO: scrollback
}

impl TermState {
    pub fn new() -> TermState {
        TermState {
            mode: VTMode::default(),
            screen_current: VTScreenChoice::default(),
            screen_primary: Screen::default(),
            screen_alternate: Screen::default(),
            update: TermUpdate::default(),
        }
    }

    pub fn reset_update(&mut self) -> TermUpdate {
        mem::replace(&mut self.update, TermUpdate::default())
    }

    pub fn screen_resize(&mut self, cols: u16, rows: u16) {
        self.screen_primary.resize(cols, rows);
        self.screen_alternate.resize(cols, rows);
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
        if (screen != self.screen_current) {
            self.update.screen_switched = true;
        }
        self.screen_current = screen;
    }

    fn set_mode(&mut self, mode: VTMode, enable: bool) {
        self.mode.set(mode, enable);
        // Also copy the mode to the screens for easier access
        self.screen_primary.set_mode(mode, enable);
        self.screen_alternate.set_mode(mode, enable);
    }

    fn report_request(&mut self, report: VTReport) {
        self.update.report_requests.push(report);
    }

    fn bell(&mut self) {
        self.update.bell = true;
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
