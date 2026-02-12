use vte::ansi::{Color, NamedColor, Rgb};

use crate::cell::CellFlags;
use crate::config::ColorConfig;

pub const NUM_COLORS: usize = 270;

/// A color scheme defines the 16 ANSI colors plus foreground, background, and cursor colors.
#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub name: &'static str,
    pub ansi: [Rgb; 16],
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
}

pub const CATPPUCCIN_MOCHA: ColorScheme = ColorScheme {
    name: "Catppuccin Mocha",
    ansi: [
        Rgb {
            r: 0x45,
            g: 0x47,
            b: 0x5a,
        }, // Black (Surface1)
        Rgb {
            r: 0xf3,
            g: 0x8b,
            b: 0xa8,
        }, // Red
        Rgb {
            r: 0xa6,
            g: 0xe3,
            b: 0xa1,
        }, // Green
        Rgb {
            r: 0xf9,
            g: 0xe2,
            b: 0xaf,
        }, // Yellow
        Rgb {
            r: 0x89,
            g: 0xb4,
            b: 0xfa,
        }, // Blue
        Rgb {
            r: 0xf5,
            g: 0xc2,
            b: 0xe7,
        }, // Magenta (Pink)
        Rgb {
            r: 0x94,
            g: 0xe2,
            b: 0xd5,
        }, // Cyan (Teal)
        Rgb {
            r: 0xba,
            g: 0xc2,
            b: 0xde,
        }, // White (Subtext1)
        Rgb {
            r: 0x58,
            g: 0x5b,
            b: 0x70,
        }, // Bright Black (Surface2)
        Rgb {
            r: 0xf3,
            g: 0x8b,
            b: 0xa8,
        }, // Bright Red
        Rgb {
            r: 0xa6,
            g: 0xe3,
            b: 0xa1,
        }, // Bright Green
        Rgb {
            r: 0xf9,
            g: 0xe2,
            b: 0xaf,
        }, // Bright Yellow
        Rgb {
            r: 0x89,
            g: 0xb4,
            b: 0xfa,
        }, // Bright Blue
        Rgb {
            r: 0xf5,
            g: 0xc2,
            b: 0xe7,
        }, // Bright Magenta
        Rgb {
            r: 0x94,
            g: 0xe2,
            b: 0xd5,
        }, // Bright Cyan
        Rgb {
            r: 0xa6,
            g: 0xad,
            b: 0xc8,
        }, // Bright White (Subtext0)
    ],
    fg: Rgb {
        r: 0xcd,
        g: 0xd6,
        b: 0xf4,
    }, // Text
    bg: Rgb {
        r: 0x1e,
        g: 0x1e,
        b: 0x2e,
    }, // Base
    cursor: Rgb {
        r: 0xf5,
        g: 0xe0,
        b: 0xdc,
    }, // Rosewater
};

pub const CATPPUCCIN_LATTE: ColorScheme = ColorScheme {
    name: "Catppuccin Latte",
    ansi: [
        Rgb {
            r: 0x5c,
            g: 0x5f,
            b: 0x77,
        }, // Black (Subtext1)
        Rgb {
            r: 0xd2,
            g: 0x0f,
            b: 0x39,
        }, // Red
        Rgb {
            r: 0x40,
            g: 0xa0,
            b: 0x2b,
        }, // Green
        Rgb {
            r: 0xdf,
            g: 0x8e,
            b: 0x1d,
        }, // Yellow
        Rgb {
            r: 0x1e,
            g: 0x66,
            b: 0xf5,
        }, // Blue
        Rgb {
            r: 0xea,
            g: 0x76,
            b: 0xcb,
        }, // Magenta (Pink)
        Rgb {
            r: 0x17,
            g: 0x9c,
            b: 0x99,
        }, // Cyan (Teal)
        Rgb {
            r: 0xac,
            g: 0xb0,
            b: 0xbe,
        }, // White (Surface2)
        Rgb {
            r: 0x6c,
            g: 0x6f,
            b: 0x85,
        }, // Bright Black (Subtext0)
        Rgb {
            r: 0xd2,
            g: 0x0f,
            b: 0x39,
        }, // Bright Red
        Rgb {
            r: 0x40,
            g: 0xa0,
            b: 0x2b,
        }, // Bright Green
        Rgb {
            r: 0xdf,
            g: 0x8e,
            b: 0x1d,
        }, // Bright Yellow
        Rgb {
            r: 0x1e,
            g: 0x66,
            b: 0xf5,
        }, // Bright Blue
        Rgb {
            r: 0xea,
            g: 0x76,
            b: 0xcb,
        }, // Bright Magenta
        Rgb {
            r: 0x17,
            g: 0x9c,
            b: 0x99,
        }, // Bright Cyan
        Rgb {
            r: 0xbc,
            g: 0xc0,
            b: 0xcc,
        }, // Bright White (Surface1)
    ],
    fg: Rgb {
        r: 0x4c,
        g: 0x4f,
        b: 0x69,
    }, // Text
    bg: Rgb {
        r: 0xef,
        g: 0xf1,
        b: 0xf5,
    }, // Base
    cursor: Rgb {
        r: 0xdc,
        g: 0x8a,
        b: 0x78,
    }, // Rosewater
};

pub const ONE_DARK: ColorScheme = ColorScheme {
    name: "One Dark",
    ansi: [
        Rgb {
            r: 0x28,
            g: 0x2c,
            b: 0x34,
        }, // Black
        Rgb {
            r: 0xe0,
            g: 0x6c,
            b: 0x75,
        }, // Red
        Rgb {
            r: 0x98,
            g: 0xc3,
            b: 0x79,
        }, // Green
        Rgb {
            r: 0xe5,
            g: 0xc0,
            b: 0x7b,
        }, // Yellow
        Rgb {
            r: 0x61,
            g: 0xaf,
            b: 0xef,
        }, // Blue
        Rgb {
            r: 0xc6,
            g: 0x78,
            b: 0xdd,
        }, // Magenta
        Rgb {
            r: 0x56,
            g: 0xb6,
            b: 0xc2,
        }, // Cyan
        Rgb {
            r: 0xab,
            g: 0xb2,
            b: 0xbf,
        }, // White
        Rgb {
            r: 0x54,
            g: 0x58,
            b: 0x62,
        }, // Bright Black
        Rgb {
            r: 0xe0,
            g: 0x6c,
            b: 0x75,
        }, // Bright Red
        Rgb {
            r: 0x98,
            g: 0xc3,
            b: 0x79,
        }, // Bright Green
        Rgb {
            r: 0xe5,
            g: 0xc0,
            b: 0x7b,
        }, // Bright Yellow
        Rgb {
            r: 0x61,
            g: 0xaf,
            b: 0xef,
        }, // Bright Blue
        Rgb {
            r: 0xc6,
            g: 0x78,
            b: 0xdd,
        }, // Bright Magenta
        Rgb {
            r: 0x56,
            g: 0xb6,
            b: 0xc2,
        }, // Bright Cyan
        Rgb {
            r: 0xbe,
            g: 0xc5,
            b: 0xd4,
        }, // Bright White
    ],
    fg: Rgb {
        r: 0xab,
        g: 0xb2,
        b: 0xbf,
    },
    bg: Rgb {
        r: 0x28,
        g: 0x2c,
        b: 0x34,
    },
    cursor: Rgb {
        r: 0x52,
        g: 0x8b,
        b: 0xff,
    },
};

pub const SOLARIZED_DARK: ColorScheme = ColorScheme {
    name: "Solarized Dark",
    ansi: [
        Rgb {
            r: 0x07,
            g: 0x36,
            b: 0x42,
        }, // Black (base02)
        Rgb {
            r: 0xdc,
            g: 0x32,
            b: 0x2f,
        }, // Red
        Rgb {
            r: 0x85,
            g: 0x99,
            b: 0x00,
        }, // Green
        Rgb {
            r: 0xb5,
            g: 0x89,
            b: 0x00,
        }, // Yellow
        Rgb {
            r: 0x26,
            g: 0x8b,
            b: 0xd2,
        }, // Blue
        Rgb {
            r: 0xd3,
            g: 0x36,
            b: 0x82,
        }, // Magenta
        Rgb {
            r: 0x2a,
            g: 0xa1,
            b: 0x98,
        }, // Cyan
        Rgb {
            r: 0xee,
            g: 0xe8,
            b: 0xd5,
        }, // White (base2)
        Rgb {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }, // Bright Black (base03)
        Rgb {
            r: 0xcb,
            g: 0x4b,
            b: 0x16,
        }, // Bright Red (orange)
        Rgb {
            r: 0x58,
            g: 0x6e,
            b: 0x75,
        }, // Bright Green (base01)
        Rgb {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }, // Bright Yellow (base00)
        Rgb {
            r: 0x83,
            g: 0x94,
            b: 0x96,
        }, // Bright Blue (base0)
        Rgb {
            r: 0x6c,
            g: 0x71,
            b: 0xc4,
        }, // Bright Magenta (violet)
        Rgb {
            r: 0x93,
            g: 0xa1,
            b: 0xa1,
        }, // Bright Cyan (base1)
        Rgb {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }, // Bright White (base3)
    ],
    fg: Rgb {
        r: 0x83,
        g: 0x94,
        b: 0x96,
    }, // base0
    bg: Rgb {
        r: 0x00,
        g: 0x2b,
        b: 0x36,
    }, // base03
    cursor: Rgb {
        r: 0x83,
        g: 0x94,
        b: 0x96,
    },
};

pub const SOLARIZED_LIGHT: ColorScheme = ColorScheme {
    name: "Solarized Light",
    ansi: [
        Rgb {
            r: 0xee,
            g: 0xe8,
            b: 0xd5,
        }, // Black (base2)
        Rgb {
            r: 0xdc,
            g: 0x32,
            b: 0x2f,
        }, // Red
        Rgb {
            r: 0x85,
            g: 0x99,
            b: 0x00,
        }, // Green
        Rgb {
            r: 0xb5,
            g: 0x89,
            b: 0x00,
        }, // Yellow
        Rgb {
            r: 0x26,
            g: 0x8b,
            b: 0xd2,
        }, // Blue
        Rgb {
            r: 0xd3,
            g: 0x36,
            b: 0x82,
        }, // Magenta
        Rgb {
            r: 0x2a,
            g: 0xa1,
            b: 0x98,
        }, // Cyan
        Rgb {
            r: 0x07,
            g: 0x36,
            b: 0x42,
        }, // White (base02)
        Rgb {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }, // Bright Black (base3)
        Rgb {
            r: 0xcb,
            g: 0x4b,
            b: 0x16,
        }, // Bright Red (orange)
        Rgb {
            r: 0x93,
            g: 0xa1,
            b: 0xa1,
        }, // Bright Green (base1)
        Rgb {
            r: 0x83,
            g: 0x94,
            b: 0x96,
        }, // Bright Yellow (base0)
        Rgb {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }, // Bright Blue (base00)
        Rgb {
            r: 0x6c,
            g: 0x71,
            b: 0xc4,
        }, // Bright Magenta (violet)
        Rgb {
            r: 0x58,
            g: 0x6e,
            b: 0x75,
        }, // Bright Cyan (base01)
        Rgb {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }, // Bright White (base03)
    ],
    fg: Rgb {
        r: 0x65,
        g: 0x7b,
        b: 0x83,
    }, // base00
    bg: Rgb {
        r: 0xfd,
        g: 0xf6,
        b: 0xe3,
    }, // base3
    cursor: Rgb {
        r: 0x65,
        g: 0x7b,
        b: 0x83,
    },
};

pub const DRACULA: ColorScheme = ColorScheme {
    name: "Dracula",
    ansi: [
        Rgb {
            r: 0x21,
            g: 0x22,
            b: 0x2c,
        }, // Black
        Rgb {
            r: 0xff,
            g: 0x55,
            b: 0x55,
        }, // Red
        Rgb {
            r: 0x50,
            g: 0xfa,
            b: 0x7b,
        }, // Green
        Rgb {
            r: 0xf1,
            g: 0xfa,
            b: 0x8c,
        }, // Yellow
        Rgb {
            r: 0xbd,
            g: 0x93,
            b: 0xf9,
        }, // Blue (Purple)
        Rgb {
            r: 0xff,
            g: 0x79,
            b: 0xc6,
        }, // Magenta (Pink)
        Rgb {
            r: 0x8b,
            g: 0xe9,
            b: 0xfd,
        }, // Cyan
        Rgb {
            r: 0xf8,
            g: 0xf8,
            b: 0xf2,
        }, // White
        Rgb {
            r: 0x62,
            g: 0x72,
            b: 0xa4,
        }, // Bright Black (Comment)
        Rgb {
            r: 0xff,
            g: 0x6e,
            b: 0x6e,
        }, // Bright Red
        Rgb {
            r: 0x69,
            g: 0xff,
            b: 0x94,
        }, // Bright Green
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xa5,
        }, // Bright Yellow
        Rgb {
            r: 0xd6,
            g: 0xac,
            b: 0xff,
        }, // Bright Blue
        Rgb {
            r: 0xff,
            g: 0x92,
            b: 0xdf,
        }, // Bright Magenta
        Rgb {
            r: 0xa4,
            g: 0xff,
            b: 0xff,
        }, // Bright Cyan
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        }, // Bright White
    ],
    fg: Rgb {
        r: 0xf8,
        g: 0xf8,
        b: 0xf2,
    },
    bg: Rgb {
        r: 0x28,
        g: 0x2a,
        b: 0x36,
    },
    cursor: Rgb {
        r: 0xf8,
        g: 0xf8,
        b: 0xf2,
    },
};

pub const TOKYO_NIGHT: ColorScheme = ColorScheme {
    name: "Tokyo Night",
    ansi: [
        Rgb {
            r: 0x15,
            g: 0x16,
            b: 0x1e,
        }, // Black
        Rgb {
            r: 0xf7,
            g: 0x76,
            b: 0x8e,
        }, // Red
        Rgb {
            r: 0x9e,
            g: 0xce,
            b: 0x6a,
        }, // Green
        Rgb {
            r: 0xe0,
            g: 0xaf,
            b: 0x68,
        }, // Yellow
        Rgb {
            r: 0x7a,
            g: 0xa2,
            b: 0xf7,
        }, // Blue
        Rgb {
            r: 0xbb,
            g: 0x9a,
            b: 0xf7,
        }, // Magenta (Purple)
        Rgb {
            r: 0x7d,
            g: 0xcf,
            b: 0xff,
        }, // Cyan
        Rgb {
            r: 0xa9,
            g: 0xb1,
            b: 0xd6,
        }, // White
        Rgb {
            r: 0x41,
            g: 0x48,
            b: 0x68,
        }, // Bright Black
        Rgb {
            r: 0xf7,
            g: 0x76,
            b: 0x8e,
        }, // Bright Red
        Rgb {
            r: 0x9e,
            g: 0xce,
            b: 0x6a,
        }, // Bright Green
        Rgb {
            r: 0xe0,
            g: 0xaf,
            b: 0x68,
        }, // Bright Yellow
        Rgb {
            r: 0x7a,
            g: 0xa2,
            b: 0xf7,
        }, // Bright Blue
        Rgb {
            r: 0xbb,
            g: 0x9a,
            b: 0xf7,
        }, // Bright Magenta
        Rgb {
            r: 0x7d,
            g: 0xcf,
            b: 0xff,
        }, // Bright Cyan
        Rgb {
            r: 0xc0,
            g: 0xca,
            b: 0xf5,
        }, // Bright White
    ],
    fg: Rgb {
        r: 0xa9,
        g: 0xb1,
        b: 0xd6,
    },
    bg: Rgb {
        r: 0x1a,
        g: 0x1b,
        b: 0x26,
    },
    cursor: Rgb {
        r: 0xc0,
        g: 0xca,
        b: 0xf5,
    },
};

pub const WEZTERM_DEFAULT: ColorScheme = ColorScheme {
    name: "WezTerm Default",
    ansi: [
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00,
        }, // Black
        Rgb {
            r: 0xcc,
            g: 0x55,
            b: 0x55,
        }, // Red (Maroon)
        Rgb {
            r: 0x55,
            g: 0xcc,
            b: 0x55,
        }, // Green
        Rgb {
            r: 0xcd,
            g: 0xcd,
            b: 0x55,
        }, // Yellow (Olive)
        Rgb {
            r: 0x54,
            g: 0x55,
            b: 0xcb,
        }, // Blue (Navy)
        Rgb {
            r: 0xcc,
            g: 0x55,
            b: 0xcc,
        }, // Magenta (Purple)
        Rgb {
            r: 0x7a,
            g: 0xca,
            b: 0xca,
        }, // Cyan (Teal)
        Rgb {
            r: 0xcc,
            g: 0xcc,
            b: 0xcc,
        }, // White (Silver)
        Rgb {
            r: 0x55,
            g: 0x55,
            b: 0x55,
        }, // Bright Black (Grey)
        Rgb {
            r: 0xff,
            g: 0x55,
            b: 0x55,
        }, // Bright Red
        Rgb {
            r: 0x55,
            g: 0xff,
            b: 0x55,
        }, // Bright Green
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0x55,
        }, // Bright Yellow
        Rgb {
            r: 0x55,
            g: 0x55,
            b: 0xff,
        }, // Bright Blue
        Rgb {
            r: 0xff,
            g: 0x55,
            b: 0xff,
        }, // Bright Magenta
        Rgb {
            r: 0x55,
            g: 0xff,
            b: 0xff,
        }, // Bright Cyan
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        }, // Bright White
    ],
    fg: Rgb {
        r: 0xb2,
        g: 0xb2,
        b: 0xb2,
    },
    bg: Rgb {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    },
    cursor: Rgb {
        r: 0x52,
        g: 0xad,
        b: 0x70,
    },
};

/// All built-in color schemes.
pub const BUILTIN_SCHEMES: &[&ColorScheme] = &[
    &WEZTERM_DEFAULT,
    &CATPPUCCIN_MOCHA,
    &CATPPUCCIN_LATTE,
    &ONE_DARK,
    &SOLARIZED_DARK,
    &SOLARIZED_LIGHT,
    &DRACULA,
    &TOKYO_NIGHT,
];

/// Look up a built-in color scheme by name (case-insensitive).
pub fn find_scheme(name: &str) -> Option<&'static ColorScheme> {
    BUILTIN_SCHEMES
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .copied()
}

#[derive(Debug, Clone)]
pub struct Palette {
    colors: [Rgb; NUM_COLORS],
    defaults: [Rgb; NUM_COLORS],
    pub bold_is_bright: bool,
    /// Custom selection foreground color. None = use BG (swap behavior).
    pub selection_fg: Option<Rgb>,
    /// Custom selection background color. None = use FG (swap behavior).
    pub selection_bg: Option<Rgb>,
}

impl Palette {
    pub fn new() -> Self {
        Self::from_scheme(&CATPPUCCIN_MOCHA)
    }

    pub fn from_scheme(scheme: &ColorScheme) -> Self {
        let mut colors = [Rgb { r: 0, g: 0, b: 0 }; NUM_COLORS];

        // 0-15: ANSI colors from scheme
        for (i, &c) in scheme.ansi.iter().enumerate() {
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

        // 256+: semantic colors from scheme
        colors[NamedColor::Foreground as usize] = scheme.fg;
        colors[NamedColor::Background as usize] = scheme.bg;
        colors[NamedColor::Cursor as usize] = scheme.cursor;

        // Dim variants (260-267)
        for i in 0..8 {
            let base = colors[i];
            colors[NamedColor::DimBlack as usize + i] = dim_color(base);
        }

        // BrightForeground / DimForeground
        colors[NamedColor::BrightForeground as usize] = scheme.fg;
        colors[NamedColor::DimForeground as usize] = dim_color(scheme.fg);

        let defaults = colors;
        Self {
            colors,
            defaults,
            bold_is_bright: true,
            selection_fg: None,
            selection_bg: None,
        }
    }

    /// Replace the palette with a new color scheme. Resets all colors.
    /// Preserves the `bold_is_bright` setting.
    pub fn set_scheme(&mut self, scheme: &ColorScheme) {
        let fresh = Self::from_scheme(scheme);
        self.colors = fresh.colors;
        self.defaults = fresh.defaults;
        self.selection_fg = None;
        self.selection_bg = None;
    }

    /// Apply config color overrides on top of the current scheme.
    /// Call this after `set_scheme()`.
    pub fn apply_overrides(&mut self, colors: &ColorConfig) {
        if let Some(rgb) = colors.foreground.as_deref().and_then(parse_hex_color) {
            self.colors[NamedColor::Foreground as usize] = rgb;
            self.defaults[NamedColor::Foreground as usize] = rgb;
        }
        if let Some(rgb) = colors.background.as_deref().and_then(parse_hex_color) {
            self.colors[NamedColor::Background as usize] = rgb;
            self.defaults[NamedColor::Background as usize] = rgb;
        }
        if let Some(rgb) = colors.cursor.as_deref().and_then(parse_hex_color) {
            self.colors[NamedColor::Cursor as usize] = rgb;
            self.defaults[NamedColor::Cursor as usize] = rgb;
        }
        // ANSI 0-7
        for (key, hex) in &colors.ansi {
            if let Ok(i) = key.parse::<usize>() {
                if i < 8 {
                    if let Some(rgb) = parse_hex_color(hex) {
                        self.colors[i] = rgb;
                        self.defaults[i] = rgb;
                    }
                }
            }
        }
        // Bright 8-15
        for (key, hex) in &colors.bright {
            if let Ok(i) = key.parse::<usize>() {
                if i < 8 {
                    if let Some(rgb) = parse_hex_color(hex) {
                        self.colors[8 + i] = rgb;
                        self.defaults[8 + i] = rgb;
                    }
                }
            }
        }
        // Selection colors
        self.selection_fg = colors
            .selection_foreground
            .as_deref()
            .and_then(parse_hex_color);
        self.selection_bg = colors
            .selection_background
            .as_deref()
            .and_then(parse_hex_color);
        // Recalculate dim variants for overridden ANSI 0-7
        for i in 0..8 {
            let base = self.colors[i];
            self.colors[NamedColor::DimBlack as usize + i] = dim_color(base);
        }
        self.colors[NamedColor::DimForeground as usize] =
            dim_color(self.colors[NamedColor::Foreground as usize]);
        // Update bright foreground if fg was overridden
        self.colors[NamedColor::BrightForeground as usize] =
            self.colors[NamedColor::Foreground as usize];
    }

    /// Return selection colors. Falls back to FG/BG swap when not configured.
    pub fn selection_colors(&self, fg: Rgb, bg: Rgb) -> (Rgb, Rgb) {
        (
            self.selection_fg.unwrap_or(bg),
            self.selection_bg.unwrap_or(fg),
        )
    }

    pub fn resolve(&self, color: Color, flags: CellFlags) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => self.colors[idx as usize],
            Color::Named(name) => {
                let idx = name as usize;
                if idx < NUM_COLORS {
                    // Bold-as-bright: for standard colors 0-7, promote to 8-15
                    if self.bold_is_bright && flags.contains(CellFlags::BOLD) && idx < 8 {
                        self.colors[idx + 8]
                    } else {
                        self.colors[idx]
                    }
                } else {
                    self.colors[NamedColor::Foreground as usize]
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

/// Parse "#RRGGBB" or "#RGB" to Rgb. Returns None on invalid input.
pub fn parse_hex_color(s: &str) -> Option<Rgb> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    let bytes = hex.as_bytes();
    match bytes.len() {
        6 => {
            let r = u8::from_str_radix(std::str::from_utf8(&bytes[0..2]).ok()?, 16).ok()?;
            let g = u8::from_str_radix(std::str::from_utf8(&bytes[2..4]).ok()?, 16).ok()?;
            let b = u8::from_str_radix(std::str::from_utf8(&bytes[4..6]).ok()?, 16).ok()?;
            Some(Rgb { r, g, b })
        }
        3 => {
            let r = u8::from_str_radix(std::str::from_utf8(&bytes[0..1]).ok()?, 16).ok()?;
            let g = u8::from_str_radix(std::str::from_utf8(&bytes[1..2]).ok()?, 16).ok()?;
            let b = u8::from_str_radix(std::str::from_utf8(&bytes[2..3]).ok()?, 16).ok()?;
            Some(Rgb {
                r: r * 17,
                g: g * 17,
                b: b * 17,
            })
        }
        _ => None,
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
        assert_eq!(p.colors[1], CATPPUCCIN_MOCHA.ansi[1]);
        // Check grayscale ramp
        assert_eq!(p.colors[232], Rgb { r: 8, g: 8, b: 8 });
        assert_eq!(
            p.colors[255],
            Rgb {
                r: 238,
                g: 238,
                b: 238
            }
        );
    }

    #[test]
    fn resolve_named() {
        let p = Palette::new();
        let rgb = p.resolve(Color::Named(NamedColor::Red), CellFlags::empty());
        assert_eq!(rgb, CATPPUCCIN_MOCHA.ansi[1]);
    }

    #[test]
    fn resolve_bold_bright() {
        let p = Palette::new();
        let normal = p.resolve(Color::Named(NamedColor::Black), CellFlags::empty());
        let bold = p.resolve(Color::Named(NamedColor::Black), CellFlags::BOLD);
        assert_eq!(normal, CATPPUCCIN_MOCHA.ansi[0]); // Black
        assert_eq!(bold, CATPPUCCIN_MOCHA.ansi[8]); // BrightBlack
    }

    #[test]
    fn resolve_bold_bright_disabled() {
        let mut p = Palette::new();
        p.bold_is_bright = false;
        let normal = p.resolve(Color::Named(NamedColor::Black), CellFlags::empty());
        let bold = p.resolve(Color::Named(NamedColor::Black), CellFlags::BOLD);
        // When disabled, bold should NOT promote to bright
        assert_eq!(normal, CATPPUCCIN_MOCHA.ansi[0]);
        assert_eq!(bold, CATPPUCCIN_MOCHA.ansi[0]);
    }

    #[test]
    fn set_scheme_changes_colors() {
        let mut p = Palette::new();
        assert_eq!(p.default_bg(), CATPPUCCIN_MOCHA.bg);

        p.set_scheme(&DRACULA);
        assert_eq!(p.default_bg(), DRACULA.bg);
        assert_eq!(p.default_fg(), DRACULA.fg);
        assert_eq!(p.colors[0], DRACULA.ansi[0]);
    }

    #[test]
    fn from_scheme_solarized() {
        let p = Palette::from_scheme(&SOLARIZED_DARK);
        assert_eq!(p.default_fg(), SOLARIZED_DARK.fg);
        assert_eq!(p.default_bg(), SOLARIZED_DARK.bg);
        assert_eq!(p.colors[4], SOLARIZED_DARK.ansi[4]); // Blue
    }

    #[test]
    fn resolve_spec() {
        let p = Palette::new();
        let rgb = p.resolve(
            Color::Spec(Rgb {
                r: 255,
                g: 0,
                b: 128,
            }),
            CellFlags::empty(),
        );
        assert_eq!(
            rgb,
            Rgb {
                r: 255,
                g: 0,
                b: 128
            }
        );
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
        assert_eq!(
            rgb_to_u32(Rgb {
                r: 0xFF,
                g: 0x00,
                b: 0x80
            }),
            0x00FF0080
        );
        assert_eq!(
            rgb_to_u32(Rgb {
                r: 0xCD,
                g: 0xD6,
                b: 0xF4
            }),
            0x00CDD6F4
        );
    }

    #[test]
    fn parse_hex_color_6char() {
        assert_eq!(
            parse_hex_color("#FF0080"),
            Some(Rgb {
                r: 255,
                g: 0,
                b: 128
            })
        );
        assert_eq!(parse_hex_color("#000000"), Some(Rgb { r: 0, g: 0, b: 0 }));
        assert_eq!(
            parse_hex_color("#ffffff"),
            Some(Rgb {
                r: 255,
                g: 255,
                b: 255
            })
        );
    }

    #[test]
    fn parse_hex_color_3char() {
        assert_eq!(
            parse_hex_color("#F80"),
            Some(Rgb {
                r: 255,
                g: 136,
                b: 0
            })
        );
        assert_eq!(parse_hex_color("#000"), Some(Rgb { r: 0, g: 0, b: 0 }));
        assert_eq!(
            parse_hex_color("#fff"),
            Some(Rgb {
                r: 255,
                g: 255,
                b: 255
            })
        );
    }

    #[test]
    fn parse_hex_color_no_hash() {
        assert_eq!(
            parse_hex_color("FF0080"),
            Some(Rgb {
                r: 255,
                g: 0,
                b: 128
            })
        );
        assert_eq!(
            parse_hex_color("abc"),
            Some(Rgb {
                r: 170,
                g: 187,
                b: 204
            })
        );
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#"), None);
        assert_eq!(parse_hex_color("#GG0000"), None);
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#1234567"), None);
    }

    #[test]
    fn apply_overrides_fg_bg_cursor() {
        let mut p = Palette::new();
        let colors = ColorConfig {
            foreground: Some("#FFFFFF".to_owned()),
            background: Some("#000000".to_owned()),
            cursor: Some("#FF0000".to_owned()),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        assert_eq!(
            p.default_fg(),
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(p.default_bg(), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(p.cursor_color(), Rgb { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn apply_overrides_ansi() {
        use std::collections::HashMap;
        let mut p = Palette::new();
        let colors = ColorConfig {
            ansi: HashMap::from([
                ("0".to_owned(), "#111111".to_owned()),
                ("7".to_owned(), "#EEEEEE".to_owned()),
            ]),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        assert_eq!(
            p.colors[0],
            Rgb {
                r: 0x11,
                g: 0x11,
                b: 0x11
            }
        );
        // Index 1 should still be the scheme default
        assert_eq!(p.colors[1], CATPPUCCIN_MOCHA.ansi[1]);
        assert_eq!(
            p.colors[7],
            Rgb {
                r: 0xEE,
                g: 0xEE,
                b: 0xEE
            }
        );
    }

    #[test]
    fn apply_overrides_bright() {
        use std::collections::HashMap;
        let mut p = Palette::new();
        let colors = ColorConfig {
            bright: HashMap::from([("1".to_owned(), "#FF0000".to_owned())]),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        assert_eq!(p.colors[9], Rgb { r: 255, g: 0, b: 0 }); // bright red
        assert_eq!(p.colors[8], CATPPUCCIN_MOCHA.ansi[8]); // bright black unchanged
    }

    #[test]
    fn apply_overrides_selection_colors() {
        let mut p = Palette::new();
        let colors = ColorConfig {
            selection_foreground: Some("#FFFFFF".to_owned()),
            selection_background: Some("#3A3D5C".to_owned()),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        let fg = Rgb {
            r: 100,
            g: 100,
            b: 100,
        };
        let bg = Rgb {
            r: 50,
            g: 50,
            b: 50,
        };
        let (sel_fg, sel_bg) = p.selection_colors(fg, bg);
        assert_eq!(
            sel_fg,
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(
            sel_bg,
            Rgb {
                r: 0x3A,
                g: 0x3D,
                b: 0x5C
            }
        );
    }

    #[test]
    fn selection_colors_default_swap() {
        let p = Palette::new();
        let fg = Rgb {
            r: 200,
            g: 200,
            b: 200,
        };
        let bg = Rgb {
            r: 30,
            g: 30,
            b: 30,
        };
        let (sel_fg, sel_bg) = p.selection_colors(fg, bg);
        // Default: swap fg and bg
        assert_eq!(sel_fg, bg);
        assert_eq!(sel_bg, fg);
    }

    #[test]
    fn apply_overrides_partial() {
        let mut p = Palette::new();
        let original_bg = p.default_bg();
        let colors = ColorConfig {
            foreground: Some("#AABBCC".to_owned()),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        assert_eq!(
            p.default_fg(),
            Rgb {
                r: 0xAA,
                g: 0xBB,
                b: 0xCC
            }
        );
        // BG should remain from the scheme
        assert_eq!(p.default_bg(), original_bg);
    }

    #[test]
    fn apply_overrides_recalculates_dim() {
        use std::collections::HashMap;
        let mut p = Palette::new();
        let colors = ColorConfig {
            ansi: HashMap::from([("0".to_owned(), "#FFFFFF".to_owned())]),
            ..ColorConfig::default()
        };
        p.apply_overrides(&colors);
        // Dim black should be 2/3 of overridden black (#FFFFFF)
        let expected_dim = dim_color(Rgb {
            r: 255,
            g: 255,
            b: 255,
        });
        assert_eq!(p.colors[NamedColor::DimBlack as usize], expected_dim);
    }
}
