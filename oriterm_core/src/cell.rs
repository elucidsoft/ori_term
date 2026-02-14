//! Terminal cell types.
//!
//! A `Cell` represents one character position in the terminal grid. Most cells
//! are 24 bytes on the stack. Only cells with combining marks, colored
//! underlines, or hyperlinks allocate heap storage via `CellExtra`.

use std::fmt;

use bitflags::bitflags;
use unicode_width::UnicodeWidthChar;
use vte::ansi::Color;

bitflags! {
    /// Per-cell attribute flags (SGR and internal).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CellFlags: u16 {
        const BOLD              = 1 << 0;
        const DIM               = 1 << 1;
        const ITALIC            = 1 << 2;
        const UNDERLINE         = 1 << 3;
        const BLINK             = 1 << 4;
        const INVERSE           = 1 << 5;
        const HIDDEN            = 1 << 6;
        const STRIKETHROUGH     = 1 << 7;
        const WIDE_CHAR         = 1 << 8;
        const WIDE_CHAR_SPACER  = 1 << 9;
        const WRAP              = 1 << 10;
        const CURLY_UNDERLINE   = 1 << 11;
        const DOTTED_UNDERLINE  = 1 << 12;
        const DASHED_UNDERLINE  = 1 << 13;
        const DOUBLE_UNDERLINE  = 1 << 14;
    }
}

impl Default for CellFlags {
    fn default() -> Self {
        Self::empty()
    }
}

/// Heap-allocated optional data for cells that need it.
///
/// Only allocated when a cell has combining marks, a colored underline,
/// or a hyperlink. Normal cells keep `extra: None` (zero overhead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellExtra {
    /// Colored underline (SGR 58).
    pub underline_color: Option<Color>,
    /// OSC 8 hyperlink.
    pub hyperlink: Option<Hyperlink>,
    /// Combining marks and zero-width characters appended to this cell.
    pub zerowidth: Vec<char>,
}

impl CellExtra {
    /// Create an empty extra with all fields at their defaults.
    pub fn new() -> Self {
        Self {
            underline_color: None,
            hyperlink: None,
            zerowidth: Vec::new(),
        }
    }
}

impl Default for CellExtra {
    fn default() -> Self {
        Self::new()
    }
}

/// OSC 8 hyperlink data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    /// Optional link id for grouping.
    pub id: Option<String>,
    /// The URI target.
    pub uri: String,
}

impl fmt::Display for Hyperlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri)
    }
}

/// One character position in the terminal grid.
///
/// Target size: 24 bytes. Fields are ordered to minimize padding:
/// `char(4) + Color(4) + Color(4) + CellFlags(2) + pad(2) + Option<Box>(8)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character stored in this cell.
    pub ch: char,
    /// Foreground color (deferred palette resolution).
    pub fg: Color,
    /// Background color (deferred palette resolution).
    pub bg: Color,
    /// SGR attribute flags.
    pub flags: CellFlags,
    /// Optional heap data for combining marks, underline color, or hyperlinks.
    pub extra: Option<Box<CellExtra>>,
}

const _: () = assert!(size_of::<Cell>() <= 24);

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Named(vte::ansi::NamedColor::Foreground),
            bg: Color::Named(vte::ansi::NamedColor::Background),
            flags: CellFlags::empty(),
            extra: None,
        }
    }
}

impl Cell {
    /// Reset this cell to match the given template.
    pub fn reset(&mut self, template: &Self) {
        self.ch = template.ch;
        self.fg = template.fg;
        self.bg = template.bg;
        self.flags = template.flags;
        self.extra.clone_from(&template.extra);
    }

    /// Returns `true` if this cell is visually empty (space, default colors, no flags).
    pub fn is_empty(&self) -> bool {
        self.ch == ' '
            && self.fg == Color::Named(vte::ansi::NamedColor::Foreground)
            && self.bg == Color::Named(vte::ansi::NamedColor::Background)
            && self.flags.is_empty()
            && self.extra.is_none()
    }

    /// Display width of this cell's character.
    ///
    /// Respects the `WIDE_CHAR` flag and falls back to `unicode-width`.
    pub fn width(&self) -> usize {
        if self.flags.contains(CellFlags::WIDE_CHAR) {
            return 2;
        }
        if self.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            return 0;
        }
        UnicodeWidthChar::width(self.ch).unwrap_or(1)
    }

    /// Append a combining mark (zero-width character) to this cell.
    ///
    /// Lazily allocates `CellExtra` on first combining mark.
    pub fn push_zerowidth(&mut self, ch: char) {
        let extra = self.extra.get_or_insert_with(|| Box::new(CellExtra::new()));
        extra.zerowidth.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use vte::ansi::{Color, NamedColor};

    use super::{Cell, CellExtra, CellFlags, Hyperlink};

    #[test]
    fn size_assertion() {
        assert!(
            size_of::<Cell>() <= 24,
            "Cell is {} bytes, expected <= 24",
            size_of::<Cell>()
        );
    }

    #[test]
    fn default_cell_is_space_with_default_colors() {
        let cell = Cell::default();
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, Color::Named(NamedColor::Foreground));
        assert_eq!(cell.bg, Color::Named(NamedColor::Background));
        assert!(cell.flags.is_empty());
        assert!(cell.extra.is_none());
    }

    #[test]
    fn reset_clears_to_template() {
        let mut cell = Cell::default();
        cell.ch = 'X';
        cell.flags = CellFlags::BOLD;

        let template = Cell::default();
        cell.reset(&template);

        assert_eq!(cell.ch, ' ');
        assert!(cell.flags.is_empty());
    }

    #[test]
    fn is_empty_for_default() {
        assert!(Cell::default().is_empty());
    }

    #[test]
    fn is_empty_false_after_setting_char() {
        let mut cell = Cell::default();
        cell.ch = 'A';
        assert!(!cell.is_empty());
    }

    #[test]
    fn wide_char_width() {
        let mut cell = Cell::default();
        cell.ch = '好';
        cell.flags = CellFlags::WIDE_CHAR;
        assert_eq!(cell.width(), 2);
    }

    #[test]
    fn spacer_width() {
        let mut cell = Cell::default();
        cell.flags = CellFlags::WIDE_CHAR_SPACER;
        assert_eq!(cell.width(), 0);
    }

    #[test]
    fn normal_char_width() {
        let mut cell = Cell::default();
        cell.ch = 'A';
        assert_eq!(cell.width(), 1);
    }

    #[test]
    fn extra_is_none_for_normal_cells() {
        let cell = Cell::default();
        assert!(cell.extra.is_none());
    }

    #[test]
    fn extra_created_for_underline_color() {
        let mut cell = Cell::default();
        cell.extra = Some(Box::new(CellExtra {
            underline_color: Some(Color::Spec(vte::ansi::Rgb { r: 255, g: 0, b: 0 })),
            hyperlink: None,
            zerowidth: Vec::new(),
        }));
        assert!(cell.extra.is_some());
        assert_eq!(
            cell.extra.as_ref().unwrap().underline_color,
            Some(Color::Spec(vte::ansi::Rgb { r: 255, g: 0, b: 0 }))
        );
    }

    #[test]
    fn extra_created_for_hyperlink() {
        let mut cell = Cell::default();
        cell.extra = Some(Box::new(CellExtra {
            underline_color: None,
            hyperlink: Some(Hyperlink {
                id: None,
                uri: "https://example.com".to_string(),
            }),
            zerowidth: Vec::new(),
        }));
        assert!(cell.extra.is_some());
    }

    #[test]
    fn push_zerowidth_creates_extra() {
        let mut cell = Cell::default();
        assert!(cell.extra.is_none());

        // U+0301 COMBINING ACUTE ACCENT.
        cell.push_zerowidth('\u{0301}');

        assert!(cell.extra.is_some());
        assert_eq!(cell.extra.as_ref().unwrap().zerowidth, vec!['\u{0301}']);
    }

    #[test]
    fn cellflags_set_clear_query() {
        let mut flags = CellFlags::empty();
        assert!(!flags.contains(CellFlags::BOLD));

        flags |= CellFlags::BOLD;
        assert!(flags.contains(CellFlags::BOLD));

        flags &= !CellFlags::BOLD;
        assert!(!flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn cellflags_combine() {
        let flags = CellFlags::BOLD | CellFlags::ITALIC | CellFlags::UNDERLINE;
        assert!(flags.contains(CellFlags::BOLD));
        assert!(flags.contains(CellFlags::ITALIC));
        assert!(flags.contains(CellFlags::UNDERLINE));
        assert!(!flags.contains(CellFlags::DIM));
    }
}
