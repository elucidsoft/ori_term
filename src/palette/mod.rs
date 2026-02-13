//! Color palette management — 270-entry RGB table plus color scheme definitions.

mod schemes;

pub use schemes::{
    ColorScheme, find_scheme, BUILTIN_SCHEMES, CATPPUCCIN_LATTE, CATPPUCCIN_MOCHA, DRACULA,
    ONE_DARK, SOLARIZED_DARK, SOLARIZED_LIGHT, TOKYO_NIGHT, WEZTERM_DEFAULT,
};

use vte::ansi::{Color, NamedColor, Rgb};

use crate::cell::CellFlags;
use crate::config::ColorConfig;

pub const NUM_COLORS: usize = 270;

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
mod tests;
