//! Grid resize and column reflow (Ghostty-style cell-by-cell rewriting).

use crate::cell::{Cell, CellFlags};

use super::Grid;
use super::row::Row;

impl Grid {
    /// Resize the grid to new dimensions.
    ///
    /// When `reflow` is true, soft-wrapped lines are re-wrapped to fit the new
    /// column width (Ghostty-style cell-by-cell rewriting). When false, rows
    /// are simply truncated or extended (for alt screen).
    pub fn resize(&mut self, new_cols: usize, new_lines: usize, reflow: bool) {
        if new_cols == 0 || new_lines == 0 {
            return;
        }
        if new_cols == self.cols && new_lines == self.lines {
            return;
        }

        if reflow && new_cols != self.cols {
            if new_cols > self.cols {
                // Growing cols: reflow first (unwrap), then adjust rows
                self.reflow_cols(new_cols);
                self.cols = new_cols;
                self.tab_stops = Self::build_tab_stops(new_cols);
                self.resize_rows(new_lines);
            } else {
                // Shrinking cols: adjust rows first, then reflow (wrap)
                self.resize_rows(new_lines);
                self.reflow_cols(new_cols);
                self.cols = new_cols;
                self.tab_stops = Self::build_tab_stops(new_cols);
            }
        } else {
            // No reflow: just resize dimensions
            self.resize_rows(new_lines);

            if new_cols != self.cols {
                for row in &mut self.rows {
                    row.resize(new_cols);
                }
                for row in &mut self.scrollback {
                    row.resize(new_cols);
                }
                self.cols = new_cols;
                self.tab_stops = Self::build_tab_stops(new_cols);
            }
        }

        self.lines = new_lines;

        // Reset scroll region
        self.scroll_top = 0;
        self.scroll_bottom = self.lines.saturating_sub(1);

        // Clamp cursor
        self.cursor.row = self.cursor.row.min(self.lines.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
        self.cursor.input_needs_wrap = false;

        // Clamp display offset
        self.display_offset = self.display_offset.min(self.scrollback.len());
    }

    pub(super) fn resize_rows(&mut self, new_lines: usize) {
        match new_lines.cmp(&self.lines) {
            std::cmp::Ordering::Less => {
                // Shrinking: prefer trimming trailing blank rows first
                let to_remove = self.lines - new_lines;
                let trimmed = self.trim_trailing_blank_rows(to_remove);
                let remaining = to_remove - trimmed;

                for _ in 0..remaining {
                    if !self.rows.is_empty() {
                        let row = self.rows.remove(0);
                        if self.scrollback.len() >= self.max_scrollback {
                            self.scrollback.pop_front();
                        }
                        self.scrollback.push_back(row);
                        self.cursor.row = self.cursor.row.saturating_sub(1);
                    }
                }

                self.rows.truncate(new_lines);
                while self.rows.len() < new_lines {
                    self.rows.push(Row::new(self.cols));
                }
            }
            std::cmp::Ordering::Greater => {
                let delta = new_lines - self.lines;

                if self.cursor.row < self.lines.saturating_sub(1) {
                    for _ in 0..delta {
                        self.rows.push(Row::new(self.cols));
                    }
                } else {
                    let from_scrollback = delta.min(self.scrollback.len());
                    let mut prepend = Vec::new();
                    for _ in 0..from_scrollback {
                        prepend.push(self.scrollback.pop_back().expect("checked len"));
                    }
                    prepend.reverse();
                    self.cursor.row += from_scrollback;

                    let mut new_rows = prepend;
                    new_rows.append(&mut self.rows);
                    while new_rows.len() < new_lines {
                        new_rows.push(Row::new(self.cols));
                    }
                    self.rows = new_rows;
                }
            }
            std::cmp::Ordering::Equal => {}
        }
        self.lines = new_lines;
    }

    /// Reflow content to fit new column width using cell-by-cell rewriting.
    ///
    /// Unified function that handles both growing (unwrapping) and shrinking
    /// (wrapping) columns, inspired by Ghostty's reflow approach. Iterates all
    /// cells from all rows (scrollback + visible) and writes them into new
    /// output rows at the target width.
    #[allow(clippy::else_if_without_else, reason = "Wrapped row conditional, no else needed")]
    fn reflow_cols(&mut self, new_cols: usize) {
        let old_cols = self.cols;
        if old_cols == new_cols || new_cols == 0 {
            return;
        }

        // Collect all rows: scrollback first, then visible
        let mut all_rows: Vec<Row> = self.scrollback.drain(..).collect();
        let visible_start = all_rows.len();
        all_rows.append(&mut self.rows);

        // Cursor position in the unified list
        let cursor_abs = visible_start + self.cursor.row;
        let cursor_col = self.cursor.col;
        let mut new_cursor_abs = 0usize;
        let mut new_cursor_col = 0usize;

        let mut result: Vec<Row> = Vec::with_capacity(all_rows.len());
        let mut out_row = Row::new(new_cols);
        let mut out_col = 0usize;

        for (src_idx, src_row) in all_rows.iter().enumerate() {
            // Transfer prompt_start flag to the current output row.
            if src_row.prompt_start {
                out_row.prompt_start = true;
            }

            // A row is wrapped if WRAPLINE is set at the old column boundary
            let wrapped = old_cols > 0
                && src_row.len() >= old_cols
                && src_row[old_cols - 1].flags.contains(CellFlags::WRAPLINE);

            // Wrapped rows: all cells up to old_cols are content
            // Non-wrapped rows: trim trailing blanks
            let content_len = if wrapped {
                old_cols
            } else {
                src_row.content_len()
            };

            // Process each source cell
            for src_col in 0..content_len {
                let cell = &src_row[src_col];

                // Skip generated spacer cells (will be regenerated)
                if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                    if src_idx == cursor_abs && src_col == cursor_col {
                        new_cursor_abs = result.len();
                        new_cursor_col = out_col.saturating_sub(1);
                    }
                    continue;
                }
                if cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER) {
                    if src_idx == cursor_abs && src_col == cursor_col {
                        new_cursor_abs = result.len();
                        new_cursor_col = out_col.min(new_cols.saturating_sub(1));
                    }
                    continue;
                }

                // Treat wide chars as narrow if grid too narrow (new_cols < 2)
                let is_wide = cell.flags.contains(CellFlags::WIDE_CHAR) && new_cols >= 2;
                let cell_width = if is_wide { 2 } else { 1 };

                // Wrap to next output row if cell doesn't fit
                if out_col + cell_width > new_cols {
                    // Pad with spacer if wide char at boundary
                    if is_wide && out_col < new_cols {
                        out_row[out_col] = Cell::default();
                        out_row[out_col]
                            .flags
                            .insert(CellFlags::LEADING_WIDE_CHAR_SPACER);
                    }
                    out_row.occ = new_cols;
                    out_row[new_cols - 1].flags.insert(CellFlags::WRAPLINE);
                    result.push(out_row);
                    out_row = Row::new(new_cols);
                    out_col = 0;
                }

                // Track cursor position
                if src_idx == cursor_abs && src_col == cursor_col {
                    new_cursor_abs = result.len();
                    new_cursor_col = out_col;
                }

                // Write cell (strip old WRAPLINE flag)
                let mut new_cell = cell.clone();
                new_cell.flags.remove(CellFlags::WRAPLINE);
                if !is_wide && cell.flags.contains(CellFlags::WIDE_CHAR) {
                    // Wide char forced narrow due to new_cols < 2
                    new_cell.flags.remove(CellFlags::WIDE_CHAR);
                }
                out_row[out_col] = new_cell;
                out_col += 1;

                // Write wide char spacer in next column
                if is_wide {
                    let mut spacer = Cell::default();
                    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
                    spacer.fg = cell.fg;
                    spacer.bg = cell.bg;
                    out_row[out_col] = spacer;
                    out_col += 1;
                }

                out_row.occ = out_col;
            }

            // End of source row
            if !wrapped {
                // Non-wrapped: finalize output row
                if src_idx == cursor_abs && cursor_col >= content_len {
                    new_cursor_abs = result.len();
                    new_cursor_col = cursor_col.min(new_cols.saturating_sub(1));
                }
                result.push(out_row);
                out_row = Row::new(new_cols);
                out_col = 0;
            } else if src_idx == cursor_abs && cursor_col >= content_len {
                // Wrapped row with cursor past content
                new_cursor_abs = result.len();
                new_cursor_col = out_col.min(new_cols.saturating_sub(1));
            }
        }

        // Push remaining content if last source row was wrapped
        if out_col > 0 {
            result.push(out_row);
        }

        // Ensure at least one row exists
        if result.is_empty() {
            result.push(Row::new(new_cols));
        }

        // Split into scrollback + visible
        let total = result.len();
        if total > self.lines {
            let sb_count = total - self.lines;
            self.scrollback = result.drain(..sb_count).collect();
            self.rows = result;
        } else {
            self.scrollback.clear();
            self.rows = result;
            while self.rows.len() < self.lines {
                self.rows.push(Row::new(new_cols));
            }
        }

        // Ensure all rows have correct width
        for row in &mut self.scrollback {
            row.resize(new_cols);
        }
        for row in &mut self.rows {
            row.resize(new_cols);
        }

        // Update cursor
        let sb_len = self.scrollback.len();
        self.cursor.row = if new_cursor_abs >= sb_len {
            (new_cursor_abs - sb_len).min(self.lines.saturating_sub(1))
        } else {
            0
        };
        self.cursor.col = new_cursor_col.min(new_cols.saturating_sub(1));
    }

    /// Trim up to `max` trailing blank rows from the bottom of the active area.
    /// Returns how many were actually trimmed. Does not trim rows at or above the cursor.
    pub(super) fn trim_trailing_blank_rows(&mut self, max: usize) -> usize {
        let mut trimmed = 0;
        while trimmed < max && self.rows.len() > 1 {
            let last_idx = self.rows.len() - 1;
            // Don't trim the row the cursor is on or above
            if last_idx <= self.cursor.row {
                break;
            }
            // Check if the last row is blank
            let is_blank = self.rows[last_idx]
                .iter()
                .all(|c| c.c == ' ' || c.c == '\0');
            if !is_blank {
                break;
            }
            self.rows.pop();
            trimmed += 1;
        }
        trimmed
    }
}
