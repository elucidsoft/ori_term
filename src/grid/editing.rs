//! Character writing, erasing, and insertion/deletion operations.

use vte::ansi::{ClearMode, LineClearMode};

use crate::cell::CellFlags;

use super::Grid;

impl Grid {
    pub fn put_char(&mut self, c: char) {
        if self.cursor.input_needs_wrap {
            self.wrap_cursor();
        }

        if self.cursor.col >= self.cols {
            self.cursor.col = self.cols.saturating_sub(1);
        }

        let col = self.cursor.col;
        let row = self.cursor.row;

        // Clear wide char spacer if we're overwriting the first cell of a wide char
        if col > 0
            && self.rows[row][col]
                .flags
                .contains(CellFlags::WIDE_CHAR_SPACER)
        {
            self.rows[row][col - 1].c = ' ';
            self.rows[row][col - 1].flags.remove(CellFlags::WIDE_CHAR);
        }
        // Clear wide char if we're overwriting the wide char itself
        if self.rows[row][col].flags.contains(CellFlags::WIDE_CHAR) && col + 1 < self.cols {
            self.rows[row][col + 1].c = ' ';
            self.rows[row][col + 1]
                .flags
                .remove(CellFlags::WIDE_CHAR_SPACER);
        }

        let cell = &mut self.rows[row][col];
        cell.c = c;
        cell.fg = self.cursor.template.fg;
        cell.bg = self.cursor.template.bg;
        cell.flags = self.cursor.template.flags;
        cell.extra = None;

        if col >= self.rows[row].occ {
            self.rows[row].occ = col + 1;
        }

        self.cursor.col += 1;
        if self.cursor.col >= self.cols {
            self.cursor.input_needs_wrap = true;
            self.cursor.col = self.cols - 1;
        }
    }

    pub fn put_wide_char(&mut self, c: char) {
        if self.cursor.input_needs_wrap {
            self.wrap_cursor();
        }

        // If at the last column, we need to wrap
        if self.cursor.col + 1 >= self.cols {
            // Put a spacer at the current position and wrap
            let col = self.cursor.col;
            let row = self.cursor.row;
            self.rows[row][col].c = ' ';
            self.rows[row][col].flags = CellFlags::LEADING_WIDE_CHAR_SPACER;
            self.rows[row].occ = col + 1;
            self.wrap_cursor();
        }

        let col = self.cursor.col;
        let row = self.cursor.row;

        // Write the wide char
        let cell = &mut self.rows[row][col];
        cell.c = c;
        cell.fg = self.cursor.template.fg;
        cell.bg = self.cursor.template.bg;
        cell.flags = self.cursor.template.flags | CellFlags::WIDE_CHAR;
        cell.extra = None;

        // Write spacer in next column
        let spacer = &mut self.rows[row][col + 1];
        spacer.c = ' ';
        spacer.fg = self.cursor.template.fg;
        spacer.bg = self.cursor.template.bg;
        spacer.flags = CellFlags::WIDE_CHAR_SPACER;
        spacer.extra = None;

        if col + 1 >= self.rows[row].occ {
            self.rows[row].occ = col + 2;
        }

        self.cursor.col += 2;
        if self.cursor.col >= self.cols {
            self.cursor.input_needs_wrap = true;
            self.cursor.col = self.cols - 1;
        }
    }

    pub(super) fn wrap_cursor(&mut self) {
        // Set WRAPLINE flag on current row
        let row = self.cursor.row;
        if self.cols > 0 {
            self.rows[row][self.cols - 1]
                .flags
                .insert(CellFlags::WRAPLINE);
        }

        self.cursor.col = 0;
        self.cursor.input_needs_wrap = false;

        if self.cursor.row >= self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.row += 1;
        }
    }

    #[allow(clippy::needless_pass_by_value, reason = "VTE trait requires consuming enum parameter")]
    pub fn erase_display(&mut self, mode: ClearMode) {
        let template = &self.cursor.template;
        match mode {
            ClearMode::Below => {
                // Clear from cursor to end of line
                let row = self.cursor.row;
                let col = self.cursor.col;
                for c in col..self.cols {
                    self.rows[row][c].reset(template);
                }
                // Clear all rows below
                for r in (row + 1)..self.lines {
                    self.rows[r].reset(template);
                }
            }
            ClearMode::Above => {
                // Clear from start to cursor
                let row = self.cursor.row;
                let col = self.cursor.col;
                for r in 0..row {
                    self.rows[r].reset(template);
                }
                for c in 0..=col.min(self.cols.saturating_sub(1)) {
                    self.rows[row][c].reset(template);
                }
            }
            ClearMode::All => {
                for r in 0..self.lines {
                    self.rows[r].reset(template);
                }
            }
            ClearMode::Saved => {
                self.scrollback.clear();
                self.display_offset = 0;
            }
        }
    }

    #[allow(clippy::needless_pass_by_value, reason = "VTE trait requires consuming enum parameter")]
    pub fn erase_line(&mut self, mode: LineClearMode) {
        let template = &self.cursor.template;
        let row = self.cursor.row;
        let col = self.cursor.col;
        match mode {
            LineClearMode::Right => {
                for c in col..self.cols {
                    self.rows[row][c].reset(template);
                }
            }
            LineClearMode::Left => {
                for c in 0..=col.min(self.cols.saturating_sub(1)) {
                    self.rows[row][c].reset(template);
                }
            }
            LineClearMode::All => {
                self.rows[row].reset(template);
            }
        }
    }

    pub fn erase_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let template = self.cursor.template.clone();
        let end = (col + count).min(self.cols);
        for c in col..end {
            self.rows[row][c].reset(&template);
        }
    }

    pub fn insert_blank_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let count = count.min(self.cols.saturating_sub(col));

        // Shift cells right
        for c in (col + count..self.cols).rev() {
            self.rows[row][c] = self.rows[row][c - count].clone();
        }
        // Clear inserted cells
        let template = self.cursor.template.clone();
        for c in col..(col + count).min(self.cols) {
            self.rows[row][c].reset(&template);
        }
    }

    pub fn delete_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let count = count.min(self.cols.saturating_sub(col));

        // Shift cells left
        for c in col..(self.cols - count) {
            self.rows[row][c] = self.rows[row][c + count].clone();
        }
        // Clear trailing cells
        let template = self.cursor.template.clone();
        for c in (self.cols - count)..self.cols {
            self.rows[row][c].reset(&template);
        }
    }

    pub fn insert_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row < self.scroll_top || row > self.scroll_bottom {
            return;
        }
        self.scroll_down_in_region(row, self.scroll_bottom, count);
    }

    pub fn delete_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row < self.scroll_top || row > self.scroll_bottom {
            return;
        }
        self.scroll_up_in_region(row, self.scroll_bottom, count);
    }
}
