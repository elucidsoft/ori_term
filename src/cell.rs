//! Terminal grid cell representation with attributes and flags.

use std::sync::Arc;

use bitflags::bitflags;
use vte::ansi::{Color, Hyperlink, Rgb};

bitflags! {
    /// Bitflags for cell text attributes and layout hints.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct CellFlags: u16 {
        const BOLD                    = 0b0000_0000_0001;
        const DIM                     = 0b0000_0000_0010;
        const ITALIC                  = 0b0000_0000_0100;
        const UNDERLINE               = 0b0000_0000_1000;
        const DOUBLE_UNDERLINE        = 0b0000_0001_0000;
        const UNDERCURL               = 0b0000_0010_0000;
        const DOTTED_UNDERLINE        = 0b0000_0100_0000;
        const DASHED_UNDERLINE        = 0b0000_1000_0000;
        const BLINK                   = 0b0001_0000_0000;
        const INVERSE                 = 0b0010_0000_0000;
        const HIDDEN                  = 0b0100_0000_0000;
        const STRIKEOUT               = 0b1000_0000_0000;
        const WIDE_CHAR               = 0b0001_0000_0000_0000;
        const WIDE_CHAR_SPACER        = 0b0010_0000_0000_0000;
        const WRAPLINE                = 0b0100_0000_0000_0000;
        const LEADING_WIDE_CHAR_SPACER = 0b1000_0000_0000_0000;
    }
}

impl CellFlags {
    /// Combined mask for all underline variants.
    pub const ANY_UNDERLINE: Self = Self::UNDERLINE
        .union(Self::DOUBLE_UNDERLINE)
        .union(Self::UNDERCURL)
        .union(Self::DOTTED_UNDERLINE)
        .union(Self::DASHED_UNDERLINE);
}

/// Extended cell data stored out-of-line (combining marks, hyperlinks, custom underline color).
#[derive(Debug, PartialEq, Eq, Default)]
pub struct CellExtra {
    pub zerowidth: Vec<char>,
    pub underline_color: Option<Color>,
    pub hyperlink: Option<Hyperlink>,
}

impl Clone for CellExtra {
    fn clone(&self) -> Self {
        Self {
            zerowidth: self.zerowidth.clone(),
            underline_color: self.underline_color,
            hyperlink: self.hyperlink.as_ref().map(|h| Hyperlink {
                id: h.id.clone(),
                uri: h.uri.clone(),
            }),
        }
    }
}

/// A single grid cell with character, colors, attributes, and optional extended data.
#[derive(Debug, Clone)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
    pub extra: Option<Arc<CellExtra>>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Named(vte::ansi::NamedColor::Foreground),
            bg: Color::Named(vte::ansi::NamedColor::Background),
            flags: CellFlags::empty(),
            extra: None,
        }
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.c == other.c && self.fg == other.fg && self.bg == other.bg && self.flags == other.flags
    }
}

impl Cell {
    // Accessors

    /// Returns the zero-width combining characters for this cell.
    pub fn zerowidth(&self) -> &[char] {
        match &self.extra {
            Some(extra) => &extra.zerowidth,
            None => &[],
        }
    }

    /// Returns the custom underline color if set.
    pub fn underline_color(&self) -> Option<Color> {
        self.extra.as_ref().and_then(|e| e.underline_color)
    }

    /// Returns the hyperlink associated with this cell.
    pub fn hyperlink(&self) -> Option<&Hyperlink> {
        self.extra.as_ref().and_then(|e| e.hyperlink.as_ref())
    }

    // Operations

    /// Resets this cell to match the template, preserving character layout flags.
    pub fn reset(&mut self, template: &Self) {
        self.c = template.c;
        self.fg = template.fg;
        self.bg = template.bg;
        self.flags = template.flags
            & !(CellFlags::WIDE_CHAR
                | CellFlags::WIDE_CHAR_SPACER
                | CellFlags::WRAPLINE
                | CellFlags::LEADING_WIDE_CHAR_SPACER);
        self.extra = None;
    }

    /// Adds a zero-width combining character to this cell.
    pub fn push_zerowidth(&mut self, c: char) {
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).zerowidth.push(c);
    }

    /// Sets the custom underline color for this cell.
    pub fn set_underline_color(&mut self, color: Option<Color>) {
        if color.is_none() && self.extra.is_none() {
            return;
        }
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).underline_color = color;
    }

    /// Sets the hyperlink for this cell.
    pub fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        if hyperlink.is_none() && self.extra.is_none() {
            return;
        }
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).hyperlink = hyperlink;
    }

    // Conversion

    /// Converts a Color to an RGB triple, using fallback values for indexed/named colors.
    pub fn to_rgb(color: Color) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => Rgb {
                r: idx,
                g: idx,
                b: idx,
            },
            Color::Named(_) => Rgb { r: 0, g: 0, b: 0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn cell_size() {
        // Cell should be reasonably compact. With Option<Arc<CellExtra>> it's:
        // char(4) + Color(4) + Color(4) + CellFlags(2) + padding(2) + Option<Arc>(8) = 24
        assert!(
            size_of::<Cell>() <= 32,
            "Cell is {} bytes",
            size_of::<Cell>()
        );
    }

    #[test]
    fn cell_default() {
        let cell = Cell::default();
        assert_eq!(cell.c, ' ');
        assert_eq!(cell.fg, Color::Named(vte::ansi::NamedColor::Foreground));
        assert_eq!(cell.bg, Color::Named(vte::ansi::NamedColor::Background));
        assert!(cell.flags.is_empty());
        assert!(cell.extra.is_none());
    }

    #[test]
    fn cell_zerowidth() {
        let mut cell = Cell::default();
        assert!(cell.zerowidth().is_empty());
        cell.push_zerowidth('\u{0300}'); // combining grave accent
        assert_eq!(cell.zerowidth(), &['\u{0300}']);
    }

    #[test]
    fn cell_reset() {
        let mut cell = Cell::default();
        cell.c = 'A';
        cell.fg = Color::Spec(Rgb { r: 255, g: 0, b: 0 });
        cell.flags = CellFlags::BOLD | CellFlags::WIDE_CHAR;
        cell.push_zerowidth('\u{0300}');

        let template = Cell::default();
        cell.reset(&template);
        assert_eq!(cell.c, ' ');
        assert_eq!(cell.fg, Color::Named(vte::ansi::NamedColor::Foreground));
        assert!(!cell.flags.contains(CellFlags::WIDE_CHAR));
        assert!(cell.extra.is_none());
    }
}
