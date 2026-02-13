//! Tests for selection types, boundaries, and text extraction.

use crate::grid::{Grid, StableRowIndex};

use super::*;

/// Helper to create a `StableRowIndex` from a raw value for tests.
fn sri(n: u64) -> StableRowIndex {
    StableRowIndex(n)
}

#[test]
fn selection_point_ordering() {
    let a = SelectionPoint {
        row: sri(0),
        col: 5,
        side: Side::Left,
    };
    let b = SelectionPoint {
        row: sri(0),
        col: 5,
        side: Side::Right,
    };
    let c = SelectionPoint {
        row: sri(1),
        col: 0,
        side: Side::Left,
    };
    assert!(a < b);
    assert!(b < c);
    assert!(a < c);
}

#[test]
fn selection_contains_single_row() {
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(5),
            col: 2,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(5),
            col: 2,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(5),
            col: 8,
            side: Side::Right,
        },
    };
    assert!(!sel.contains(sri(5), 1));
    assert!(sel.contains(sri(5), 2));
    assert!(sel.contains(sri(5), 5));
    assert!(sel.contains(sri(5), 8));
    assert!(!sel.contains(sri(5), 9));
    assert!(!sel.contains(sri(4), 5));
    assert!(!sel.contains(sri(6), 5));
}

#[test]
fn selection_contains_multi_row() {
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(4),
            col: 3,
            side: Side::Right,
        },
    };
    // Row 2: col >= 5
    assert!(!sel.contains(sri(2), 4));
    assert!(sel.contains(sri(2), 5));
    assert!(sel.contains(sri(2), 100));
    // Row 3: fully selected
    assert!(sel.contains(sri(3), 0));
    assert!(sel.contains(sri(3), 100));
    // Row 4: col <= 3
    assert!(sel.contains(sri(4), 0));
    assert!(sel.contains(sri(4), 3));
    assert!(!sel.contains(sri(4), 4));
}

#[test]
fn selection_empty() {
    let sel = Selection::new_char(sri(5), 10, Side::Left);
    assert!(sel.is_empty());

    let mut sel2 = Selection::new_char(sri(5), 10, Side::Left);
    sel2.end.col = 12;
    assert!(!sel2.is_empty());
}

#[test]
fn block_selection_contains() {
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(2),
            col: 3,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(2),
            col: 3,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(5),
            col: 7,
            side: Side::Right,
        },
    };
    assert!(sel.contains(sri(3), 5));
    assert!(!sel.contains(sri(3), 2));
    assert!(!sel.contains(sri(3), 8));
    assert!(!sel.contains(sri(1), 5));
    assert!(!sel.contains(sri(6), 5));
}

#[test]
fn word_boundaries_on_grid() {
    let mut grid = Grid::new(20, 1);
    // Write "hello world" into row 0
    for (i, c) in "hello world".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }
    // Test word boundary for 'e' (col 1)
    let (s, e) = word_boundaries(&grid, 0, 1);
    assert_eq!(s, 0);
    assert_eq!(e, 4);
    // Test word boundary for 'w' (col 6)
    let (s, e) = word_boundaries(&grid, 0, 6);
    assert_eq!(s, 6);
    assert_eq!(e, 10);
    // Test word boundary for space (col 5)
    let (s, e) = word_boundaries(&grid, 0, 5);
    assert_eq!(s, 5);
    assert_eq!(e, 5);
}

#[test]
fn word_boundaries_wide_char() {
    // Grid: "漢字 test" = [漢, spacer, 字, spacer, space, t, e, s, t]
    // CJK characters are alphanumeric in Unicode, so they group with ASCII
    // as word chars (class 0). A space separates the two word groups.
    let mut grid = Grid::new(20, 1);
    grid.put_wide_char('漢');
    grid.put_wide_char('字');
    grid.put_char(' ');
    for c in "test".chars() {
        grid.put_char(c);
    }
    // Double-click on 漢 (col 0): should select "漢字" (cols 0-3)
    let (s, e) = word_boundaries(&grid, 0, 0);
    assert_eq!(s, 0);
    assert_eq!(e, 3); // includes both wide chars + spacers

    // Double-click on spacer of 漢 (col 1): same result
    let (s, e) = word_boundaries(&grid, 0, 1);
    assert_eq!(s, 0);
    assert_eq!(e, 3);

    // Double-click on 't' (col 5): should select "test" (cols 5-8)
    let (s, e) = word_boundaries(&grid, 0, 5);
    assert_eq!(s, 5);
    assert_eq!(e, 8);
}

#[test]
fn word_boundaries_single_wide_char() {
    // Grid: "漢 A" = [漢, spacer, space, A]
    let mut grid = Grid::new(20, 1);
    grid.put_wide_char('漢');
    grid.put_char(' ');
    grid.put_char('A');

    // Double-click on 漢: should select just 漢 (cols 0-1)
    let (s, e) = word_boundaries(&grid, 0, 0);
    assert_eq!(s, 0);
    assert_eq!(e, 1); // wide char + its spacer
}

#[test]
fn extract_text_with_zerowidth() {
    let mut grid = Grid::new(10, 1);
    grid.put_char('e');
    // Attach combining acute accent
    grid.row_mut(0)[0].push_zerowidth('\u{0301}');
    grid.put_char('x');

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 1,
            side: Side::Right,
        },
    };
    let text = extract_text(&grid, &sel);
    assert_eq!(text, "e\u{0301}x");
}

#[test]
fn extract_text_simple() {
    let mut grid = Grid::new(10, 2);
    for (i, c) in "Hello".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }
    for (i, c) in "World".chars().enumerate() {
        grid.goto(1, i);
        grid.put_char(c);
    }

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 4,
            side: Side::Right,
        },
    };
    let text = extract_text(&grid, &sel);
    assert_eq!(text, "Hello\nWorld");
}
