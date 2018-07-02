use ::term::VTColor;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub fn rgb(r: u8, g: u8, b: u8) -> Rgba { Rgba(r, g, b, 0xff) }
}

impl Default for Rgba {
    fn default() -> Rgba { Self::rgb(0, 0, 0) }
}


#[derive(Debug)]
pub struct ColorScheme {
    fg: Rgba,
    bg: Rgba,
    system: [Rgba ; 16],
}

impl Default for ColorScheme {
    fn default() -> ColorScheme {
        ColorScheme {
            fg: Rgba::rgb(0xff, 0xff, 0xff),
            bg: Rgba::default(),
            system: [
                // Base colors
                Rgba::rgb(0x00, 0x00, 0x00),
                Rgba::rgb(0xc0, 0x00, 0x00),
                Rgba::rgb(0x00, 0xc0, 0x00),
                Rgba::rgb(0xc0, 0xc0, 0x00),
                Rgba::rgb(0x00, 0x00, 0xc0),
                Rgba::rgb(0xc0, 0x00, 0xc0),
                Rgba::rgb(0x00, 0xc0, 0xc0),
                Rgba::rgb(0xc0, 0xc0, 0xc0),
                // Bright colors
                Rgba::rgb(0x58, 0x58, 0x58),
                Rgba::rgb(0xff, 0x00, 0x00),
                Rgba::rgb(0x00, 0xff, 0x00),
                Rgba::rgb(0xff, 0xff, 0x00),
                Rgba::rgb(0x00, 0x30, 0xff),
                Rgba::rgb(0xff, 0x30, 0xff),
                Rgba::rgb(0x00, 0xff, 0xff),
                Rgba::rgb(0xff, 0xff, 0xff),
            ],
        }
    }
}

impl ColorScheme {
    fn indexed_color(&self, idx: u8) -> Rgba {
        const CUBE: [u8 ; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];

        match idx {
            // System palette
            0 ... 15 => self.system[idx as usize],
            // 6 level RGB
            16 ... 231 => {
                let i = idx - 16;
                let b = i % 6;
                let g = i / 6 % 6;
                let r = i / 36;
                Rgba::rgb(CUBE[r as usize], CUBE[g as usize], CUBE[b as usize])
            },
            // Grayscale
            232 ... 255 => {
                let gray = (idx - 232) * 10 + 8;
                Rgba::rgb(gray, gray, gray)
            },
            _ => unreachable!(),
        }
    }

    pub fn get_color(&self, vtcolor: VTColor) -> Rgba {
        use self::VTColor::*;

        match vtcolor {
            DefaultFg => self.fg,
            DefaultBg => self.bg,
            Indexed(idx) => self.indexed_color(idx),
            Rgb(r, g, b) => Rgba(r, g, b, 0xff),
        }
    }
}
