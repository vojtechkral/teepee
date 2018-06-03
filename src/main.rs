#[macro_use] extern crate glib;
extern crate glib_sys;
extern crate gtk;
extern crate gio;
extern crate cairo;

extern crate tp_lib as tp;

mod giochannel;

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


#[derive(Debug)]
pub struct TermWidget {
    draw_area: gtk::DrawingArea,
}

impl TermWidget {
    pub fn new() -> TermWidget {
        // let draw_area = gtk::DrawingArea::new();

        // let widget = Rc::new(TermWidget {
        //     draw_area
        // });

        // let widget_ = widget.clone();
        // widget.draw_area.connect_draw(move |_, cr| {
        //     widget_.redraw(cr);
        //     Inhibit(false)
        // });

        // widget

        TermWidget {
            draw_area: gtk::DrawingArea::new()
        }
    }

    fn redraw(&self, cr: &cairo::Context, session: &tp::Session) {
        cr.move_to(0.0, 20.0);
        cr.show_text("Hokus pokus");
    }

    // pub fn connect_draw<T>(container: Rc<T>) where T: 'static + AsRef<TermWidget> + AsRef<tp::Session> {
    //     let term_widget: &TermWidget = (*container).as_ref();

    //     let container_ = container.clone();
    //     term_widget.draw_area.connect_draw(move |_, cr| {
    //         let term_widget: &TermWidget = (*container_).as_ref();
    //         let session: &tp::Session = (*container_).as_ref();

    //         term_widget.redraw(cr, session);

    //         Inhibit(false)
    //     });
    // }

    fn connect_draw<F>(&self, func: F)
    where F: Fn(&gtk::DrawingArea, &cairo::Context) -> Inhibit + 'static {
        self.draw_area.connect_draw(func);
    }

    pub fn queue_draw(&self) {
        self.draw_area.queue_draw();
    }

    pub fn draw_area(&self) -> &gtk::DrawingArea {
        &self.draw_area
    }
}

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
        shell.term_widget.connect_draw(move |_, cr| {
            shell_.term_widget.redraw(cr, &*shell_.session.borrow());
            Inhibit(false)
        });

        let shell_cell = ShellCell::new(&shell);
        shell.iochannel.add_watch(IOCondition::IN, move |_, _| {
            let shell = &*shell_cell;
            let mut session = shell.session.borrow_mut();
            let res = session.notify_read();
            if (res.is_ok()) {
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
}


fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("Hello, World!");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(500, 500);

    let window_ = window.clone();
    window.connect_delete_event(move |_, _| {
        window_.destroy();
        Inhibit(false)
    });

    let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

    // let time = current_time();
    let label = gtk::Label::new(None);
    label.set_text("Hello, World!");
    container.add(&label);

    // let drawing_area = gtk::DrawingArea::new();
    // // drawing_area.set_default_size(400, 400);
    // drawing_area.set_vexpand(true);
    // drawing_area.connect_draw(|_, cr| {
    //     cr.set_dash(&[3., 2., 1.], 1.);
    //     assert_eq!(cr.get_dash(), (vec![3., 2., 1.], 1.));

    //         cr.scale(500f64, 500f64);

    //         cr.set_source_rgb(250.0/255.0, 224.0/255.0, 55.0/255.0);
    //         cr.paint();

    //         cr.set_line_width(0.05);

    //         // border
    //         cr.set_source_rgb(0.3, 0.3, 0.3);
    //         cr.rectangle(0.0, 0.0, 1.0, 1.0);
    //         cr.stroke();

    //         cr.set_line_width(0.03);

    //         // draw circle
    //         cr.arc(0.5, 0.5, 0.4, 0.0, PI * 2.);
    //         cr.stroke();


    //         // mouth
    //         let mouth_top = 0.68;
    //         let mouth_width = 0.38;

    //         let mouth_dx = 0.10;
    //         let mouth_dy = 0.10;

    //         cr.move_to( 0.50 - mouth_width/2.0, mouth_top);
    //         cr.curve_to(0.50 - mouth_dx,        mouth_top + mouth_dy,
    //                     0.50 + mouth_dx,        mouth_top + mouth_dy,
    //                     0.50 + mouth_width/2.0, mouth_top);

    //         println!("Extents: {:?}", cr.fill_extents());

    //         cr.stroke();

    //         let eye_y = 0.38;
    //         let eye_dx = 0.15;
    //         cr.arc(0.5 - eye_dx, eye_y, 0.05, 0.0, PI * 2.);
    //         cr.fill();

    //         cr.arc(0.5 + eye_dx, eye_y, 0.05, 0.0, PI * 2.);
    //         cr.fill();

    //         Inhibit(false)
    // });

    // container.add(&drawing_area);
    // container.pack_start(&drawing_area, true, false, 0);

    // window.add(&container);
    // window.add(&drawing_area);

    // let term_widget = TermWidget::new();
    // window.add(term_widget.draw_area());

    let shell = Shell::new();
    window.add(shell.main_widget());

    window.show_all();

    // we are using a closure to capture the label (else we could also use a normal function)
    // let tick = move || {
    //     let time = current_time();
    //     label.set_text(&time);
    //     // we could return gtk::Continue(false) to stop our clock after this tick
    //     gtk::Continue(true)
    // };

    // // executes the closure once every second
    // gtk::timeout_add_seconds(1, tick);
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
