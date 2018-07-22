#[macro_use] extern crate glib;
extern crate glib_sys;   // XXX: needed?
extern crate gtk;
extern crate gdk;
extern crate gdk_sys;
extern crate gio;
extern crate cairo;

extern crate tp_app as tp;

mod giochannel;
mod utils;
mod shellwidget;
mod input;

use std::env;
use std::process::Command;

use gio::prelude::*;
use gtk::prelude::*;

use shellwidget::ShellWidget;



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

    let session = tp::Session::new(Command::new("zsh")).unwrap();    // XXX
    let shell = ShellWidget::new(session);
    window.add(&shell.main_widget());

    window.show_all();

    shell.grab_focus();
}

fn main() {
    let application = gtk::Application::new("hk.kral.teepee", gio::ApplicationFlags::empty()).unwrap();
    application.connect_startup(|app| build_ui(app));
    application.connect_activate(|_| {});
    application.run(&env::args().collect::<Vec<_>>());
}
