//! Cursor movement, tab stops, save/restore cursor, scroll region, and logical line detection.

use vte::ansi::TabulationClearMode;

use crate::cell::CellFlags;

use super::Grid;

/// How to detect that a row continues onto the next row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapDetection {
    /// Strict: only WRAPLINE flag (terminal auto-wrap). Used by selection.
    WrapFlag,
    /// Extended: WRAPLINE flag OR last cell is non-empty. Catches
    /// application-driven wrapping (e.g. CLI that breaks long URLs at
    /// the terminal width). Used by URL detection.
    WrapOrFilled,
}

impl Grid {
    pub fn goto(&mut self, row: usize, col: usize) {
        self.cursor.row = row.min(self.lines.saturating_sub(1));
        self.cursor.col = col.min(self.cols.saturating_sub(1));
        self.cursor.input_needs_wrap = false;
    }

    pub fn goto_line(&mut self, row: usize) {
        self.cursor.row = row.min(self.lines.saturating_sub(1));
        self.cursor.input_needs_wrap = false;
    }

    pub fn goto_col(&mut self, col: usize) {
        self.cursor.col = col.min(self.cols.saturating_sub(1));
        self.cursor.input_needs_wrap = false;
    }

    pub fn move_up(&mut self, n: usize) {
        self.cursor.row = self.cursor.row.saturating_sub(n);
        self.cursor.input_needs_wrap = false;
    }

    pub fn move_down(&mut self, n: usize) {
        self.cursor.row = (self.cursor.row + n).min(self.lines.saturating_sub(1));
        self.cursor.input_needs_wrap = false;
    }

    pub fn move_forward(&mut self, n: usize) {
        self.cursor.col = (self.cursor.col + n).min(self.cols.saturating_sub(1));
        self.cursor.input_needs_wrap = false;
    }

    pub fn move_backward(&mut self, n: usize) {
        self.cursor.col = self.cursor.col.saturating_sub(n);
        self.cursor.input_needs_wrap = false;
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor.clone());
    }

    pub fn restore_cursor(&mut self) {
        if let Some(saved) = self.saved_cursor.clone() {
            self.cursor = saved;
            // Clamp to current dimensions
            self.cursor.row = self.cursor.row.min(self.lines.saturating_sub(1));
            self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
        }
    }

    pub fn set_scroll_region(&mut self, top: usize, bottom: Option<usize>) {
        let bottom = bottom.unwrap_or_else(|| self.lines.saturating_sub(1));
        if top < bottom && bottom < self.lines {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
        }
    }

    pub fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    pub fn scroll_bottom(&self) -> usize {
        self.scroll_bottom
    }

    pub fn set_tab_stop(&mut self) {
        if self.cursor.col < self.cols {
            self.tab_stops[self.cursor.col] = true;
        }
    }

    #[allow(clippy::needless_pass_by_value, reason = "VTE trait requires consuming enum parameter")]
    pub fn clear_tab_stops(&mut self, mode: TabulationClearMode) {
        match mode {
            TabulationClearMode::Current => {
                if self.cursor.col < self.cols {
                    self.tab_stops[self.cursor.col] = false;
                }
            }
            TabulationClearMode::All => {
                self.tab_stops.fill(false);
            }
        }
    }

    pub fn advance_tab(&mut self, count: u16) {
        for _ in 0..count {
            let mut col = self.cursor.col + 1;
            while col < self.cols && !self.tab_stops[col] {
                col += 1;
            }
            self.cursor.col = col.min(self.cols.saturating_sub(1));
        }
    }

    pub fn backward_tab(&mut self, count: u16) {
        for _ in 0..count {
            if self.cursor.col == 0 {
                break;
            }
            let mut col = self.cursor.col - 1;
            while col > 0 && !self.tab_stops[col] {
                col -= 1;
            }
            self.cursor.col = col;
        }
    }

    /// Walk backwards to find the start of a logical (soft-wrapped) line.
    pub fn logical_line_start(&self, abs_row: usize, detection: WrapDetection) -> usize {
        let mut r = abs_row;
        while r > 0 {
            if let Some(prev_row) = self.absolute_row(r - 1) {
                if row_continues(prev_row, detection) {
                    r -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        r
    }

    /// Walk forwards to find the end of a logical (soft-wrapped) line.
    pub fn logical_line_end(&self, abs_row: usize, detection: WrapDetection) -> usize {
        let total = self.scrollback.len() + self.lines;
        let mut r = abs_row;
        while let Some(row) = self.absolute_row(r) {
            if row_continues(row, detection) && r + 1 < total {
                r += 1;
            } else {
                break;
            }
        }
        r
    }
}

/// Check whether a row's content continues onto the next row.
fn row_continues(row: &super::row::Row, detection: WrapDetection) -> bool {
    let cols = row.len();
    if cols == 0 {
        return false;
    }
    let last = &row[cols - 1];
    match detection {
        WrapDetection::WrapFlag => last.flags.contains(CellFlags::WRAPLINE),
        WrapDetection::WrapOrFilled => {
            last.flags.contains(CellFlags::WRAPLINE) || (last.c != '\0' && last.c != ' ')
        }
    }
}
