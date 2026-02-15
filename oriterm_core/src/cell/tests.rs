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
    cell.ch = '\u{597d}';
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
    cell.ch = '\u{1f980}';
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
