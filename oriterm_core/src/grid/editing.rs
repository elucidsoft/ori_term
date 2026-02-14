//! Grid editing operations.
//!
//! Character insertion, deletion, and erase operations. These are the
//! primitives the VTE handler calls for writing text and manipulating
//! grid content.

use unicode_width::UnicodeWidthChar;

use crate::cell::{Cell, CellFlags};
use crate::index::Column;

use super::Grid;

/// Erase mode for display and line erase operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseMode {
    /// Erase from cursor to end (of display or line).
    Below,
    /// Erase from start (of display or line) to cursor.
    Above,
    /// Erase entire (display or line).
    All,
    /// Erase scrollback buffer only (display erase only).
    Scrollback,
}

impl Grid {
    /// Write a character at the cursor position.
    ///
    /// Handles wide characters (writes cell + spacer), wrap at end of line,
    /// and clearing overwritten wide char pairs.
    pub fn put_char(&mut self, ch: char) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let width = UnicodeWidthChar::width(ch).unwrap_or(1);
        let cols = self.cols;

        // Wide char can never fit in this terminal width — skip it.
        // Without this guard, a width-2 char on a 1-column grid would
        // loop forever: wrap → col 0 → can't fit → wrap → col 0 → …
        if width > cols {
            return;
        }

        loop {
            let line = self.cursor.line();
            let col = self.cursor.col().0;

            // If a pending wrap is active and we're at the last column, wrap now.
            if col >= cols {
                self.rows[line][Column(cols - 1)].flags |= CellFlags::WRAP;
                self.linefeed();
                self.cursor.set_col(Column(0));
                continue;
            }

            // For wide chars at the last column, wrap instead of splitting.
            if width == 2 && col + 1 >= cols {
                self.rows[line][Column(col)].flags |= CellFlags::WRAP;
                self.linefeed();
                self.cursor.set_col(Column(0));
                continue;
            }

            // Clear any wide char pair that we're overwriting.
            self.clear_wide_char_at(line, col);

            // Extract template fields before mutable row borrow. `rows` and
            // `cursor` are disjoint Grid fields, so this avoids a full Cell clone.
            let tmpl_fg = self.cursor.template.fg;
            let tmpl_bg = self.cursor.template.bg;
            let tmpl_flags = self.cursor.template.flags;
            let tmpl_extra = self.cursor.template.extra.clone();
            let cell = &mut self.rows[line][Column(col)];
            cell.ch = ch;
            cell.fg = tmpl_fg;
            cell.bg = tmpl_bg;
            cell.flags = tmpl_flags;
            cell.extra = tmpl_extra;

            if width == 2 {
                cell.flags |= CellFlags::WIDE_CHAR;

                // Write the spacer in the next column.
                if col + 1 < cols {
                    self.clear_wide_char_at(line, col + 1);
                    let spacer = &mut self.rows[line][Column(col + 1)];
                    spacer.ch = ' ';
                    spacer.fg = tmpl_fg;
                    spacer.bg = tmpl_bg;
                    spacer.flags = CellFlags::WIDE_CHAR_SPACER;
                    spacer.extra = None;
                }
            }

            // Advance cursor by character width.
            self.cursor.set_col(Column(col + width));

            break;
        }
    }

    /// Insert `count` blank cells at the cursor, shifting existing cells right.
    ///
    /// Cells that shift past the right edge are lost.
    pub fn insert_blank(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        if col >= cols {
            return;
        }

        let count = count.min(cols - col);
        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();

        // Shift cells right by swapping (no allocation).
        for i in (col + count..cols).rev() {
            cells.swap(i, i - count);
        }

        // Reset the gap cells in-place.
        for cell in &mut cells[col..col + count] {
            cell.reset(&template);
        }

        // Cells shifted right: occ grows by at most `count`, capped at cols.
        row.set_occ((row.occ() + count).min(cols));
    }

    /// Delete `count` cells at the cursor, shifting remaining cells left.
    ///
    /// New cells at the right edge are blank.
    pub fn delete_chars(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        if col >= cols {
            return;
        }

        let count = count.min(cols - col);
        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();

        // Shift cells left by swapping (no allocation).
        for i in col..cols - count {
            cells.swap(i, i + count);
        }

        // Reset the vacated right cells in-place.
        for cell in &mut cells[cols - count..cols] {
            cell.reset(&template);
        }

        if !template.is_empty() {
            // BCE: fill cells at [cols-count..cols] are dirty.
            row.set_occ(cols);
        }
        // else: Content shifted left; existing occ remains a valid upper
        // bound. Fill cells are empty and don't extend the dirty range.
    }

    /// Erase part or all of the display.
    pub fn erase_display(&mut self, mode: EraseMode) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);
        match mode {
            EraseMode::Below => {
                self.erase_line_with_template(EraseMode::Below, &template);
                for line in self.cursor.line() + 1..self.lines {
                    self.rows[line].reset(self.cols, &template);
                }
            }
            EraseMode::Above => {
                self.erase_line_with_template(EraseMode::Above, &template);
                for line in 0..self.cursor.line() {
                    self.rows[line].reset(self.cols, &template);
                }
            }
            EraseMode::All => {
                for line in 0..self.lines {
                    self.rows[line].reset(self.cols, &template);
                }
            }
            EraseMode::Scrollback => {
                // Scrollback clearing will be implemented in 1.10.
            }
        }
    }

    /// Erase part or all of the current line.
    pub fn erase_line(&mut self, mode: EraseMode) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let template = Cell::from(self.cursor.template.bg);
        self.erase_line_with_template(mode, &template);
    }

    /// Erase part or all of the current line using a pre-built BCE template.
    fn erase_line_with_template(&mut self, mode: EraseMode, template: &Cell) {
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;

        debug_assert!(
            mode != EraseMode::Scrollback,
            "Scrollback mode not applicable to erase_line"
        );

        match mode {
            EraseMode::Below => {
                let row = &mut self.rows[line];
                let cells = row.as_mut_slice();
                for cell in &mut cells[col..cols] {
                    cell.reset(template);
                }
                if template.is_empty() {
                    row.clamp_occ(col);
                } else {
                    row.set_occ(cols);
                }
            }
            EraseMode::Above => {
                let end = col.min(cols - 1) + 1;
                let row = &mut self.rows[line];
                let cells = row.as_mut_slice();
                for cell in &mut cells[..end] {
                    cell.reset(template);
                }
                if template.is_empty() {
                    // Cells [0..end] are now empty. Only cells beyond end
                    // may be dirty, so if occ was within the erased range
                    // all dirty cells are gone.
                    if row.occ() <= end {
                        row.set_occ(0);
                    }
                } else {
                    row.set_occ(row.occ().max(end));
                }
            }
            EraseMode::All => {
                self.rows[line].reset(cols, template);
            }
            // Scrollback clearing has no meaning at the line level (CSI 3 K
            // doesn't exist in xterm/ECMA-48). Treat as no-op in release builds.
            EraseMode::Scrollback => {}
        }
    }

    /// Erase `count` cells starting at cursor (replace with template, don't shift).
    pub fn erase_chars(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        let end = (col + count).min(cols);
        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();
        for cell in &mut cells[col..end] {
            cell.reset(&template);
        }
        // BCE template has a colored bg — the erased cells are dirty.
        // Default template produces truly empty cells, so existing occ
        // remains a valid upper bound (we only cleared, didn't extend).
        if !template.is_empty() {
            row.set_occ(row.occ().max(end));
        }
    }

    /// Clear any wide char pair at the given position.
    ///
    /// If the cell is a wide char spacer, clears the preceding wide char.
    /// If the cell is a wide char, clears its trailing spacer.
    fn clear_wide_char_at(&mut self, line: usize, col: usize) {
        let cols = self.cols;

        if col >= cols {
            return;
        }

        let flags = self.rows[line][Column(col)].flags;

        // Overwriting a spacer: clear the wide char that owns it.
        if flags.contains(CellFlags::WIDE_CHAR_SPACER) && col > 0 {
            let prev = &mut self.rows[line][Column(col - 1)];
            prev.ch = ' ';
            prev.flags.remove(CellFlags::WIDE_CHAR);
        }

        // Overwriting a wide char: clear its spacer.
        if flags.contains(CellFlags::WIDE_CHAR) && col + 1 < cols {
            let next = &mut self.rows[line][Column(col + 1)];
            next.ch = ' ';
            next.flags.remove(CellFlags::WIDE_CHAR_SPACER);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EraseMode;
    use crate::grid::Grid;
    use crate::index::Column;

    /// Helper: create a grid and write a string of ASCII chars.
    fn grid_with_text(lines: usize, cols: usize, text: &str) -> Grid {
        let mut grid = Grid::new(lines, cols);
        for ch in text.chars() {
            grid.put_char(ch);
        }
        grid
    }

    #[test]
    fn put_char_writes_and_advances() {
        let mut grid = Grid::new(24, 80);
        grid.put_char('A');
        assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
        assert_eq!(grid.cursor().col(), Column(1));
    }

    #[test]
    fn put_char_wide_writes_pair() {
        let mut grid = Grid::new(24, 80);
        grid.put_char('好');
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, '好');
        assert!(grid[line][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR));
        assert!(grid[line][Column(1)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR_SPACER));
        assert_eq!(grid.cursor().col(), Column(2));
    }

    #[test]
    fn wide_char_at_last_column_wraps() {
        let mut grid = Grid::new(24, 5);
        // Fill columns 0..4 with 'A', cursor at col 4.
        for _ in 0..4 {
            grid.put_char('A');
        }
        assert_eq!(grid.cursor().col(), Column(4));
        // Writing a wide char at col 4 should wrap to next line.
        grid.put_char('好');
        assert_eq!(grid.cursor().line(), 1);
        assert_eq!(grid.cursor().col(), Column(2));
        assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, '好');
    }

    #[test]
    fn overwrite_spacer_clears_wide_char() {
        let mut grid = Grid::new(24, 80);
        grid.put_char('好');
        // Now cursor is at col 2. Move cursor to col 1 (the spacer).
        grid.cursor_mut().set_col(Column(1));
        grid.put_char('X');
        let line = crate::index::Line(0);
        // The wide char at col 0 should be cleared.
        assert_eq!(grid[line][Column(0)].ch, ' ');
        assert!(!grid[line][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR));
        assert_eq!(grid[line][Column(1)].ch, 'X');
    }

    #[test]
    fn overwrite_wide_char_clears_spacer() {
        let mut grid = Grid::new(24, 80);
        grid.put_char('好');
        // Move cursor back to col 0 (the wide char).
        grid.cursor_mut().set_col(Column(0));
        grid.put_char('Y');
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, 'Y');
        // The spacer at col 1 should be cleared.
        assert_eq!(grid[line][Column(1)].ch, ' ');
        assert!(!grid[line][Column(1)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR_SPACER));
    }

    #[test]
    fn insert_blank_shifts_right() {
        let mut grid = grid_with_text(24, 80, "ABCDE");
        grid.cursor_mut().set_col(Column(1));
        grid.insert_blank(3);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, 'A');
        assert_eq!(grid[line][Column(1)].ch, ' ');
        assert_eq!(grid[line][Column(2)].ch, ' ');
        assert_eq!(grid[line][Column(3)].ch, ' ');
        assert_eq!(grid[line][Column(4)].ch, 'B');
        assert_eq!(grid[line][Column(5)].ch, 'C');
    }

    #[test]
    fn delete_chars_shifts_left() {
        let mut grid = grid_with_text(24, 80, "ABCDE");
        grid.cursor_mut().set_col(Column(1));
        grid.delete_chars(2);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, 'A');
        assert_eq!(grid[line][Column(1)].ch, 'D');
        assert_eq!(grid[line][Column(2)].ch, 'E');
        // Cells at right are blank.
        assert!(grid[line][Column(3)].is_empty());
    }

    #[test]
    fn erase_display_below() {
        let mut grid = Grid::new(3, 10);
        // Fill all 3 lines with 'X'.
        for line in 0..3 {
            grid.cursor_mut().set_line(line);
            grid.cursor_mut().set_col(Column(0));
            for _ in 0..10 {
                grid.put_char('X');
            }
        }
        // Position cursor at line 1, col 5 and erase below.
        grid.cursor_mut().set_line(1);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_display(EraseMode::Below);
        let line0 = crate::index::Line(0);
        let line1 = crate::index::Line(1);
        let line2 = crate::index::Line(2);
        // Line 0 untouched.
        assert_eq!(grid[line0][Column(0)].ch, 'X');
        // Line 1: cols 0-4 untouched, 5+ erased.
        assert_eq!(grid[line1][Column(4)].ch, 'X');
        assert!(grid[line1][Column(5)].is_empty());
        // Line 2 fully erased.
        assert!(grid[line2][Column(0)].is_empty());
    }

    #[test]
    fn erase_display_above() {
        let mut grid = Grid::new(3, 10);
        for line in 0..3 {
            grid.cursor_mut().set_line(line);
            grid.cursor_mut().set_col(Column(0));
            for _ in 0..10 {
                grid.put_char('X');
            }
        }
        grid.cursor_mut().set_line(1);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_display(EraseMode::Above);
        let line0 = crate::index::Line(0);
        let line1 = crate::index::Line(1);
        let line2 = crate::index::Line(2);
        // Line 0 fully erased.
        assert!(grid[line0][Column(0)].is_empty());
        // Line 1: 0-5 erased, 6+ untouched.
        assert!(grid[line1][Column(5)].is_empty());
        assert_eq!(grid[line1][Column(6)].ch, 'X');
        // Line 2 untouched.
        assert_eq!(grid[line2][Column(0)].ch, 'X');
    }

    #[test]
    fn erase_display_all() {
        let mut grid = grid_with_text(3, 10, "AAAAAAAAAA");
        grid.erase_display(EraseMode::All);
        for line in 0..3 {
            for col in 0..10 {
                assert!(
                    grid[crate::index::Line(line as i32)][Column(col)].is_empty(),
                    "Cell ({line}, {col}) not empty"
                );
            }
        }
    }

    #[test]
    fn erase_line_below() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_line(EraseMode::Below);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(4)].ch, 'E');
        assert!(grid[line][Column(5)].is_empty());
        assert!(grid[line][Column(9)].is_empty());
    }

    #[test]
    fn erase_line_all() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_line(EraseMode::All);
        let line = crate::index::Line(0);
        for col in 0..10 {
            assert!(grid[line][Column(col)].is_empty());
        }
    }

    #[test]
    fn erase_chars_no_shift() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(2));
        grid.erase_chars(5);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, 'A');
        assert_eq!(grid[line][Column(1)].ch, 'B');
        assert!(grid[line][Column(2)].is_empty());
        assert!(grid[line][Column(6)].is_empty());
        assert_eq!(grid[line][Column(7)].ch, 'H');
    }

    #[test]
    fn erase_chars_default_bg_does_not_inflate_occ() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        // occ is 10 after writing 10 chars.
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_chars(3);
        // Erased [5..8) with default bg. occ should stay at 10
        // (cells beyond 8 are still dirty from the original write).
        let line = crate::index::Line(0);
        assert!(grid[line][Column(5)].is_empty());
        assert!(grid[line][Column(7)].is_empty());
        assert_eq!(grid[line][Column(8)].ch, 'I');
    }

    #[test]
    fn wide_char_on_single_column_grid_does_not_hang() {
        let mut grid = Grid::new(3, 1);
        // Width-2 char can never fit in a 1-column grid. Must return
        // immediately without writing or looping.
        grid.put_char('好');
        assert_eq!(grid.cursor().col(), Column(0));
        assert!(grid[crate::index::Line(0)][Column(0)].is_empty());
    }

    // --- Additional tests from reference repo gap analysis ---

    #[test]
    fn put_char_inherits_template_attributes() {
        use vte::ansi::Color;
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().template.fg = Color::Indexed(1);
        grid.cursor_mut().template.bg = Color::Indexed(2);
        grid.cursor_mut().template.flags = crate::cell::CellFlags::BOLD;
        grid.put_char('A');

        let cell = &grid[crate::index::Line(0)][Column(0)];
        assert_eq!(cell.ch, 'A');
        assert_eq!(cell.fg, Color::Indexed(1));
        assert_eq!(cell.bg, Color::Indexed(2));
        assert!(cell.flags.contains(crate::cell::CellFlags::BOLD));
    }

    #[test]
    fn put_char_fills_row_and_wraps_to_next_line() {
        let mut grid = Grid::new(3, 5);
        for ch in "ABCDE".chars() {
            grid.put_char(ch);
        }
        // After filling row, cursor is at col 5 (pending wrap).
        assert_eq!(grid.cursor().col(), Column(5));
        assert_eq!(grid.cursor().line(), 0);

        // Writing another char triggers wrap to next line.
        grid.put_char('F');
        assert_eq!(grid.cursor().line(), 1);
        assert_eq!(grid.cursor().col(), Column(1));
        assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'F');
    }

    #[test]
    fn put_char_sequence_fills_correctly() {
        let mut grid = Grid::new(24, 10);
        for ch in "ABCDEFGHIJ".chars() {
            grid.put_char(ch);
        }
        let line = crate::index::Line(0);
        for (i, ch) in "ABCDEFGHIJ".chars().enumerate() {
            assert_eq!(grid[line][Column(i)].ch, ch, "Column {i} mismatch");
        }
    }

    #[test]
    fn insert_blank_at_end_of_line() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(9));
        grid.insert_blank(1);
        let line = crate::index::Line(0);
        // Last cell should be blank, 'J' shifted off the edge.
        assert!(grid[line][Column(9)].is_empty());
        assert_eq!(grid[line][Column(8)].ch, 'I');
    }

    #[test]
    fn insert_blank_count_exceeds_remaining() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(5));
        grid.insert_blank(100);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(0)].ch, 'A');
        assert_eq!(grid[line][Column(4)].ch, 'E');
        for col in 5..10 {
            assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
        }
    }

    #[test]
    fn insert_blank_with_bce() {
        use vte::ansi::Color;
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(2));
        grid.cursor_mut().template.bg = Color::Indexed(3);
        grid.insert_blank(2);
        let line = crate::index::Line(0);
        // Inserted blanks should have the BCE background.
        assert_eq!(grid[line][Column(2)].bg, Color::Indexed(3));
        assert_eq!(grid[line][Column(3)].bg, Color::Indexed(3));
        assert_eq!(grid[line][Column(2)].ch, ' ');
    }

    #[test]
    fn insert_blank_cursor_past_end_is_noop() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(10));
        grid.insert_blank(5);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(9)].ch, 'J');
    }

    #[test]
    fn delete_chars_at_end_of_line() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(9));
        grid.delete_chars(1);
        let line = crate::index::Line(0);
        assert!(grid[line][Column(9)].is_empty());
        assert_eq!(grid[line][Column(8)].ch, 'I');
    }

    #[test]
    fn delete_chars_count_exceeds_remaining() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(5));
        grid.delete_chars(100);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(4)].ch, 'E');
        for col in 5..10 {
            assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
        }
    }

    #[test]
    fn delete_chars_with_bce() {
        use vte::ansi::Color;
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(2));
        grid.cursor_mut().template.bg = Color::Indexed(5);
        grid.delete_chars(3);
        let line = crate::index::Line(0);
        // Shifted: col 2 now has 'F', col 3 has 'G', etc.
        assert_eq!(grid[line][Column(2)].ch, 'F');
        // Right edge filled with BCE cells.
        assert_eq!(grid[line][Column(7)].bg, Color::Indexed(5));
        assert_eq!(grid[line][Column(8)].bg, Color::Indexed(5));
        assert_eq!(grid[line][Column(9)].bg, Color::Indexed(5));
    }

    #[test]
    fn delete_chars_cursor_past_end_is_noop() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(10));
        grid.delete_chars(5);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(9)].ch, 'J');
    }

    #[test]
    fn erase_line_above() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_line(EraseMode::Above);
        let line = crate::index::Line(0);
        // Cols 0..=5 should be erased.
        for col in 0..=5 {
            assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
        }
        // Cols 6..9 untouched.
        assert_eq!(grid[line][Column(6)].ch, 'G');
        assert_eq!(grid[line][Column(9)].ch, 'J');
    }

    #[test]
    fn erase_chars_past_end_of_line() {
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(7));
        grid.erase_chars(100);
        let line = crate::index::Line(0);
        assert_eq!(grid[line][Column(6)].ch, 'G');
        for col in 7..10 {
            assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
        }
    }

    #[test]
    fn erase_display_with_bce_background() {
        use vte::ansi::Color;
        let mut grid = Grid::new(3, 10);
        for line in 0..3 {
            grid.cursor_mut().set_line(line);
            grid.cursor_mut().set_col(Column(0));
            for _ in 0..10 {
                grid.put_char('X');
            }
        }
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));
        grid.cursor_mut().template.bg = Color::Indexed(6);
        grid.erase_display(EraseMode::All);
        // All cells should have the BCE background.
        for line in 0..3 {
            for col in 0..10 {
                assert_eq!(
                    grid[crate::index::Line(line as i32)][Column(col)].bg,
                    Color::Indexed(6),
                    "Cell ({line}, {col}) bg mismatch"
                );
            }
        }
    }

    #[test]
    fn erase_display_below_at_last_line() {
        let mut grid = grid_with_text(3, 10, "AAAAAAAAAA");
        grid.cursor_mut().set_line(2);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_display(EraseMode::Below);
        // Only line 2 from col 5 should be erased (line 2 was empty anyway).
        let line2 = crate::index::Line(2);
        assert!(grid[line2][Column(5)].is_empty());
        // Line 0 untouched.
        assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    }

    #[test]
    fn erase_display_above_at_first_line() {
        let mut grid = grid_with_text(3, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.erase_display(EraseMode::Above);
        let line0 = crate::index::Line(0);
        // Cols 0..=5 erased on line 0.
        assert!(grid[line0][Column(0)].is_empty());
        assert!(grid[line0][Column(5)].is_empty());
        // Cols 6+ untouched.
        assert_eq!(grid[line0][Column(6)].ch, 'G');
    }

    #[test]
    fn wrap_flag_set_on_wrapped_line() {
        let mut grid = Grid::new(3, 5);
        for ch in "ABCDEF".chars() {
            grid.put_char(ch);
        }
        // The last cell of line 0 should have the WRAP flag.
        let line0 = crate::index::Line(0);
        assert!(grid[line0][Column(4)]
            .flags
            .contains(crate::cell::CellFlags::WRAP));
    }

    #[test]
    fn put_char_wide_spacer_inherits_template_bg() {
        use vte::ansi::Color;
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().template.bg = Color::Indexed(3);
        grid.put_char('好');
        let line = crate::index::Line(0);
        // Wide char cell gets template bg.
        assert_eq!(grid[line][Column(0)].bg, Color::Indexed(3));
        // Spacer also gets template bg.
        assert_eq!(grid[line][Column(1)].bg, Color::Indexed(3));
    }

    #[test]
    fn erase_chars_with_bce_background() {
        use vte::ansi::Color;
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_col(Column(3));
        grid.cursor_mut().template.bg = Color::Indexed(7);
        grid.erase_chars(4);
        let line = crate::index::Line(0);
        // Erased cells [3..7) get BCE background.
        for col in 3..7 {
            assert_eq!(grid[line][Column(col)].bg, Color::Indexed(7));
            assert_eq!(grid[line][Column(col)].ch, ' ');
        }
        // Surrounding cells untouched.
        assert_eq!(grid[line][Column(2)].ch, 'C');
        assert_eq!(grid[line][Column(7)].ch, 'H');
    }

    #[test]
    fn erase_line_below_with_bce() {
        use vte::ansi::Color;
        let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(5));
        grid.cursor_mut().template.bg = Color::Indexed(2);
        grid.erase_line(EraseMode::Below);
        let line = crate::index::Line(0);
        for col in 5..10 {
            assert_eq!(grid[line][Column(col)].bg, Color::Indexed(2));
        }
        // Cols before cursor untouched.
        assert_eq!(grid[line][Column(4)].ch, 'E');
    }
}
