use std::fmt;

use gtk;
use gtk::prelude::*;
// use gtk::WidgetExt;
use cairo;
use cairo::Context as Cairo;
use cairo::{FontFace, FontExtents, FontSlant, FontWeight};

use tp;
use tp::term::{Cell, VTDispatch, VTRendition};


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

    // fn textpos(&self, x: u32, y: u32) -> (f64, f64) {
    //     (x as f64 * self.cellw, y as f64 * self.cellh + self.cellh - self.descent)
    // }

    // fn cellpos(&self, x: u32, y: u32) -> (f64, f64) {
    //     (x as f64 * self.cellw, y as f64 * self.cellh)
    // }
}

impl Default for Font {
    fn default() -> Font {
        // Font::new("Monospace".to_owned(), 11.0)
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


#[derive(Debug)]
pub struct TermWidget {
    draw_area: gtk::DrawingArea,
    font: Font,
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
            draw_area: gtk::DrawingArea::new(),
            font: Font::default(),
        }
    }

    fn render_cell(&self, cr: &Cairo, cell: &mut Cell, x: usize, y: usize) {
        let (cellw, cellh) = (self.font.cellw, self.font.cellh);
        let (x, y) = (x as f64 * cellw, y as f64 * cellh);
        let y_text = y + cellh - self.font.descent;
        let bold = cell.rendition().contains(VTRendition::Bold);

        // Draw cell background
        let (br, bg, bb) = (0.0, 0.0, 0.0);   // XXX
        cr.set_source_rgb(br, bg, bb);
        cr.rectangle(x, y, cellw, cellh);
        cr.fill();

        // Draw cell text
        let (fr, fg, fb) = (240.0, 240.0, 240.0);   // XXX
        cr.set_source_rgb(fr, fg, fb);
        cr.move_to(x, y_text);
        cr.set_font_face(if bold { self.font.boldface.clone() } else {self.font.face.clone()});
        cr.set_font_size(self.font.size);
        cr.show_text(cell.as_str())
    }

    pub fn render(&self, cr: &Cairo, session: &mut tp::Session) {
        // let mut y = 20.0;
        // let mut x = 0.0;
        for (y, line) in session.term.screen().line_iter().enumerate() {
            for (x, cell) in line.iter_mut().enumerate() {
                // cr.move_to(x, y);
                // cr.show_text(c.as_str());
                self.render_cell(cr, cell, x, y);

                // x += 10.0;
            }

            // x = 0.0;
            // y += 20.0;
        }
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

    pub fn queue_draw(&self) {
        self.draw_area.queue_draw();
    }

    pub fn draw_area(&self) -> &gtk::DrawingArea {
        &self.draw_area
    }

    pub fn set_font(&mut self, family: String, size: f64) {
        self.font = Font::new(family, size);
    }

    pub fn screen_size(&self) -> (u32, u32) {
        let alloc = self.draw_area.get_allocation();
        (alloc.width as u32 / self.font.cellw as u32, alloc.height as u32 / self.font.cellh as u32)
    }
}
