//! Cursor movement and navigation operations.
//!
//! Implements CUU/CUD/CUF/CUB/CUP/CHA/VPA/CR/LF/RI/NEL/HT/CBT and
//! tab stop management. All movement is clamped to grid bounds and
//! respects the scroll region where applicable.

use crate::index::Column;

use super::Grid;

/// Tab clear mode for TBC (Tabulation Clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabClearMode {
    /// Clear tab stop at the current column.
    Current,
    /// Clear all tab stops.
    All,
}

impl Grid {
    /// CUU: move cursor up by `count` lines, clamped to the top of the
    /// scroll region (if inside it) or line 0.
    pub fn move_up(&mut self, count: usize) {
        let line = self.cursor.line();
        let top = if line >= self.scroll_region.start && line < self.scroll_region.end {
            self.scroll_region.start
        } else {
            0
        };
        self.cursor.set_line(line.saturating_sub(count).max(top));
    }

    /// CUD: move cursor down by `count` lines, clamped to the bottom of
    /// the scroll region (if inside it) or the last line.
    pub fn move_down(&mut self, count: usize) {
        let line = self.cursor.line();
        let bottom = if line >= self.scroll_region.start && line < self.scroll_region.end {
            self.scroll_region.end - 1
        } else {
            self.lines - 1
        };
        self.cursor.set_line((line + count).min(bottom));
    }

    /// CUF: move cursor right by `count` columns, clamped to the last column.
    pub fn move_forward(&mut self, count: usize) {
        let col = self.cursor.col().0;
        let last = self.cols - 1;
        self.cursor.set_col(Column((col + count).min(last)));
    }

    /// CUB: move cursor left by `count` columns, clamped to column 0.
    pub fn move_backward(&mut self, count: usize) {
        let col = self.cursor.col().0;
        self.cursor.set_col(Column(col.saturating_sub(count)));
    }

    /// CUP: set cursor to absolute `(line, col)`, clamped to grid bounds.
    pub fn move_to(&mut self, line: usize, col: Column) {
        self.cursor.set_line(line.min(self.lines - 1));
        self.cursor.set_col(Column(col.0.min(self.cols - 1)));
    }

    /// CHA: set cursor column to `col`, clamped to the last column.
    pub fn move_to_column(&mut self, col: Column) {
        self.cursor.set_col(Column(col.0.min(self.cols - 1)));
    }

    /// VPA: set cursor line to `line`, clamped to the last line.
    pub fn move_to_line(&mut self, line: usize) {
        self.cursor.set_line(line.min(self.lines - 1));
    }

    /// CR: move cursor to column 0.
    pub fn carriage_return(&mut self) {
        self.cursor.set_col(Column(0));
    }

    /// LF: move cursor down one line. If at the bottom of the scroll
    /// region, scroll the region up instead of moving.
    pub fn linefeed(&mut self) {
        let line = self.cursor.line();
        if line + 1 == self.scroll_region.end {
            // At bottom of scroll region: scroll region content up.
            self.scroll_up(1);
        } else if line + 1 < self.lines {
            self.cursor.set_line(line + 1);
        } else {
            // Already at last line, outside scroll region: no-op.
        }
    }

    /// RI: move cursor up one line. If at the top of the scroll region,
    /// scroll the region down instead of moving.
    pub fn reverse_index(&mut self) {
        let line = self.cursor.line();
        if line == self.scroll_region.start {
            // At top of scroll region: scroll region content down.
            self.scroll_down(1);
        } else if line > 0 {
            self.cursor.set_line(line - 1);
        } else {
            // Already at line 0, outside scroll region: no-op.
        }
    }

    /// NEL: carriage return followed by linefeed.
    pub fn next_line(&mut self) {
        self.carriage_return();
        self.linefeed();
    }

    /// HT: advance cursor to the next tab stop, or end of line.
    pub fn tab(&mut self) {
        let col = self.cursor.col().0;
        let last = self.cols - 1;

        // Search forward for the next tab stop.
        for c in (col + 1)..self.cols {
            if self.tab_stops[c] {
                self.cursor.set_col(Column(c));
                return;
            }
        }
        // No tab stop found: move to last column.
        self.cursor.set_col(Column(last));
    }

    /// CBT: move cursor to the previous tab stop, or column 0.
    pub fn tab_backward(&mut self) {
        let col = self.cursor.col().0;

        // Search backward for the previous tab stop.
        for c in (0..col).rev() {
            if self.tab_stops[c] {
                self.cursor.set_col(Column(c));
                return;
            }
        }
        // No tab stop found: move to column 0.
        self.cursor.set_col(Column(0));
    }

    /// HTS: set a tab stop at the current cursor column.
    pub fn set_tab_stop(&mut self) {
        let col = self.cursor.col().0;
        if col < self.cols {
            self.tab_stops[col] = true;
        }
    }

    /// TBC: clear tab stop(s) according to mode.
    pub fn clear_tab_stop(&mut self, mode: TabClearMode) {
        match mode {
            TabClearMode::Current => {
                let col = self.cursor.col().0;
                if col < self.cols {
                    self.tab_stops[col] = false;
                }
            }
            TabClearMode::All => {
                self.tab_stops.fill(false);
            }
        }
    }

    /// DECSC: save cursor position and template.
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor.clone());
    }

    /// DECRC: restore cursor from saved state, or reset to origin if
    /// nothing was saved.
    pub fn restore_cursor(&mut self) {
        if let Some(saved) = &self.saved_cursor {
            self.cursor = saved.clone();
        } else {
            self.cursor = super::cursor::Cursor::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TabClearMode;
    use crate::grid::Grid;
    use crate::index::{Column, Line};

    #[test]
    fn move_up_from_line_5_to_line_2() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(5);
        grid.move_up(3);
        assert_eq!(grid.cursor().line(), 2);
    }

    #[test]
    fn move_up_clamps_to_top() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(5);
        grid.move_up(100);
        assert_eq!(grid.cursor().line(), 0);
    }

    #[test]
    fn move_down_from_line_0_to_line_3() {
        let mut grid = Grid::new(24, 80);
        grid.move_down(3);
        assert_eq!(grid.cursor().line(), 3);
    }

    #[test]
    fn move_down_clamps_to_bottom() {
        let mut grid = Grid::new(24, 80);
        grid.move_down(100);
        assert_eq!(grid.cursor().line(), 23);
    }

    #[test]
    fn move_forward_from_col_0_to_col_5() {
        let mut grid = Grid::new(24, 80);
        grid.move_forward(5);
        assert_eq!(grid.cursor().col(), Column(5));
    }

    #[test]
    fn move_forward_clamps_to_last_column() {
        let mut grid = Grid::new(24, 80);
        grid.move_forward(100);
        assert_eq!(grid.cursor().col(), Column(79));
    }

    #[test]
    fn move_backward_from_col_5_to_col_2() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_col(Column(5));
        grid.move_backward(3);
        assert_eq!(grid.cursor().col(), Column(2));
    }

    #[test]
    fn move_to_sets_position() {
        let mut grid = Grid::new(24, 80);
        grid.move_to(5, Column(10));
        assert_eq!(grid.cursor().line(), 5);
        assert_eq!(grid.cursor().col(), Column(10));
    }

    #[test]
    fn carriage_return_sets_col_zero() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_col(Column(40));
        grid.carriage_return();
        assert_eq!(grid.cursor().col(), Column(0));
    }

    #[test]
    fn linefeed_at_bottom_triggers_scroll() {
        let mut grid = Grid::new(3, 10);
        // Write 'A' on line 0.
        grid.put_char('A');
        // Move to bottom line.
        grid.cursor_mut().set_line(2);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char('Z');

        // Linefeed at bottom should scroll up.
        grid.linefeed();
        assert_eq!(grid.cursor().line(), 2);
        // Line 0 content ('A') should have scrolled off; line 0 is now
        // what was line 1 (empty).
        assert!(grid[Line(0)][Column(0)].is_empty());
        // Line 1 now has what was line 2 ('Z').
        assert_eq!(grid[Line(1)][Column(0)].ch, 'Z');
        // Line 2 is the new blank row.
        assert!(grid[Line(2)][Column(0)].is_empty());
    }

    #[test]
    fn linefeed_in_middle_moves_down() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(5);
        grid.linefeed();
        assert_eq!(grid.cursor().line(), 6);
    }

    #[test]
    fn reverse_index_at_top_triggers_scroll_down() {
        let mut grid = Grid::new(3, 10);
        // Write 'B' on line 0.
        grid.put_char('B');
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));

        // Reverse index at top should scroll down.
        grid.reverse_index();
        assert_eq!(grid.cursor().line(), 0);
        // Line 0 is now a blank row (inserted at top).
        assert!(grid[Line(0)][Column(0)].is_empty());
        // Line 1 has what was line 0 ('B').
        assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    }

    #[test]
    fn tab_advances_to_next_stop() {
        let mut grid = Grid::new(24, 80);
        // Default stops at 0, 8, 16, 24, ...
        grid.cursor_mut().set_col(Column(1));
        grid.tab();
        assert_eq!(grid.cursor().col(), Column(8));
    }

    #[test]
    fn tab_at_last_stop_goes_to_end() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_col(Column(72));
        grid.tab();
        // No tab stop after 72 until end of line.
        assert_eq!(grid.cursor().col(), Column(79));
    }

    #[test]
    fn tab_backward_moves_to_previous_stop() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_col(Column(10));
        grid.tab_backward();
        assert_eq!(grid.cursor().col(), Column(8));
    }

    #[test]
    fn set_and_clear_tab_stop() {
        let mut grid = Grid::new(24, 80);
        // Column 5 is not a default stop.
        assert!(!grid.tab_stops()[5]);

        grid.cursor_mut().set_col(Column(5));
        grid.set_tab_stop();
        assert!(grid.tab_stops()[5]);

        grid.clear_tab_stop(TabClearMode::Current);
        assert!(!grid.tab_stops()[5]);

        // Clear all.
        grid.set_tab_stop();
        grid.clear_tab_stop(TabClearMode::All);
        assert!(!grid.tab_stops()[0]); // Even default stops are cleared.
        assert!(!grid.tab_stops()[8]);
    }

    #[test]
    fn save_and_restore_cursor_round_trip() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(10);
        grid.cursor_mut().set_col(Column(42));
        grid.save_cursor();

        // Move cursor elsewhere.
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));
        assert_eq!(grid.cursor().line(), 0);

        grid.restore_cursor();
        assert_eq!(grid.cursor().line(), 10);
        assert_eq!(grid.cursor().col(), Column(42));
    }

    // --- Additional tests from reference repo gap analysis ---

    #[test]
    fn move_backward_clamps_to_zero() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_col(Column(3));
        grid.move_backward(100);
        assert_eq!(grid.cursor().col(), Column(0));
    }

    #[test]
    fn move_to_clamps_out_of_bounds() {
        let mut grid = Grid::new(24, 80);
        grid.move_to(999, Column(999));
        assert_eq!(grid.cursor().line(), 23);
        assert_eq!(grid.cursor().col(), Column(79));
    }

    #[test]
    fn move_to_column_clamps_to_last() {
        let mut grid = Grid::new(24, 80);
        grid.move_to_column(Column(999));
        assert_eq!(grid.cursor().col(), Column(79));
    }

    #[test]
    fn move_to_line_clamps_to_last() {
        let mut grid = Grid::new(24, 80);
        grid.move_to_line(999);
        assert_eq!(grid.cursor().line(), 23);
    }

    #[test]
    fn next_line_combines_cr_and_lf() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(5);
        grid.cursor_mut().set_col(Column(40));
        grid.next_line();
        assert_eq!(grid.cursor().line(), 6);
        assert_eq!(grid.cursor().col(), Column(0));
    }

    #[test]
    fn linefeed_at_last_line_outside_scroll_region_is_noop() {
        let mut grid = Grid::new(5, 10);
        grid.scroll_region = 0..3;
        grid.cursor_mut().set_line(4);
        grid.linefeed();
        // Cursor at last line, outside scroll region bottom: no movement.
        assert_eq!(grid.cursor().line(), 4);
    }

    #[test]
    fn reverse_index_in_middle_moves_up() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(5);
        grid.reverse_index();
        assert_eq!(grid.cursor().line(), 4);
    }

    #[test]
    fn reverse_index_at_line_zero_outside_scroll_region_is_noop() {
        let mut grid = Grid::new(5, 10);
        grid.scroll_region = 2..5;
        grid.cursor_mut().set_line(0);
        grid.reverse_index();
        // Line 0 is outside scroll region; already at 0, can't go further.
        assert_eq!(grid.cursor().line(), 0);
    }

    #[test]
    fn tab_from_col_zero() {
        let mut grid = Grid::new(24, 80);
        // Col 0 is a tab stop; next stop is at col 8.
        grid.tab();
        assert_eq!(grid.cursor().col(), Column(8));
    }

    #[test]
    fn tab_backward_at_col_zero_stays() {
        let mut grid = Grid::new(24, 80);
        grid.tab_backward();
        assert_eq!(grid.cursor().col(), Column(0));
    }

    #[test]
    fn tab_after_clearing_all_stops_goes_to_end() {
        let mut grid = Grid::new(24, 80);
        grid.clear_tab_stop(TabClearMode::All);
        grid.cursor_mut().set_col(Column(5));
        grid.tab();
        // No tab stops anywhere: go to last column.
        assert_eq!(grid.cursor().col(), Column(79));
    }

    #[test]
    fn restore_cursor_without_save_resets_to_origin() {
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(10);
        grid.cursor_mut().set_col(Column(40));
        // No save_cursor() call.
        grid.restore_cursor();
        assert_eq!(grid.cursor().line(), 0);
        assert_eq!(grid.cursor().col(), Column(0));
    }

    #[test]
    fn scroll_region_up_preserves_content_outside() {
        let mut grid = Grid::new(5, 10);
        // Write identifiable chars on each line.
        for line in 0..5 {
            grid.cursor_mut().set_line(line);
            grid.cursor_mut().set_col(Column(0));
            grid.put_char((b'A' + line as u8) as char);
        }
        // Set scroll region to lines 1..4 (middle three lines).
        grid.scroll_region = 1..4;
        grid.cursor_mut().set_line(3);
        // Linefeed at bottom of scroll region triggers scroll up.
        grid.linefeed();

        // Line 0 ('A') should be untouched.
        assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
        // Line 4 ('E') should be untouched.
        assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
        // Inside region: line 1 now has what was line 2 ('C').
        assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
        // Line 2 now has what was line 3 ('D').
        assert_eq!(grid[Line(2)][Column(0)].ch, 'D');
        // Line 3 is the new blank row.
        assert!(grid[Line(3)][Column(0)].is_empty());
    }

    #[test]
    fn scroll_region_down_preserves_content_outside() {
        let mut grid = Grid::new(5, 10);
        for line in 0..5 {
            grid.cursor_mut().set_line(line);
            grid.cursor_mut().set_col(Column(0));
            grid.put_char((b'A' + line as u8) as char);
        }
        // Set scroll region to lines 1..4.
        grid.scroll_region = 1..4;
        grid.cursor_mut().set_line(1);
        // Reverse index at top of scroll region triggers scroll down.
        grid.reverse_index();

        // Line 0 ('A') untouched.
        assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
        // Line 4 ('E') untouched.
        assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
        // Line 1 is the new blank row (inserted at top of region).
        assert!(grid[Line(1)][Column(0)].is_empty());
        // Line 2 now has what was line 1 ('B').
        assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
        // Line 3 now has what was line 2 ('C').
        assert_eq!(grid[Line(3)][Column(0)].ch, 'C');
    }

    #[test]
    fn scroll_region_fill_uses_bce_background() {
        use vte::ansi::Color;
        let mut grid = Grid::new(3, 10);
        grid.put_char('A');
        grid.cursor_mut().set_line(2);
        grid.cursor_mut().set_col(Column(0));
        grid.cursor_mut().template.bg = Color::Indexed(4);
        // Linefeed at bottom triggers scroll up with BCE.
        grid.linefeed();
        // The new bottom row should have the cursor's bg color.
        assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(4));
        assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(4));
    }

    #[test]
    fn move_up_clamped_to_scroll_region_top() {
        let mut grid = Grid::new(10, 80);
        grid.scroll_region = 3..8;
        grid.cursor_mut().set_line(5);
        grid.move_up(100);
        assert_eq!(grid.cursor().line(), 3);
    }

    #[test]
    fn move_down_clamped_to_scroll_region_bottom() {
        let mut grid = Grid::new(10, 80);
        grid.scroll_region = 3..8;
        grid.cursor_mut().set_line(5);
        grid.move_down(100);
        // Clamped to scroll_region.end - 1.
        assert_eq!(grid.cursor().line(), 7);
    }

    #[test]
    fn move_up_outside_scroll_region_clamps_to_zero() {
        let mut grid = Grid::new(10, 80);
        grid.scroll_region = 3..8;
        // Cursor outside scroll region (line 1).
        grid.cursor_mut().set_line(1);
        grid.move_up(100);
        assert_eq!(grid.cursor().line(), 0);
    }

    #[test]
    fn move_down_outside_scroll_region_clamps_to_last() {
        let mut grid = Grid::new(10, 80);
        grid.scroll_region = 3..8;
        // Cursor outside scroll region (line 9).
        grid.cursor_mut().set_line(9);
        grid.move_down(100);
        assert_eq!(grid.cursor().line(), 9);
    }

    #[test]
    fn save_cursor_preserves_template() {
        use vte::ansi::Color;
        let mut grid = Grid::new(24, 80);
        grid.cursor_mut().set_line(3);
        grid.cursor_mut().set_col(Column(7));
        grid.cursor_mut().template.fg = Color::Indexed(1);
        grid.cursor_mut().template.flags = crate::cell::CellFlags::BOLD;
        grid.save_cursor();

        // Change cursor state.
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().template.fg = Color::Named(vte::ansi::NamedColor::Foreground);
        grid.cursor_mut().template.flags = crate::cell::CellFlags::empty();

        grid.restore_cursor();
        assert_eq!(grid.cursor().line(), 3);
        assert_eq!(grid.cursor().col(), Column(7));
        assert_eq!(grid.cursor().template.fg, Color::Indexed(1));
        assert!(grid.cursor().template.flags.contains(crate::cell::CellFlags::BOLD));
    }
}
