use std::mem;
use std::fmt;
use std::ffi::CStr;

use glib::translate::ToGlibPtr;
use gtk;
use gtk::prelude::*;
use gdk;
use gdk_sys;
use cairo;
use cairo::Context as Cairo;
use cairo::{FontFace, FontExtents, FontSlant, FontWeight};

use tp;
use tp::term::{Cell, VTDispatch, VTRendition, InputData, Modifier, Key};


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frgba(pub f64, pub f64, pub f64, pub f64);

impl From<tp::Rgba> for Frgba {
    fn from(rgba: tp::Rgba) -> Frgba {
        Frgba(
            (rgba.0 as f64) / 255.0,
            (rgba.1 as f64) / 255.0,
            (rgba.2 as f64) / 255.0,
            (rgba.3 as f64) / 255.0,
        )
    }
}


struct Font {
    family: String,
    face: FontFace,
    boldface: FontFace,
    size: f64,
    cellw: f64,
    cellh: f64,
    descent: f64,
}

impl Font {
    fn new(family: String, size: f64) -> Font {
        let surface = cairo::ImageSurface::create(cairo::Format::A8, 1, 1).unwrap();   // FIXME: error handling
        let ctx = Cairo::new(&surface);

        let face = FontFace::toy_create(family.as_ref(), FontSlant::Normal, FontWeight::Normal);
        let boldface = FontFace::toy_create(family.as_ref(), FontSlant::Normal, FontWeight::Bold);
        ctx.set_font_face(face.clone());
        ctx.set_font_size(size);
        let exts = ctx.font_extents();

        Font {
            family,
            face,
            boldface,
            size,
            cellw: exts.max_x_advance,
            cellh: exts.height,
            descent: exts.descent
        }
    }
}

impl Default for Font {
    fn default() -> Font {
        Font::new("Monospace".to_owned(), 15.0)
    }
}

impl fmt::Debug for Font {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Font")
           .field("family", &self.family)
           .field("size", &self.size)
           .field("cellw", &self.cellw)
           .field("cellh", &self.cellh)
           .field("descent", &self.descent)
           .finish()
    }
}

struct ModifierType(gdk::ModifierType);

impl From<ModifierType> for Modifier {
    fn from(modifier: ModifierType) -> Modifier {
        let bits = modifier.0.bits();
        let bits = bits & 1 | bits & 4 | bits & 8 << 2;
        Modifier::from_bits_truncate(bits as u8)
    }
}

struct EventKey<'a>(&'a gdk::EventKey);

// TODO: Use TryFrom when it's been stable enough
impl<'a> From<EventKey<'a>> for Option<InputData<'a>> {
    fn from(evt: EventKey<'a>) -> Option<InputData<'a>> {
        use gdk::enums::key;
        use gdk_sys::*;

        let keyval = evt.0.get_keyval();
        let unicode = unsafe { gdk_sys::gdk_keyval_to_unicode(keyval as _) };
        let unicode: char = unsafe { mem::transmute(unicode) };
        let modifier: Modifier = ModifierType(evt.0.get_state()).into();

        Some(match keyval {
            key::Return    | key::KP_Enter     => InputData::Key(Key::Return, modifier),
            key::Tab                           => InputData::Key(Key::Tab, modifier),
            key::BackSpace                     => InputData::Key(Key::Backspace, modifier),
            key::Up        | key::KP_Up        => InputData::Key(Key::Up, modifier),
            key::Down      | key::KP_Down      => InputData::Key(Key::Down, modifier),
            key::Right     | key::KP_Right     => InputData::Key(Key::Right, modifier),
            key::Left      | key::KP_Left      => InputData::Key(Key::Left, modifier),
            key::Page_Up   | key::KP_Page_Up   => InputData::Key(Key::PageUp, modifier),
            key::Page_Down | key::KP_Page_Down => InputData::Key(Key::PageDown, modifier),
            key::Home      | key::KP_Home      => InputData::Key(Key::Home, modifier),
            key::End       | key::KP_End       => InputData::Key(Key::End, modifier),
            key::Insert    | key::KP_Insert    => InputData::Key(Key::Insert, modifier),
            key::Delete    | key::KP_Delete    => InputData::Key(Key::Delete, modifier),

            key::F1 ... key::F35 => InputData::FKey((keyval - key::F1 + 1) as u8, modifier),
            key::KP_F1 ... key::KP_F4 => InputData::FKey((keyval - key::KP_F1 + 1) as u8, modifier),

            _ if unicode != '\0' => InputData::Char(unicode, modifier),
            _ => return None,
        })
    }
}

#[derive(Debug)]
pub struct TermWidget {
    draw_area: gtk::DrawingArea,
    font: Font,
}

impl TermWidget {
    pub fn new() -> TermWidget {
        let draw_area = gtk::DrawingArea::new();

        // Enable key press events
        draw_area.add_events(gdk::EventMask::KEY_PRESS_MASK.bits() as i32);

        TermWidget {
            draw_area: gtk::DrawingArea::new(),
            font: Font::default(),
        }
    }

    fn render_cell(&self, cr: &Cairo, cell: &mut Cell, x: usize, y: usize, colors: &tp::ColorScheme) {
        let (cellw, cellh) = (self.font.cellw, self.font.cellh);
        let (x, y) = (x as f64 * cellw, y as f64 * cellh);
        let y_text = y + cellh - self.font.descent;
        let bold = cell.rendition().contains(VTRendition::BOLD);
        let mut bg: Frgba = colors.get_color(cell.style.col_bg).into();
        let mut fg: Frgba = colors.get_color(cell.style.col_fg).into();
        if cell.rendition().contains(VTRendition::INVERSE) {    // TODO: also REVERSE_VIDEO
            mem::swap(&mut bg, &mut fg);
        }

        // Draw cell background
        cr.set_source_rgba(bg.0, bg.1, bg.2, bg.3);
        cr.rectangle(x, y, cellw, cellh);
        cr.fill();

        // Draw cell text
        cr.set_source_rgba(fg.0, fg.1, fg.2, fg.3);
        cr.move_to(x, y_text);
        cr.set_font_face(if bold { self.font.boldface.clone() } else {self.font.face.clone()});
        cr.set_font_size(self.font.size);
        cr.show_text(cell.as_str())
    }

    pub fn render(&self, cr: &Cairo, session: &mut tp::Session) {
        let alloc = self.draw_area.get_allocation();
        cr.set_source_rgb(0.0, 0.0, 0.0);
        cr.rectangle(0.0, 0.0, alloc.width as f64, alloc.height as f64);
        cr.fill();

        let colors = &session.colors;
        for (y, line) in session.term.screen_mut().line_iter().enumerate() {
            for (x, cell) in line.iter_mut().enumerate() {
                self.render_cell(cr, cell, x, y, colors);
            }
        }

        // Draw cursor: TODO
        // let (cellw, cellh) = (self.font.cellw, self.font.cellh);
        // let (cx, cy) = session.term.screen().cursor_position();
        // let (cx, cy) = (cx as f64 * cellw, cy as f64 * cellh);
    }

    pub fn connect_draw<F>(&self, func: F)
    where F: Fn(&Cairo) + 'static {
        self.draw_area.connect_draw(move |_, cairo| {
            func(cairo);
            Inhibit(false)
        });
    }

    pub fn connect_resize<F>(&self, func: F)
    where F: Fn() + 'static {
        self.draw_area.connect_configure_event(move |_, _| {
            func();
            false
        });
    }

    pub fn connect_input<F>(&self, func: F)
    where F: Fn(InputData) + 'static {
        self.draw_area.connect_key_press_event(move |_, evt| {
            // TODO: https://stackoverflow.com/questions/40011838/how-to-receive-characters-from-input-method-in-gtk2
            // (Or do whatever Gnome vte widget does)

            let input: Option<InputData> = EventKey(evt).into();
            if let Some(input) = input {
                func(input);
            }

            Inhibit(false)
        });
    }

    pub fn grab_focus(&self) {
        self.draw_area.set_can_focus(true);   // Apparently this needs to be done when the ui is built
        self.draw_area.grab_focus();
    }

    pub fn queue_draw(&self) {
        self.draw_area.queue_draw();
    }

    pub fn draw_area(&self) -> &gtk::DrawingArea {
        &self.draw_area
    }

    pub fn set_font(&mut self, family: String, size: f64) {
        self.font = Font::new(family, size);
    }

    pub fn screen_size(&self) -> (u16, u16) {
        let alloc = self.draw_area.get_allocation();
        (alloc.width as u16 / self.font.cellw as u16, alloc.height as u16 / self.font.cellh as u16)
    }
}
