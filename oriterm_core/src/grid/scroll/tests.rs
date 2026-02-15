use vte::ansi::Color;

use crate::grid::Grid;
use crate::index::{Column, Line};

// --- set_scroll_region ---

#[test]
fn set_scroll_region_full_screen() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(1, None);
    assert_eq!(grid.scroll_region, 0..24);
}

#[test]
fn set_scroll_region_sub_region() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(2, Some(10));
    assert_eq!(grid.scroll_region, 1..10);
}

#[test]
fn set_scroll_region_default_bottom() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(5, None);
    assert_eq!(grid.scroll_region, 4..24);
}

#[test]
fn set_scroll_region_invalid_top_ge_bottom() {
    let mut grid = Grid::new(24, 80);
    let original = grid.scroll_region.clone();
    // top >= bottom: no change.
    grid.set_scroll_region(10, Some(5));
    assert_eq!(grid.scroll_region, original);
}

#[test]
fn set_scroll_region_top_zero_treated_as_one() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(0, Some(10));
    // top=0 treated as top=1 -> 0-based top=0.
    assert_eq!(grid.scroll_region, 0..10);
}

#[test]
fn set_scroll_region_clamps_oversized_bottom() {
    let mut grid = Grid::new(10, 80);
    grid.set_scroll_region(1, Some(100));
    assert_eq!(grid.scroll_region, 0..10);
}

#[test]
fn set_scroll_region_moves_cursor_to_origin() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(10);
    grid.cursor_mut().set_col(Column(40));
    grid.set_scroll_region(5, Some(20));
    assert_eq!(grid.cursor().line(), 0);
    assert_eq!(grid.cursor().col(), Column(0));
}

// --- scroll_up ---

#[test]
fn scroll_up_one_line_full_screen() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_up(1);
    // Line 0 now has what was line 1 ('B').
    assert_eq!(grid[Line(0)][Column(0)].ch, 'B');
    // Line 1 now has what was line 2 ('C').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    // Line 2 is blank.
    assert!(grid[Line(2)][Column(0)].is_empty());
}

#[test]
fn scroll_up_three_lines_full_screen() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_up(3);
    // Lines 0-1 have what was lines 3-4 ('D', 'E').
    assert_eq!(grid[Line(0)][Column(0)].ch, 'D');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'E');
    // Lines 2-4 are blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_up_sub_region_preserves_outside() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    grid.scroll_up(1);
    // Line 0 ('A') untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Inside region: line 1 now has 'C', line 2 has 'D', line 3 blank.
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    assert_eq!(grid[Line(2)][Column(0)].ch, 'D');
    assert!(grid[Line(3)][Column(0)].is_empty());
}

#[test]
fn scroll_up_count_exceeds_region() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Count larger than region: clamped, all lines blank.
    grid.scroll_up(100);
    for line in 0..3 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_up_bce_fill() {
    let mut grid = Grid::new(3, 10);
    grid.put_char('A');
    grid.cursor_mut().template.bg = Color::Indexed(4);
    grid.scroll_up(1);
    // New bottom row has BCE background.
    assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(4));
    assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(4));
}

// --- scroll_down ---

#[test]
fn scroll_down_one_line_full_screen() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_down(1);
    // Line 0 is blank.
    assert!(grid[Line(0)][Column(0)].is_empty());
    // Line 1 has what was line 0 ('A').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'A');
    // Line 2 has what was line 1 ('B').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
}

#[test]
fn scroll_down_sub_region_preserves_outside() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    grid.scroll_down(1);
    // Line 0 ('A') untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Inside region: line 1 blank, line 2 has 'B', line 3 has 'C'.
    assert!(grid[Line(1)][Column(0)].is_empty());
    assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
    assert_eq!(grid[Line(3)][Column(0)].ch, 'C');
}

#[test]
fn scroll_down_count_exceeds_region() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_down(100);
    for line in 0..3 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_down_bce_fill() {
    let mut grid = Grid::new(3, 10);
    grid.put_char('A');
    grid.cursor_mut().template.bg = Color::Indexed(2);
    grid.scroll_down(1);
    // New top row has BCE background.
    assert_eq!(grid[Line(0)][Column(0)].bg, Color::Indexed(2));
    assert_eq!(grid[Line(0)][Column(9)].bg, Color::Indexed(2));
}

// --- insert_lines ---

#[test]
fn insert_lines_mid_region() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Cursor at line 2, insert 2 blank lines.
    grid.cursor_mut().set_line(2);
    grid.insert_lines(2);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-3 are blank (inserted).
    assert!(grid[Line(2)][Column(0)].is_empty());
    assert!(grid[Line(3)][Column(0)].is_empty());
    // Line 4 has what was line 2 ('C'). Lines D and E pushed off.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'C');
}

#[test]
fn insert_lines_outside_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    // Cursor outside scroll region.
    grid.cursor_mut().set_line(0);
    grid.insert_lines(1);
    // Nothing changed.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
}

#[test]
fn insert_lines_count_capped() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(2);
    // Insert more lines than remaining in region.
    grid.insert_lines(100);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-4 all blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn insert_lines_bce_fill() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().template.bg = Color::Indexed(5);
    grid.insert_lines(1);
    // Inserted line at 1 has BCE background.
    assert_eq!(grid[Line(1)][Column(0)].bg, Color::Indexed(5));
    assert_eq!(grid[Line(1)][Column(9)].bg, Color::Indexed(5));
}

// --- delete_lines ---

#[test]
fn delete_lines_mid_region() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Cursor at line 1, delete 2 lines.
    grid.cursor_mut().set_line(1);
    grid.delete_lines(2);
    // Line 0 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 1 now has what was line 3 ('D').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'D');
    // Line 2 now has what was line 4 ('E').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'E');
    // Lines 3-4 are blank.
    assert!(grid[Line(3)][Column(0)].is_empty());
    assert!(grid[Line(4)][Column(0)].is_empty());
}

#[test]
fn delete_lines_outside_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    // Cursor outside scroll region.
    grid.cursor_mut().set_line(4);
    grid.delete_lines(1);
    // Nothing changed.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    assert_eq!(grid[Line(3)][Column(0)].ch, 'D');
}

#[test]
fn delete_lines_count_capped() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(2);
    grid.delete_lines(100);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-4 all blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn delete_lines_bce_fill() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().template.bg = Color::Indexed(3);
    grid.delete_lines(1);
    // New bottom row has BCE background.
    assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(3));
    assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(3));
}
