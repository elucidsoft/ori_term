//! Scroll region management and scroll operations.
//!
//! Provides `set_scroll_region` (DECSTBM), `scroll_up`, `scroll_down`,
//! `insert_lines`, and `delete_lines`. All operations use O(1) rotation
//! of existing row allocations and fill new rows with BCE background.

use std::ops::Range;

use crate::cell::Cell;
use crate::index::Column;

use super::Grid;

impl Grid {
    /// DECSTBM: set the scroll region.
    ///
    /// Parameters are 1-based (matching VTE/ECMA-48). `top` is the first
    /// line of the region, `bottom` is the last line (or `None` for the
    /// screen height). Stored internally as a 0-based half-open range.
    /// Moves the cursor to the origin after setting.
    pub fn set_scroll_region(&mut self, top: usize, bottom: Option<usize>) {
        // 1-based params: top=0 is invalid, treat as 1.
        let top = top.max(1) - 1;
        let bottom = bottom.map_or(self.lines, |b| b.min(self.lines));

        // Region must span at least 2 lines.
        if top + 1 >= bottom {
            return;
        }

        self.scroll_region = top..bottom;
        self.cursor.set_line(0);
        self.cursor.set_col(Column(0));
    }

    /// Scroll the scroll region up by `count` lines.
    ///
    /// Top rows are lost (scrollback not yet implemented). Blank rows
    /// appear at the bottom of the region.
    pub fn scroll_up(&mut self, count: usize) {
        let range = self.scroll_region.clone();
        self.scroll_range_up(range, count);
    }

    /// Scroll the scroll region down by `count` lines.
    ///
    /// Bottom rows are lost. Blank rows appear at the top of the region.
    pub fn scroll_down(&mut self, count: usize) {
        let range = self.scroll_region.clone();
        self.scroll_range_down(range, count);
    }

    /// IL: insert `count` blank lines at the cursor, pushing existing
    /// lines down within the scroll region.
    ///
    /// Only operates if the cursor is inside the scroll region. Lines
    /// pushed past the bottom of the region are lost.
    pub fn insert_lines(&mut self, count: usize) {
        let line = self.cursor.line();
        if line < self.scroll_region.start || line >= self.scroll_region.end {
            return;
        }
        let range = line..self.scroll_region.end;
        self.scroll_range_down(range, count);
    }

    /// DL: delete `count` lines at the cursor, pulling remaining lines
    /// up within the scroll region.
    ///
    /// Only operates if the cursor is inside the scroll region. Blank
    /// lines appear at the bottom of the region.
    pub fn delete_lines(&mut self, count: usize) {
        let line = self.cursor.line();
        if line < self.scroll_region.start || line >= self.scroll_region.end {
            return;
        }
        let range = line..self.scroll_region.end;
        self.scroll_range_up(range, count);
    }

    /// Scroll a range of rows up by `count` using O(1) rotation.
    ///
    /// Top rows rotate to the bottom and are reset with BCE background.
    fn scroll_range_up(&mut self, range: Range<usize>, count: usize) {
        let len = range.end - range.start;
        if len == 0 {
            return;
        }
        let count = count.min(len);
        let template = Cell::from(self.cursor.template.bg);

        self.rows[range.start..range.end].rotate_left(count);

        for i in (range.end - count)..range.end {
            self.rows[i].reset(self.cols, &template);
        }
    }

    /// Scroll a range of rows down by `count` using O(1) rotation.
    ///
    /// Bottom rows rotate to the top and are reset with BCE background.
    fn scroll_range_down(&mut self, range: Range<usize>, count: usize) {
        let len = range.end - range.start;
        if len == 0 {
            return;
        }
        let count = count.min(len);
        let template = Cell::from(self.cursor.template.bg);

        self.rows[range.start..range.end].rotate_right(count);

        for i in range.start..range.start + count {
            self.rows[i].reset(self.cols, &template);
        }
    }
}

#[cfg(test)]
mod tests;
