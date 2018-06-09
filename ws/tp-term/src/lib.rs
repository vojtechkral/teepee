#[macro_use] extern crate bitflags;
extern crate smallvec;
extern crate unicode_width;
#[macro_use] extern crate error_chain;

use std::ops;

pub mod utf8;
mod smallstring;
mod screen;
mod vt;
pub use smallstring::*;
pub use screen::*;
pub use vt::*;


mod err {
    error_chain! {
    }
}
pub use err::*;


#[derive(Debug)]
pub struct TermState {
    screen_current: VTScreenChoice,
    screen_primary: Screen,
    screen_alternate: Screen,
    screen_choice_prev: VTScreenChoice,
    // TODO: scrollback, request queue
}

impl TermState {
    pub fn new() -> TermState {
        TermState {
            screen_current: VTScreenChoice::default(),
            screen_primary: Screen::default(),
            screen_alternate: Screen::default(),
            screen_choice_prev: VTScreenChoice::default(),
        }
    }

    pub fn screen_switched(&mut self) -> bool {
        let res = self.screen_choice_prev != self.screen_current;
        self.screen_choice_prev = self.screen_current;
        res
    }

    pub fn screen_resize(&mut self, cols: u32, rows: u32) {
        self.screen_primary.resize(cols, rows);
        self.screen_alternate.resize(cols, rows);
    }
}

impl VTDispatch for TermState {
    type Screen = Screen;

    fn screen(&mut self) -> &mut Self::Screen {
        match self.screen_current {
            VTScreenChoice::Primary => &mut self.screen_primary,
            VTScreenChoice::Alternate => &mut self.screen_alternate,
        }
    }

    fn screen_primary(&mut self) -> &mut Self::Screen { &mut self.screen_primary }
    fn screen_alternate(&mut self) -> &mut Self::Screen { &mut self.screen_alternate }

    fn switch_screen(&mut self, screen: VTScreenChoice) {
        self.screen_current = screen;
    }

    fn set_mode(&mut self, mode: VTMode, enable: bool) {
        // TODO: set mode on both screens
        self.screen_primary.set_mode(mode, enable);
        self.screen_alternate.set_mode(mode, enable);
    }

    fn report_request(&mut self, report: VTReport) {
        // TODO: push an event, same with report requests
        unimplemented!()
    }

    // TP extensions:
    // TODO
}

#[derive(Debug)]
pub struct Term {
    parser: VTParser,
    state: TermState,
}

impl Term {
    pub fn new() -> Term {
        Term {
            parser: VTParser::new(),
            state: TermState::new(),
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.parser.input(data, &mut self.state);
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
