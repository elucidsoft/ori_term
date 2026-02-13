//! Scroll operations: scroll up/down, newline, linefeed, reverse index.

use super::Grid;
use super::row::Row;

impl Grid {
    pub fn newline(&mut self) {
        self.cursor.input_needs_wrap = false;
        if self.cursor.row >= self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.row += 1;
        }
    }

    pub fn carriage_return(&mut self) {
        self.cursor.col = 0;
        self.cursor.input_needs_wrap = false;
    }

    pub fn backspace(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            self.cursor.input_needs_wrap = false;
        }
    }

    #[allow(clippy::else_if_without_else, reason = "No else needed for boundary condition")]
    pub fn reverse_index(&mut self) {
        if self.cursor.row == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    pub fn linefeed(&mut self) {
        self.newline();
    }

    pub fn scroll_up(&mut self, count: usize) {
        self.scroll_up_in_region(self.scroll_top, self.scroll_bottom, count);
    }

    pub fn scroll_down(&mut self, count: usize) {
        self.scroll_down_in_region(self.scroll_top, self.scroll_bottom, count);
    }

    #[allow(clippy::else_if_without_else, reason = "Conditional offset update, no else needed")]
    pub(super) fn scroll_up_in_region(&mut self, top: usize, bottom: usize, count: usize) {
        if top > bottom || bottom >= self.lines {
            return;
        }
        let count = count.min(bottom - top + 1);

        // Full-screen scroll with top == 0: use ring rotation (O(1) per row).
        if top == 0 && bottom == self.lines.saturating_sub(1) {
            for _ in 0..count {
                // Rotate gives us the old top row (now at logical bottom).
                let old_top = self.viewport.rotate_up();
                let scrolled = std::mem::replace(old_top, Row::new(self.cols));

                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                    self.total_evicted += 1;
                    if self.display_offset > 0 {
                        self.display_offset = self.display_offset.saturating_sub(1);
                    }
                } else if self.display_offset > 0 {
                    self.display_offset += 1;
                }
                self.scrollback.push_back(scrolled);
            }
            self.dirty.mark_all();
            return;
        }

        // Scroll region or top > 0: use remove/insert on the ring (O(region)).
        for _ in 0..count {
            // Clone the top row of the region for scrollback.
            if top == 0 {
                let scrolled_row = self.viewport[0].clone();
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                    self.total_evicted += 1;
                    if self.display_offset > 0 {
                        self.display_offset = self.display_offset.saturating_sub(1);
                    }
                } else if self.display_offset > 0 {
                    self.display_offset += 1;
                }
                self.scrollback.push_back(scrolled_row);
            }
            self.viewport.remove_insert(top, bottom, self.cols);
        }
        self.dirty.mark_range(top, bottom);
    }

    pub(super) fn scroll_down_in_region(&mut self, top: usize, bottom: usize, count: usize) {
        if top > bottom || bottom >= self.lines {
            return;
        }
        let count = count.min(bottom - top + 1);

        for _ in 0..count {
            self.viewport.remove_insert(bottom, top, self.cols);
        }
        self.dirty.mark_range(top, bottom);
    }
}
