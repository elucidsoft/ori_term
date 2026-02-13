//! Palette unit tests.

use std::collections::HashMap;

use vte::ansi::{Color, NamedColor, Rgb};

use crate::cell::CellFlags;
use crate::config::ColorConfig;

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
