use std::{mem, ptr};
use std::cell::RefCell;
#[cfg(unix)] use std::os::unix::io::AsRawFd;

use glib_sys as glib_ffi;

use glib::prelude::*;
use glib::translate::*;
use glib::CallbackGuard;
use glib_sys::{gpointer, gboolean, GIOChannel};

pub use glib::{IOCondition, SourceId};

glib_wrapper! {
    #[derive(Debug)]
    pub struct IOChannel(Shared<GIOChannel>);

    match fn {
        ref   => |ptr| glib_ffi::g_io_channel_ref(ptr),
        unref => |ptr| glib_ffi::g_io_channel_unref(ptr),
    }
}

type ClosureCell = RefCell<Box<FnMut(IOChannel, IOCondition) -> Continue + 'static>>;

unsafe extern "C" fn trampoline(chan: *mut GIOChannel, condition: glib_ffi::GIOCondition, func: gpointer) -> gboolean {
    let _guard = CallbackGuard::new();
    let func: &ClosureCell = mem::transmute(func);
    (&mut *func.borrow_mut())(from_glib_none(chan), from_glib(condition)).to_glib()
}

fn make_closure<F: FnMut(IOChannel, IOCondition) -> Continue + Send + 'static>(func: F) -> gpointer {
    let func: Box<ClosureCell> = Box::new(RefCell::new(Box::new(func)));
    Box::into_raw(func) as gpointer
}

unsafe extern "C" fn destroy_closure(ptr: gpointer) {
    let _guard = CallbackGuard::new();
    Box::<ClosureCell>::from_raw(ptr as *mut _);
}

impl IOChannel {
    #[cfg(unix)]
    pub fn new<FD: AsRawFd>(fd: &FD) -> IOChannel {
        // assert_initialized_main_thread!();   // XXX: needed?
        let fd = fd.as_raw_fd();
        unsafe {
            IOChannel::from_glib_none(glib_ffi::g_io_channel_unix_new(fd))
        }
    }

    pub fn add_watch<F>(&self, condition: IOCondition, func: F) -> SourceId
    where F: FnMut(IOChannel, IOCondition) -> Continue + Send + 'static {
        unsafe {
            from_glib(glib_ffi::g_io_add_watch_full(
                self.to_glib_none().0,
                glib_ffi::G_PRIORITY_DEFAULT,
                condition.to_glib(),
                Some(trampoline),
                make_closure(func),
                Some(destroy_closure),
            ))
        }
    }
}
