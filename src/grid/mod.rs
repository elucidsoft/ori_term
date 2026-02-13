//! Terminal grid with scrollback, cursor, and reflow support.

pub mod cursor;
mod editing;
mod navigation;
mod reflow;
pub mod row;
mod scroll;

pub use navigation::WrapDetection;

#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use crate::cell::{Cell, CellFlags};
use cursor::Cursor;
use row::Row;

const DEFAULT_TAB_INTERVAL: usize = 8;

// Grid inset from window edges (used by GPU renderer and app layout).
pub const GRID_PADDING_LEFT: usize = 6;
pub const GRID_PADDING_TOP: usize = 10;
pub const GRID_PADDING_BOTTOM: usize = 4;

#[derive(Debug, Clone)]
pub struct Grid {
    rows: Vec<Row>,
    pub cols: usize,
    pub lines: usize,
    pub cursor: Cursor,
    saved_cursor: Option<Cursor>,
    scroll_top: usize,
    scroll_bottom: usize,
    tab_stops: Vec<bool>,
    pub scrollback: VecDeque<Row>,
    max_scrollback: usize,
    pub display_offset: usize,
    /// Total number of rows evicted from scrollback (for absolute row tracking).
    total_evicted: usize,
}

impl Grid {
    pub fn new(cols: usize, lines: usize) -> Self {
        Self::with_max_scrollback(cols, lines, 10_000)
    }

    pub fn with_max_scrollback(cols: usize, lines: usize, max_scrollback: usize) -> Self {
        let rows = (0..lines).map(|_| Row::new(cols)).collect();
        let tab_stops = Self::build_tab_stops(cols);

        Self {
            rows,
            cols,
            lines,
            cursor: Cursor::default(),
            saved_cursor: None,
            scroll_top: 0,
            scroll_bottom: lines.saturating_sub(1),
            tab_stops,
            scrollback: VecDeque::new(),
            max_scrollback,
            display_offset: 0,
            total_evicted: 0,
        }
    }

    fn build_tab_stops(cols: usize) -> Vec<bool> {
        let mut stops = vec![false; cols];
        for i in (DEFAULT_TAB_INTERVAL..cols).step_by(DEFAULT_TAB_INTERVAL) {
            stops[i] = true;
        }
        stops
    }

    pub fn row(&self, line: usize) -> &Row {
        &self.rows[line]
    }

    pub fn row_mut(&mut self, line: usize) -> &mut Row {
        &mut self.rows[line]
    }

    pub fn visible_row(&self, line: usize) -> &Row {
        if self.display_offset == 0 {
            return &self.rows[line];
        }
        let scrollback_len = self.scrollback.len();
        let offset_line = line as isize - self.display_offset as isize;
        if offset_line < 0 {
            let sb_idx = scrollback_len as isize + offset_line;
            if sb_idx >= 0 && (sb_idx as usize) < scrollback_len {
                return &self.scrollback[sb_idx as usize];
            }
            // Out of range — return first scrollback or first row
            if !self.scrollback.is_empty() {
                return &self.scrollback[0];
            }
            return &self.rows[0];
        }
        &self.rows[offset_line as usize]
    }

    /// Convert a viewport line to an absolute row index.
    ///
    /// Absolute indexing: scrollback\[0\] is the oldest row, then visible rows
    /// follow. This accounts for the current `display_offset` (scroll position).
    pub fn viewport_to_absolute(&self, line: usize) -> usize {
        self.scrollback.len().saturating_sub(self.display_offset) + line
    }

    /// Access a row by absolute index (scrollback row 0 = oldest).
    pub fn absolute_row(&self, abs_row: usize) -> Option<&Row> {
        let sb_len = self.scrollback.len();
        if abs_row < sb_len {
            Some(&self.scrollback[abs_row])
        } else {
            self.rows.get(abs_row - sb_len)
        }
    }

    pub fn clear_all(&mut self) {
        let template = Cell::default();
        for r in 0..self.lines {
            self.rows[r].reset(&template);
        }
        self.cursor.col = 0;
        self.cursor.row = 0;
        self.cursor.input_needs_wrap = false;
    }

    pub fn decaln(&mut self) {
        let default = Cell::default();
        for r in 0..self.lines {
            for c in 0..self.cols {
                self.rows[r][c].c = 'E';
                self.rows[r][c].fg = default.fg;
                self.rows[r][c].bg = default.bg;
                self.rows[r][c].flags = CellFlags::empty();
            }
        }
    }
}
