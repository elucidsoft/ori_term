//! Built-in color scheme definitions.

use vte::ansi::Rgb;

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
