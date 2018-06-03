// #[macro_use] extern crate nom;
#[macro_use] extern crate bitflags;
extern crate smallvec;
// extern crate unicode_normalization;
extern crate unicode_width;
#[macro_use] extern crate error_chain;


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
struct TermState {
    // TODO: two screens, history, ...
    screen_primary: Screen,
}

impl TermState {
    pub fn new() -> TermState {
        TermState {
            screen_primary: Screen::new(),
        }
    }   // TODO: use default?
}

impl VTDispatch for TermState {
    type Screen = Screen;

    fn screen(&mut self) -> &mut Self::Screen {
        // TODO: screen switching
        &mut self.screen_primary
    }

    fn set_mode(&mut self, mode: VTMode, enable: bool) {
        // TODO: set mode on both screens
        unimplemented!()
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

    pub fn input(&mut self, data: &[u8]) {
        self.parser.input(data, &mut self.state);
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
