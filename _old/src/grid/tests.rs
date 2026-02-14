//! Grid unit tests.

use vte::ansi::ClearMode;

use crate::cell::CellFlags;

use super::*;

#[test]
fn new_grid() {
    let g = Grid::new(80, 24);
    assert_eq!(g.cols, 80);
    assert_eq!(g.lines, 24);
    assert_eq!(g.cursor.col, 0);
    assert_eq!(g.cursor.row, 0);
}

#[test]
fn put_char_advances_cursor() {
    let mut g = Grid::new(80, 24);
    g.put_char('A');
    assert_eq!(g.cursor.col, 1);
    assert_eq!(g.row(0)[0].c, 'A');
}

#[test]
fn wrap_at_end_of_line() {
    let mut g = Grid::new(5, 3);
    for c in "hello".chars() {
        g.put_char(c);
    }
    // After 5 chars in 5-col grid, cursor is at col 4 with wrap pending
    assert!(g.cursor.input_needs_wrap);
    assert_eq!(g.cursor.col, 4);

    g.put_char('!');
    assert_eq!(g.cursor.row, 1);
    assert_eq!(g.cursor.col, 1);
    assert_eq!(g.row(1)[0].c, '!');
}

#[test]
fn scroll_up_pushes_to_scrollback() {
    let mut g = Grid::new(5, 3);
    // Fill line 0
    for c in "ABCDE".chars() {
        g.put_char(c);
    }
    g.newline();
    g.carriage_return();
    // Fill line 1
    for c in "FGHIJ".chars() {
        g.put_char(c);
    }
    g.newline();
    g.carriage_return();
    // Fill line 2
    for c in "KLMNO".chars() {
        g.put_char(c);
    }
    // Now newline should scroll
    g.newline();
    assert_eq!(g.scrollback.len(), 1);
    assert_eq!(g.scrollback[0][0].c, 'A');
}

#[test]
fn erase_display_below() {
    let mut g = Grid::new(10, 5);
    for c in "Hello".chars() {
        g.put_char(c);
    }
    g.cursor.col = 2;
    g.erase_display(ClearMode::Below);
    assert_eq!(g.row(0)[0].c, 'H');
    assert_eq!(g.row(0)[1].c, 'e');
    assert_eq!(g.row(0)[2].c, ' ');
}

#[test]
fn tab_stops() {
    let g = Grid::new(80, 24);
    assert!(g.tab_stops[8]);
    assert!(g.tab_stops[16]);
    assert!(!g.tab_stops[0]);
    assert!(!g.tab_stops[7]);
}

#[test]
fn advance_tab() {
    let mut g = Grid::new(80, 24);
    g.cursor.col = 0;
    g.advance_tab(1);
    assert_eq!(g.cursor.col, 8);
    g.advance_tab(1);
    assert_eq!(g.cursor.col, 16);
}

#[test]
fn scroll_region() {
    let mut g = Grid::new(10, 5);
    g.set_scroll_region(1, Some(3));
    // Put content in row 1
    g.cursor.row = 1;
    for c in "AAAAAAAAAA".chars() {
        g.put_char(c);
    }
    g.cursor.input_needs_wrap = false;
    g.cursor.row = 2;
    g.cursor.col = 0;
    for c in "BBBBBBBBBB".chars() {
        g.put_char(c);
    }
    g.cursor.input_needs_wrap = false;
    g.cursor.row = 3;
    g.cursor.col = 0;
    for c in "CCCCCCCCCC".chars() {
        g.put_char(c);
    }

    g.scroll_up_in_region(1, 3, 1);
    // Row 1 should now be B, row 2 should be C, row 3 should be blank
    assert_eq!(g.row(1)[0].c, 'B');
    assert_eq!(g.row(2)[0].c, 'C');
    assert_eq!(g.row(3)[0].c, ' ');
}

#[test]
fn resize_grow_cols() {
    let mut g = Grid::new(10, 5);
    g.put_char('A');
    g.resize(20, 5, false);
    assert_eq!(g.cols, 20);
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0).len(), 20);
}

#[test]
fn resize_shrink_rows() {
    let mut g = Grid::new(10, 5);
    g.cursor.row = 4;
    g.cursor.col = 0;
    g.put_char('X');
    g.resize(10, 3, false);
    assert_eq!(g.lines, 3);
    // Cursor should be clamped
    assert!(g.cursor.row < 3);
}

#[test]
fn insert_blank_chars() {
    let mut g = Grid::new(10, 1);
    for c in "ABCDE".chars() {
        g.put_char(c);
    }
    g.cursor.col = 1;
    g.insert_blank_chars(2);
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0)[1].c, ' ');
    assert_eq!(g.row(0)[2].c, ' ');
    assert_eq!(g.row(0)[3].c, 'B');
}

#[test]
fn delete_chars() {
    let mut g = Grid::new(10, 1);
    for c in "ABCDE".chars() {
        g.put_char(c);
    }
    g.cursor.col = 1;
    g.delete_chars(2);
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0)[1].c, 'D');
    assert_eq!(g.row(0)[2].c, 'E');
}

#[test]
fn reflow_shrink_wraps_long_line() {
    // Write "ABC" into a 3-col, 5-line grid, then shrink to 2 cols
    // Content: "ABC" (3 chars) wraps into "AB" + "C" = 2 rows
    // With 4 empty rows + 2 content rows = 6 total for 5 visible -> 1 to scrollback
    // Use a setup where we can verify both scrollback and visible content
    let mut g = Grid::new(4, 5);
    for c in "ABCD".chars() {
        g.put_char(c);
    }
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0)[3].c, 'D');

    g.resize(2, 5, true);
    // "ABCD" wraps to "AB" + "CD" = 2 rows, + 4 empty = 6 total
    // 6 - 5 = 1 row to scrollback
    assert_eq!(g.cols, 2);
    assert_eq!(g.scrollback.len(), 1);
    assert_eq!(g.scrollback[0][0].c, 'A');
    assert_eq!(g.scrollback[0][1].c, 'B');
    assert!(g.scrollback[0][1].flags.contains(CellFlags::WRAPLINE));
    assert_eq!(g.row(0)[0].c, 'C');
    assert_eq!(g.row(0)[1].c, 'D');
}

#[test]
fn reflow_grow_unwraps_line() {
    // Create a wrapped line by writing "ABCDEFGH" in 5 cols, then grow to 10
    let mut g = Grid::new(5, 3);
    for c in "ABCDEFGH".chars() {
        g.put_char(c);
    }
    // "ABCDE" on row 0 (wrapped), "FGH" on row 1
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(1)[0].c, 'F');
    assert!(g.row(0)[4].flags.contains(CellFlags::WRAPLINE));

    g.resize(10, 3, true);
    // Should have merged: "ABCDEFGH" on row 0
    assert_eq!(g.cols, 10);
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0)[5].c, 'F');
    assert_eq!(g.row(0)[7].c, 'H');
}

#[test]
fn reflow_roundtrip() {
    // Shrink then grow should restore content via scrollback merge
    let mut g = Grid::new(4, 5);
    for c in "ABCD".chars() {
        g.put_char(c);
    }

    g.resize(2, 5, true);
    // "ABCD" wraps to "AB" + "CD"
    // 1 row goes to scrollback
    assert_eq!(g.scrollback.len(), 1);
    assert_eq!(g.scrollback[0][0].c, 'A');
    assert_eq!(g.row(0)[0].c, 'C');

    g.resize(4, 5, true);
    // Grow should merge "AB" (scrollback, WRAPLINE) + "CD" (visible)
    // into "ABCD" on one row
    assert_eq!(g.row(0)[0].c, 'A');
    assert_eq!(g.row(0)[2].c, 'C');
    assert_eq!(g.row(0)[3].c, 'D');
    assert_eq!(g.scrollback.len(), 0);
}

#[test]
fn reflow_shrink_preserves_cursor() {
    let mut g = Grid::new(10, 5);
    for c in "ABCDEFGHIJ".chars() {
        g.put_char(c);
    }
    g.cursor.col = 7; // Position at 'H'
    g.cursor.input_needs_wrap = false;

    g.resize(5, 5, true);
    // 'H' is at index 7, which is in the second piece (col 2)
    assert_eq!(g.cursor.col, 2);
}

#[test]
fn reflow_shrink_overflow_to_scrollback() {
    // When shrink creates more rows than visible, extra goes to scrollback
    let mut g = Grid::new(10, 2);
    for c in "ABCDEFGHIJ".chars() {
        g.put_char(c);
    }
    g.newline();
    g.carriage_return();
    for c in "KLMNO".chars() {
        g.put_char(c);
    }

    g.resize(5, 2, true);
    // Row 0 ("ABCDEFGHIJ") wraps to "ABCDE" + "FGHIJ" = 2 rows
    // Row 1 ("KLMNO") stays as 1 row
    // Total 3 rows for 2 visible lines -> 1 goes to scrollback
    assert_eq!(g.scrollback.len(), 1);
    assert_eq!(g.scrollback[0][0].c, 'A');
}

#[test]
fn wide_char_occupies_two_cells() {
    let mut g = Grid::new(10, 1);
    g.put_wide_char('漢');
    assert_eq!(g.row(0)[0].c, '漢');
    assert!(g.row(0)[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(g.row(0)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert_eq!(g.cursor.col, 2);
}

#[test]
fn wide_char_at_end_of_line_wraps() {
    let mut g = Grid::new(5, 2);
    g.cursor.col = 4; // Last column
    g.put_wide_char('漢');
    // Should place LEADING_WIDE_CHAR_SPACER at col 4 and wrap
    assert!(
        g.row(0)[4]
            .flags
            .contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
    );
    assert_eq!(g.row(1)[0].c, '漢');
    assert!(g.row(1)[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(g.row(1)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
}

#[test]
fn overwrite_wide_char_clears_spacer() {
    let mut g = Grid::new(10, 1);
    g.put_wide_char('漢');
    g.cursor.col = 0;
    g.put_char('a');
    assert_eq!(g.row(0)[0].c, 'a');
    assert!(!g.row(0)[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(!g.row(0)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
}

#[test]
fn combining_mark_stored_in_cell() {
    let mut g = Grid::new(10, 1);
    g.put_char('e');
    // Attach combining acute accent to previous cell
    let col = g.cursor.col - 1;
    g.row_mut(0)[col].push_zerowidth('\u{0301}');
    assert_eq!(g.row(0)[0].c, 'e');
    assert_eq!(g.row(0)[0].zerowidth(), &['\u{0301}']);
}

#[test]
fn zerowidth_on_wide_char() {
    let mut g = Grid::new(10, 1);
    g.put_wide_char('漢');
    // Attach zerowidth to the base cell (not the spacer)
    g.row_mut(0)[0].push_zerowidth('\u{0301}');
    assert_eq!(g.row(0)[0].zerowidth(), &['\u{0301}']);
    assert!(g.row(0)[1].zerowidth().is_empty());
}
