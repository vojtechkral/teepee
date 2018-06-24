#[macro_use] extern crate glib;
extern crate glib_sys;   // XXX: needed?
extern crate gtk;
extern crate gdk;
extern crate gdk_sys;
extern crate gio;
extern crate cairo;

extern crate tp_app as tp;

mod giochannel;
mod termwidget;

use std::ops;
use std::env;
use std::f64::consts::PI;
use std::rc::Rc;
use std::cell::{RefCell, RefMut};
use std::thread::{self, ThreadId};
use std::process::Command;

use gio::prelude::*;
use gtk::prelude::*;

use giochannel::{IOChannel, IOCondition};
use termwidget::TermWidget;


#[derive(Debug)]
pub struct Shell {
    session: RefCell<tp::Session>,
    iochannel: IOChannel,
    term_widget: TermWidget,
    main_widget: gtk::Widget,
}

/// This is a bit of a hack to avoid having to make Shell Send & Sync.
/// GTK IO callbacks _may_ in some situations run on another thread, but AFAIK that only happens
/// on Windows in a specific setting and so we rely on (and ensure at runtime) that the notification
/// will in fact come up on the main thread and so locking/synchronization can be avoided.
struct ShellCell {
    shell: Rc<Shell>,
    thread_id: ThreadId,
}

unsafe impl Send for ShellCell {}

impl ShellCell {
    fn new(shell: &Rc<Shell>) -> ShellCell {
        ShellCell {
            shell: shell.clone(),
            thread_id: thread::current().id(),
        }
    }
}

impl ops::Deref for ShellCell {
    type Target = Shell;

    fn deref(&self) -> &Shell {
        assert_eq!(self.thread_id, thread::current().id(), "Attempt to share ShellCell between threads");
        &*self.shell
    }
}

impl Shell {
    pub fn new() -> Rc<Shell> {
        let session = tp::Session::new(Command::new("bash")).unwrap();    // XXX
        let iochannel = IOChannel::new(&session);
        let session = RefCell::new(session);
        let term_widget = TermWidget::new();
        let main_widget = term_widget.draw_area().clone().upcast::<gtk::Widget>();

        let shell = Rc::new(Shell {
            session,
            iochannel,
            term_widget,
            main_widget,
        });

        let shell_ = shell.clone();
        shell.term_widget.connect_draw(move |cairo| {
            shell_.term_widget.render(cairo, &mut *shell_.session.borrow_mut());
        });

        let shell_cell = ShellCell::new(&shell);
        shell.term_widget.connect_input(move |input| {
            let shell = &*shell_cell;
            let mut session = shell.session.borrow_mut();
            let res = session.input(input);
            if res.is_err() {
                // XXX: error handling
            }
        });

        let shell_ = shell.clone();
        shell.term_widget.connect_resize(move || {
            let (cols, rows) = shell_.term_widget.screen_size();
            shell_.session.borrow_mut().screen_resize(cols, rows).unwrap();    // XXX: error handling
        });

        let shell_cell = ShellCell::new(&shell);
        shell.iochannel.add_watch(IOCondition::IN, move |_, _| {
            let shell = &*shell_cell;
            let mut session = shell.session.borrow_mut();
            let res = session.notify_read();
            if res.is_ok() {
                shell.term_widget.queue_draw();
                Continue(true)
            } else {
                // XXX: error handling
                Continue(false)
            }
        });

        shell
    }

    pub fn main_widget(&self) -> &gtk::Widget {
        &self.main_widget
    }

    pub fn grab_focus(&self) {
        self.term_widget.grab_focus();
    }
}


fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("Hello, World!");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(1000, 500);

    let window_ = window.clone();
    window.connect_delete_event(move |_, _| {
        window_.destroy();
        Inhibit(false)
    });

    let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

    let label = gtk::Label::new(None);
    label.set_text("Hello, World!");
    container.add(&label);

    let shell = Shell::new();
    window.add(shell.main_widget());

    window.show_all();

    shell.grab_focus();
}

fn main() {
    // let mut session = tp_lib::Session::new(Command::new("bash")).unwrap();
    // println!("session: {:?}", session);
    // session.pk();

    let application = gtk::Application::new("hk.kral.teepee", gio::ApplicationFlags::empty()).unwrap();
    application.connect_startup(|app| build_ui(app));
    application.connect_activate(|_| {});
    application.run(&env::args().collect::<Vec<_>>());
}
