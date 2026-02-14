//! Terminal cell types.
//!
//! A `Cell` represents one character position in the terminal grid. Most cells
//! are 24 bytes on the stack. Only cells with combining marks, colored
//! underlines, or hyperlinks allocate heap storage via `CellExtra`. Extra data
//! is stored behind `Arc` so cloning a cell (e.g. from cursor template) is O(1).

use std::fmt;
use std::sync::Arc;

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
/// `char(4) + Color(4) + Color(4) + CellFlags(2) + pad(2) + Option<Arc>(8)`.
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
    ///
    /// Uses `Arc` so that cloning a cell with extra data (e.g. propagating
    /// cursor template attributes) is O(1) — a refcount bump instead of a
    /// heap allocation.
    pub extra: Option<Arc<CellExtra>>,
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

impl From<Color> for Cell {
    /// BCE (Background Color Erase) cell: default cell with a custom background.
    fn from(bg: Color) -> Self {
        Self { bg, ..Self::default() }
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
    /// Lazily allocates `CellExtra` on first combining mark. Uses
    /// `Arc::make_mut` for copy-on-write when the extra is shared.
    pub fn push_zerowidth(&mut self, ch: char) {
        let extra = self.extra.get_or_insert_with(Default::default);
        Arc::make_mut(extra).zerowidth.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
        cell.extra = Some(Arc::new(CellExtra {
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
        cell.extra = Some(Arc::new(CellExtra {
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

    // --- Additional tests from reference repo gap analysis ---

    #[test]
    fn from_color_creates_bce_cell() {
        let color = Color::Indexed(1);
        let cell = Cell::from(color);
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.bg, color);
        assert_eq!(cell.fg, Color::Named(NamedColor::Foreground));
        assert!(cell.flags.is_empty());
        assert!(cell.extra.is_none());
    }

    #[test]
    fn is_empty_false_for_non_default_bg() {
        let mut cell = Cell::default();
        cell.bg = Color::Indexed(1);
        assert!(!cell.is_empty());
    }

    #[test]
    fn is_empty_false_for_flags() {
        let mut cell = Cell::default();
        cell.flags = CellFlags::BOLD;
        assert!(!cell.is_empty());
    }

    #[test]
    fn is_empty_false_for_extra() {
        let mut cell = Cell::default();
        cell.push_zerowidth('\u{0301}');
        assert!(!cell.is_empty());
    }

    #[test]
    fn width_cjk_ideographic_space() {
        // U+3000 IDEOGRAPHIC SPACE — width 2 (wezterm issue_1161).
        let mut cell = Cell::default();
        cell.ch = '\u{3000}';
        cell.flags = CellFlags::WIDE_CHAR;
        assert_eq!(cell.width(), 2);
    }

    #[test]
    fn width_emoji() {
        // Emoji crab — width 2 via unicode-width when WIDE_CHAR flag set.
        let mut cell = Cell::default();
        cell.ch = '🦀';
        cell.flags = CellFlags::WIDE_CHAR;
        assert_eq!(cell.width(), 2);
    }

    #[test]
    fn push_zerowidth_multiple_marks() {
        let mut cell = Cell::default();
        cell.ch = 'e';
        cell.push_zerowidth('\u{0301}'); // COMBINING ACUTE ACCENT
        cell.push_zerowidth('\u{0327}'); // COMBINING CEDILLA
        let zw = &cell.extra.as_ref().unwrap().zerowidth;
        assert_eq!(zw.len(), 2);
        assert_eq!(zw[0], '\u{0301}');
        assert_eq!(zw[1], '\u{0327}');
    }

    #[test]
    fn clone_shares_arc_refcount() {
        let mut cell = Cell::default();
        cell.push_zerowidth('\u{0301}');
        let cloned = cell.clone();
        // Both cells should share the same Arc allocation.
        assert!(Arc::ptr_eq(
            cell.extra.as_ref().unwrap(),
            cloned.extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn push_zerowidth_cow_on_shared_arc() {
        let mut cell = Cell::default();
        cell.push_zerowidth('\u{0301}');
        let original = cell.clone();
        // Mutating cell's extra triggers COW — original stays unchanged.
        cell.push_zerowidth('\u{0327}');
        assert_eq!(original.extra.as_ref().unwrap().zerowidth.len(), 1);
        assert_eq!(cell.extra.as_ref().unwrap().zerowidth.len(), 2);
    }

    #[test]
    fn reset_copies_template_extra() {
        let mut cell = Cell::default();
        cell.ch = 'X';
        let mut template = Cell::default();
        template.extra = Some(Arc::new(CellExtra {
            underline_color: Some(Color::Spec(vte::ansi::Rgb { r: 0, g: 255, b: 0 })),
            hyperlink: None,
            zerowidth: Vec::new(),
        }));
        cell.reset(&template);
        assert!(cell.extra.is_some());
        assert_eq!(
            cell.extra.as_ref().unwrap().underline_color,
            Some(Color::Spec(vte::ansi::Rgb { r: 0, g: 255, b: 0 }))
        );
    }

    #[test]
    fn reset_clears_extra_when_template_has_none() {
        let mut cell = Cell::default();
        cell.push_zerowidth('\u{0301}');
        assert!(cell.extra.is_some());
        cell.reset(&Cell::default());
        assert!(cell.extra.is_none());
    }

    #[test]
    fn hyperlink_display() {
        let link = Hyperlink {
            id: Some("id1".to_string()),
            uri: "https://example.com".to_string(),
        };
        assert_eq!(format!("{link}"), "https://example.com");
    }
}
