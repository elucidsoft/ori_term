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

        for _ in 0..count {
            // Remove the top row (shifts all higher indices down by 1 via memmove).
            let scrolled_row = self.rows.remove(top);

            // Push to scrollback only if scrolling the full screen region at top
            if top == 0 {
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                    self.total_evicted += 1;
                    // If we evicted from scrollback while user is scrolled up,
                    // reduce offset so they don't drift past the top
                    if self.display_offset > 0 {
                        self.display_offset = self.display_offset.saturating_sub(1);
                    }
                } else if self.display_offset > 0 {
                    // Scrollback grew — bump offset to keep viewport anchored
                    self.display_offset += 1;
                }
                self.scrollback.push_back(scrolled_row);
            }

            // Insert a fresh row at the bottom position (after remove, bottom is
            // now at index `bottom - 1`, so inserting at `bottom` restores the
            // original row count within the region).
            self.rows.insert(bottom, Row::new(self.cols));
        }
    }

    pub(super) fn scroll_down_in_region(&mut self, top: usize, bottom: usize, count: usize) {
        if top > bottom || bottom >= self.lines {
            return;
        }
        let count = count.min(bottom - top + 1);

        for _ in 0..count {
            // Remove the bottom row and insert a fresh one at top.
            self.rows.remove(bottom);
            self.rows.insert(top, Row::new(self.cols));
        }
    }
}
