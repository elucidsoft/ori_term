use vte::ansi::{Color, NamedColor, Rgb};

use crate::cell::CellFlags;

pub const NUM_COLORS: usize = 270;

// Catppuccin Mocha ANSI colors
const CATPPUCCIN_ANSI: [Rgb; 16] = [
    Rgb { r: 0x45, g: 0x47, b: 0x5a }, // Black (Surface1)
    Rgb { r: 0xf3, g: 0x8b, b: 0xa8 }, // Red
    Rgb { r: 0xa6, g: 0xe3, b: 0xa1 }, // Green
    Rgb { r: 0xf9, g: 0xe2, b: 0xaf }, // Yellow
    Rgb { r: 0x89, g: 0xb4, b: 0xfa }, // Blue
    Rgb { r: 0xf5, g: 0xc2, b: 0xe7 }, // Magenta (Pink)
    Rgb { r: 0x94, g: 0xe2, b: 0xd5 }, // Cyan (Teal)
    Rgb { r: 0xba, g: 0xc2, b: 0xde }, // White (Subtext1)
    Rgb { r: 0x58, g: 0x5b, b: 0x70 }, // Bright Black (Surface2)
    Rgb { r: 0xf3, g: 0x8b, b: 0xa8 }, // Bright Red
    Rgb { r: 0xa6, g: 0xe3, b: 0xa1 }, // Bright Green
    Rgb { r: 0xf9, g: 0xe2, b: 0xaf }, // Bright Yellow
    Rgb { r: 0x89, g: 0xb4, b: 0xfa }, // Bright Blue
    Rgb { r: 0xf5, g: 0xc2, b: 0xe7 }, // Bright Magenta
    Rgb { r: 0x94, g: 0xe2, b: 0xd5 }, // Bright Cyan
    Rgb { r: 0xa6, g: 0xad, b: 0xc8 }, // Bright White (Subtext0)
];

const DEFAULT_FG: Rgb = Rgb { r: 0xff, g: 0xff, b: 0xff }; // White
const DEFAULT_BG: Rgb = Rgb { r: 0x00, g: 0x00, b: 0x00 }; // Pure black
const DEFAULT_CURSOR: Rgb = Rgb { r: 0xf5, g: 0xe0, b: 0xdc }; // Catppuccin Mocha "Rosewater"

#[derive(Debug, Clone)]
pub struct Palette {
    colors: [Rgb; NUM_COLORS],
    defaults: [Rgb; NUM_COLORS],
}

impl Palette {
    pub fn new() -> Self {
        let mut colors = [Rgb { r: 0, g: 0, b: 0 }; NUM_COLORS];

        // 0-15: ANSI colors
        for (i, &c) in CATPPUCCIN_ANSI.iter().enumerate() {
            colors[i] = c;
        }

        // 16-231: 6x6x6 color cube
        for r in 0..6u8 {
            for g in 0..6u8 {
                for b in 0..6u8 {
                    let idx = 16 + (r as usize * 36) + (g as usize * 6) + b as usize;
                    colors[idx] = Rgb {
                        r: if r == 0 { 0 } else { 55 + r * 40 },
                        g: if g == 0 { 0 } else { 55 + g * 40 },
                        b: if b == 0 { 0 } else { 55 + b * 40 },
                    };
                }
            }
        }

        // 232-255: grayscale ramp
        for i in 0..24u8 {
            let v = 8 + i * 10;
            colors[232 + i as usize] = Rgb { r: v, g: v, b: v };
        }

        // 256+: semantic colors (matching NamedColor enum values)
        colors[NamedColor::Foreground as usize] = DEFAULT_FG;
        colors[NamedColor::Background as usize] = DEFAULT_BG;
        colors[NamedColor::Cursor as usize] = DEFAULT_CURSOR;

        // Dim variants (260-267)
        for i in 0..8 {
            let base = colors[i];
            colors[NamedColor::DimBlack as usize + i] = dim_color(base);
        }

        // BrightForeground / DimForeground
        colors[NamedColor::BrightForeground as usize] = Rgb { r: 0xcd, g: 0xd6, b: 0xf4 };
        colors[NamedColor::DimForeground as usize] = dim_color(DEFAULT_FG);

        let defaults = colors;
        Self { colors, defaults }
    }

    pub fn resolve(&self, color: Color, flags: CellFlags) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => self.colors[idx as usize],
            Color::Named(name) => {
                let idx = name as usize;
                if idx < NUM_COLORS {
                    let mut c = self.colors[idx];
                    // Bold-as-bright: for standard colors 0-7, promote to 8-15
                    if flags.contains(CellFlags::BOLD) && idx < 8 {
                        c = self.colors[idx + 8];
                    }
                    c
                } else {
                    DEFAULT_FG
                }
            }
        }
    }

    pub fn resolve_fg(&self, fg: Color, bg: Color, flags: CellFlags) -> Rgb {
        let mut resolved_fg = self.resolve(fg, flags);
        let resolved_bg = self.resolve(bg, CellFlags::empty());

        if flags.contains(CellFlags::DIM) {
            resolved_fg = dim_color(resolved_fg);
        }
        if flags.contains(CellFlags::INVERSE) {
            return resolved_bg;
        }
        if flags.contains(CellFlags::HIDDEN) {
            return resolved_bg;
        }
        resolved_fg
    }

    pub fn resolve_bg(&self, fg: Color, bg: Color, flags: CellFlags) -> Rgb {
        let resolved_fg = self.resolve(fg, flags);
        let resolved_bg = self.resolve(bg, CellFlags::empty());

        if flags.contains(CellFlags::INVERSE) {
            return resolved_fg;
        }
        resolved_bg
    }

    pub fn default_fg(&self) -> Rgb {
        self.colors[NamedColor::Foreground as usize]
    }

    pub fn default_bg(&self) -> Rgb {
        self.colors[NamedColor::Background as usize]
    }

    pub fn cursor_color(&self) -> Rgb {
        self.colors[NamedColor::Cursor as usize]
    }

    pub fn set_color(&mut self, idx: usize, rgb: Rgb) {
        if idx < NUM_COLORS {
            self.colors[idx] = rgb;
        }
    }

    pub fn reset_color(&mut self, idx: usize) {
        if idx < NUM_COLORS {
            self.colors[idx] = self.defaults[idx];
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

pub fn rgb_to_u32(rgb: Rgb) -> u32 {
    (rgb.r as u32) << 16 | (rgb.g as u32) << 8 | rgb.b as u32
}

fn dim_color(c: Rgb) -> Rgb {
    Rgb {
        r: (c.r as u16 * 2 / 3) as u8,
        g: (c.g as u16 * 2 / 3) as u8,
        b: (c.b as u16 * 2 / 3) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_construction() {
        let p = Palette::new();
        // Check ANSI color 1 is red
        assert_eq!(p.colors[1], CATPPUCCIN_ANSI[1]);
        // Check grayscale ramp
        assert_eq!(p.colors[232], Rgb { r: 8, g: 8, b: 8 });
        assert_eq!(p.colors[255], Rgb { r: 238, g: 238, b: 238 });
    }

    #[test]
    fn resolve_named() {
        let p = Palette::new();
        let rgb = p.resolve(Color::Named(NamedColor::Red), CellFlags::empty());
        assert_eq!(rgb, CATPPUCCIN_ANSI[1]);
    }

    #[test]
    fn resolve_bold_bright() {
        let p = Palette::new();
        let normal = p.resolve(Color::Named(NamedColor::Black), CellFlags::empty());
        let bold = p.resolve(Color::Named(NamedColor::Black), CellFlags::BOLD);
        assert_eq!(normal, CATPPUCCIN_ANSI[0]); // Black
        assert_eq!(bold, CATPPUCCIN_ANSI[8]); // BrightBlack
    }

    #[test]
    fn resolve_spec() {
        let p = Palette::new();
        let rgb = p.resolve(Color::Spec(Rgb { r: 255, g: 0, b: 128 }), CellFlags::empty());
        assert_eq!(rgb, Rgb { r: 255, g: 0, b: 128 });
    }

    #[test]
    fn resolve_indexed() {
        let p = Palette::new();
        let rgb = p.resolve(Color::Indexed(232), CellFlags::empty());
        assert_eq!(rgb, Rgb { r: 8, g: 8, b: 8 });
    }

    #[test]
    fn inverse_swaps() {
        let p = Palette::new();
        let fg = Color::Named(NamedColor::Red);
        let bg = Color::Named(NamedColor::Blue);
        let flags = CellFlags::INVERSE;
        let resolved_fg = p.resolve_fg(fg, bg, flags);
        let resolved_bg = p.resolve_bg(fg, bg, flags);
        // fg should become bg and vice versa
        assert_eq!(resolved_fg, p.resolve(bg, CellFlags::empty()));
        assert_eq!(resolved_bg, p.resolve(fg, flags));
    }

    #[test]
    fn rgb_to_u32_conversion() {
        assert_eq!(rgb_to_u32(Rgb { r: 0xFF, g: 0x00, b: 0x80 }), 0x00FF0080);
        assert_eq!(rgb_to_u32(Rgb { r: 0xCD, g: 0xD6, b: 0xF4 }), 0x00CDD6F4);
    }
}
