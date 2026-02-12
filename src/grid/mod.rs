pub mod cursor;
pub mod row;

use std::collections::VecDeque;

use vte::ansi::{ClearMode, LineClearMode, TabulationClearMode};

use crate::cell::{Cell, CellFlags};
use cursor::Cursor;
use row::Row;

pub const DEFAULT_TAB_INTERVAL: usize = 8;

#[derive(Debug, Clone)]
pub struct Grid {
    rows: Vec<Row>,
    pub cols: usize,
    pub lines: usize,
    pub cursor: Cursor,
    pub saved_cursor: Option<Cursor>,
    scroll_top: usize,
    scroll_bottom: usize,
    pub tab_stops: Vec<bool>,
    pub scrollback: VecDeque<Row>,
    pub max_scrollback: usize,
    pub display_offset: usize,
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
            cursor: Cursor::new(cols, lines),
            saved_cursor: None,
            scroll_top: 0,
            scroll_bottom: lines.saturating_sub(1),
            tab_stops,
            scrollback: VecDeque::new(),
            max_scrollback,
            display_offset: 0,
        }
    }

    fn build_tab_stops(cols: usize) -> Vec<bool> {
        let mut stops = vec![false; cols];
        for i in (DEFAULT_TAB_INTERVAL..cols).step_by(DEFAULT_TAB_INTERVAL) {
            stops[i] = true;
        }
        stops
    }

    // --- Row access ---

    pub fn row(&self, line: usize) -> &Row {
        &self.rows[line]
    }

    pub fn row_mut(&mut self, line: usize) -> &mut Row {
        &mut self.rows[line]
    }

    // --- Viewport rows for rendering ---

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

    /// Access a row by absolute index (scrollback row 0 = oldest).
    pub fn absolute_row(&self, abs_row: usize) -> Option<&Row> {
        let sb_len = self.scrollback.len();
        if abs_row < sb_len {
            Some(&self.scrollback[abs_row])
        } else {
            self.rows.get(abs_row - sb_len)
        }
    }

    // --- Character output ---

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
        if col > 0 && self.rows[row][col].flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            self.rows[row][col - 1].c = ' ';
            self.rows[row][col - 1].flags.remove(CellFlags::WIDE_CHAR);
        }
        // Clear wide char if we're overwriting the wide char itself
        if self.rows[row][col].flags.contains(CellFlags::WIDE_CHAR) && col + 1 < self.cols {
            self.rows[row][col + 1].c = ' ';
            self.rows[row][col + 1].flags.remove(CellFlags::WIDE_CHAR_SPACER);
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

    fn wrap_cursor(&mut self) {
        // Set WRAPLINE flag on current row
        let row = self.cursor.row;
        if self.cols > 0 {
            self.rows[row][self.cols - 1].flags.insert(CellFlags::WRAPLINE);
        }

        self.cursor.col = 0;
        self.cursor.input_needs_wrap = false;

        if self.cursor.row >= self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.row += 1;
        }
    }

    // --- Line operations ---

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

    #[allow(clippy::else_if_without_else)]
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

    // --- Scrolling ---

    pub fn scroll_up(&mut self, count: usize) {
        self.scroll_up_in_region(self.scroll_top, self.scroll_bottom, count);
    }

    pub fn scroll_down(&mut self, count: usize) {
        self.scroll_down_in_region(self.scroll_top, self.scroll_bottom, count);
    }

    #[allow(clippy::else_if_without_else)]
    fn scroll_up_in_region(&mut self, top: usize, bottom: usize, count: usize) {
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

    fn scroll_down_in_region(&mut self, top: usize, bottom: usize, count: usize) {
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

    // --- Erase operations ---

    #[allow(clippy::needless_pass_by_value)]
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

    #[allow(clippy::needless_pass_by_value)]
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

    // --- Insert / Delete ---

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

    // --- Cursor positioning ---

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

    // --- Cursor save / restore ---

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

    // --- Scroll region ---

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

    // --- Tab stops ---

    pub fn set_tab_stop(&mut self) {
        if self.cursor.col < self.cols {
            self.tab_stops[self.cursor.col] = true;
        }
    }

    #[allow(clippy::needless_pass_by_value)]
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

    // --- Resize ---

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

    fn resize_rows(&mut self, new_lines: usize) {
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
    #[allow(clippy::else_if_without_else)]
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
            // A row is wrapped if WRAPLINE is set at the old column boundary
            let wrapped = old_cols > 0
                && src_row.len() >= old_cols
                && src_row[old_cols - 1].flags.contains(CellFlags::WRAPLINE);

            // Wrapped rows: all cells up to old_cols are content
            // Non-wrapped rows: trim trailing blanks
            let content_len = if wrapped { old_cols } else { src_row.content_len() };

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
                        out_row[out_col].flags.insert(CellFlags::LEADING_WIDE_CHAR_SPACER);
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
    fn trim_trailing_blank_rows(&mut self, max: usize) -> usize {
        let mut trimmed = 0;
        while trimmed < max && self.rows.len() > 1 {
            let last_idx = self.rows.len() - 1;
            // Don't trim the row the cursor is on or above
            if last_idx <= self.cursor.row {
                break;
            }
            // Check if the last row is blank
            let is_blank = self.rows[last_idx].iter().all(|c| c.c == ' ' || c.c == '\0');
            if !is_blank {
                break;
            }
            self.rows.pop();
            trimmed += 1;
        }
        trimmed
    }

    // --- Utility ---

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
        for r in 0..self.lines {
            for c in 0..self.cols {
                self.rows[r][c].c = 'E';
                self.rows[r][c].fg = Cell::default().fg;
                self.rows[r][c].bg = Cell::default().bg;
                self.rows[r][c].flags = CellFlags::empty();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_grid() {
        let g = Grid::new(80, 24);
        assert_eq!(g.cols, 80);
        assert_eq!(g.lines, 24);
        assert_eq!(g.cursor.col, 0);
        assert_eq!(g.cursor.row, 0);
    }

    #[test]
    fn put_char_advances_cursor() {
        let mut g = Grid::new(80, 24);
        g.put_char('A');
        assert_eq!(g.cursor.col, 1);
        assert_eq!(g.row(0)[0].c, 'A');
    }

    #[test]
    fn wrap_at_end_of_line() {
        let mut g = Grid::new(5, 3);
        for c in "hello".chars() {
            g.put_char(c);
        }
        // After 5 chars in 5-col grid, cursor is at col 4 with wrap pending
        assert!(g.cursor.input_needs_wrap);
        assert_eq!(g.cursor.col, 4);

        g.put_char('!');
        assert_eq!(g.cursor.row, 1);
        assert_eq!(g.cursor.col, 1);
        assert_eq!(g.row(1)[0].c, '!');
    }

    #[test]
    fn scroll_up_pushes_to_scrollback() {
        let mut g = Grid::new(5, 3);
        // Fill line 0
        for c in "ABCDE".chars() {
            g.put_char(c);
        }
        g.newline();
        g.carriage_return();
        // Fill line 1
        for c in "FGHIJ".chars() {
            g.put_char(c);
        }
        g.newline();
        g.carriage_return();
        // Fill line 2
        for c in "KLMNO".chars() {
            g.put_char(c);
        }
        // Now newline should scroll
        g.newline();
        assert_eq!(g.scrollback.len(), 1);
        assert_eq!(g.scrollback[0][0].c, 'A');
    }

    #[test]
    fn erase_display_below() {
        let mut g = Grid::new(10, 5);
        for c in "Hello".chars() {
            g.put_char(c);
        }
        g.cursor.col = 2;
        g.erase_display(ClearMode::Below);
        assert_eq!(g.row(0)[0].c, 'H');
        assert_eq!(g.row(0)[1].c, 'e');
        assert_eq!(g.row(0)[2].c, ' ');
    }

    #[test]
    fn tab_stops() {
        let g = Grid::new(80, 24);
        assert!(g.tab_stops[8]);
        assert!(g.tab_stops[16]);
        assert!(!g.tab_stops[0]);
        assert!(!g.tab_stops[7]);
    }

    #[test]
    fn advance_tab() {
        let mut g = Grid::new(80, 24);
        g.cursor.col = 0;
        g.advance_tab(1);
        assert_eq!(g.cursor.col, 8);
        g.advance_tab(1);
        assert_eq!(g.cursor.col, 16);
    }

    #[test]
    fn scroll_region() {
        let mut g = Grid::new(10, 5);
        g.set_scroll_region(1, Some(3));
        // Put content in row 1
        g.cursor.row = 1;
        for c in "AAAAAAAAAA".chars() {
            g.put_char(c);
        }
        g.cursor.input_needs_wrap = false;
        g.cursor.row = 2;
        g.cursor.col = 0;
        for c in "BBBBBBBBBB".chars() {
            g.put_char(c);
        }
        g.cursor.input_needs_wrap = false;
        g.cursor.row = 3;
        g.cursor.col = 0;
        for c in "CCCCCCCCCC".chars() {
            g.put_char(c);
        }

        g.scroll_up_in_region(1, 3, 1);
        // Row 1 should now be B, row 2 should be C, row 3 should be blank
        assert_eq!(g.row(1)[0].c, 'B');
        assert_eq!(g.row(2)[0].c, 'C');
        assert_eq!(g.row(3)[0].c, ' ');
    }

    #[test]
    fn resize_grow_cols() {
        let mut g = Grid::new(10, 5);
        g.put_char('A');
        g.resize(20, 5, false);
        assert_eq!(g.cols, 20);
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0).len(), 20);
    }

    #[test]
    fn resize_shrink_rows() {
        let mut g = Grid::new(10, 5);
        g.cursor.row = 4;
        g.cursor.col = 0;
        g.put_char('X');
        g.resize(10, 3, false);
        assert_eq!(g.lines, 3);
        // Cursor should be clamped
        assert!(g.cursor.row < 3);
    }

    #[test]
    fn insert_blank_chars() {
        let mut g = Grid::new(10, 1);
        for c in "ABCDE".chars() {
            g.put_char(c);
        }
        g.cursor.col = 1;
        g.insert_blank_chars(2);
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0)[1].c, ' ');
        assert_eq!(g.row(0)[2].c, ' ');
        assert_eq!(g.row(0)[3].c, 'B');
    }

    #[test]
    fn delete_chars() {
        let mut g = Grid::new(10, 1);
        for c in "ABCDE".chars() {
            g.put_char(c);
        }
        g.cursor.col = 1;
        g.delete_chars(2);
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0)[1].c, 'D');
        assert_eq!(g.row(0)[2].c, 'E');
    }

    #[test]
    fn reflow_shrink_wraps_long_line() {
        // Write "ABC" into a 3-col, 5-line grid, then shrink to 2 cols
        // Content: "ABC" (3 chars) wraps into "AB" + "C" = 2 rows
        // With 4 empty rows + 2 content rows = 6 total for 5 visible -> 1 to scrollback
        // Use a setup where we can verify both scrollback and visible content
        let mut g = Grid::new(4, 5);
        for c in "ABCD".chars() {
            g.put_char(c);
        }
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0)[3].c, 'D');

        g.resize(2, 5, true);
        // "ABCD" wraps to "AB" + "CD" = 2 rows, + 4 empty = 6 total
        // 6 - 5 = 1 row to scrollback
        assert_eq!(g.cols, 2);
        assert_eq!(g.scrollback.len(), 1);
        assert_eq!(g.scrollback[0][0].c, 'A');
        assert_eq!(g.scrollback[0][1].c, 'B');
        assert!(g.scrollback[0][1].flags.contains(CellFlags::WRAPLINE));
        assert_eq!(g.row(0)[0].c, 'C');
        assert_eq!(g.row(0)[1].c, 'D');
    }

    #[test]
    fn reflow_grow_unwraps_line() {
        // Create a wrapped line by writing "ABCDEFGH" in 5 cols, then grow to 10
        let mut g = Grid::new(5, 3);
        for c in "ABCDEFGH".chars() {
            g.put_char(c);
        }
        // "ABCDE" on row 0 (wrapped), "FGH" on row 1
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(1)[0].c, 'F');
        assert!(g.row(0)[4].flags.contains(CellFlags::WRAPLINE));

        g.resize(10, 3, true);
        // Should have merged: "ABCDEFGH" on row 0
        assert_eq!(g.cols, 10);
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0)[5].c, 'F');
        assert_eq!(g.row(0)[7].c, 'H');
    }

    #[test]
    fn reflow_roundtrip() {
        // Shrink then grow should restore content via scrollback merge
        let mut g = Grid::new(4, 5);
        for c in "ABCD".chars() {
            g.put_char(c);
        }

        g.resize(2, 5, true);
        // "ABCD" wraps to "AB" + "CD"
        // 1 row goes to scrollback
        assert_eq!(g.scrollback.len(), 1);
        assert_eq!(g.scrollback[0][0].c, 'A');
        assert_eq!(g.row(0)[0].c, 'C');

        g.resize(4, 5, true);
        // Grow should merge "AB" (scrollback, WRAPLINE) + "CD" (visible)
        // into "ABCD" on one row
        assert_eq!(g.row(0)[0].c, 'A');
        assert_eq!(g.row(0)[2].c, 'C');
        assert_eq!(g.row(0)[3].c, 'D');
        assert_eq!(g.scrollback.len(), 0);
    }

    #[test]
    fn reflow_shrink_preserves_cursor() {
        let mut g = Grid::new(10, 5);
        for c in "ABCDEFGHIJ".chars() {
            g.put_char(c);
        }
        g.cursor.col = 7; // Position at 'H'
        g.cursor.input_needs_wrap = false;

        g.resize(5, 5, true);
        // 'H' is at index 7, which is in the second piece (col 2)
        assert_eq!(g.cursor.col, 2);
    }

    #[test]
    fn reflow_shrink_overflow_to_scrollback() {
        // When shrink creates more rows than visible, extra goes to scrollback
        let mut g = Grid::new(10, 2);
        for c in "ABCDEFGHIJ".chars() {
            g.put_char(c);
        }
        g.newline();
        g.carriage_return();
        for c in "KLMNO".chars() {
            g.put_char(c);
        }

        g.resize(5, 2, true);
        // Row 0 ("ABCDEFGHIJ") wraps to "ABCDE" + "FGHIJ" = 2 rows
        // Row 1 ("KLMNO") stays as 1 row
        // Total 3 rows for 2 visible lines -> 1 goes to scrollback
        assert_eq!(g.scrollback.len(), 1);
        assert_eq!(g.scrollback[0][0].c, 'A');
    }

    #[test]
    fn wide_char_occupies_two_cells() {
        let mut g = Grid::new(10, 1);
        g.put_wide_char('漢');
        assert_eq!(g.row(0)[0].c, '漢');
        assert!(g.row(0)[0].flags.contains(CellFlags::WIDE_CHAR));
        assert!(g.row(0)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
        assert_eq!(g.cursor.col, 2);
    }

    #[test]
    fn wide_char_at_end_of_line_wraps() {
        let mut g = Grid::new(5, 2);
        g.cursor.col = 4; // Last column
        g.put_wide_char('漢');
        // Should place LEADING_WIDE_CHAR_SPACER at col 4 and wrap
        assert!(g.row(0)[4].flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER));
        assert_eq!(g.row(1)[0].c, '漢');
        assert!(g.row(1)[0].flags.contains(CellFlags::WIDE_CHAR));
        assert!(g.row(1)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    }

    #[test]
    fn overwrite_wide_char_clears_spacer() {
        let mut g = Grid::new(10, 1);
        g.put_wide_char('漢');
        g.cursor.col = 0;
        g.put_char('a');
        assert_eq!(g.row(0)[0].c, 'a');
        assert!(!g.row(0)[0].flags.contains(CellFlags::WIDE_CHAR));
        assert!(!g.row(0)[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    }

    #[test]
    fn combining_mark_stored_in_cell() {
        let mut g = Grid::new(10, 1);
        g.put_char('e');
        // Attach combining acute accent to previous cell
        let col = g.cursor.col - 1;
        g.row_mut(0)[col].push_zerowidth('\u{0301}');
        assert_eq!(g.row(0)[0].c, 'e');
        assert_eq!(g.row(0)[0].zerowidth(), &['\u{0301}']);
    }

    #[test]
    fn zerowidth_on_wide_char() {
        let mut g = Grid::new(10, 1);
        g.put_wide_char('漢');
        // Attach zerowidth to the base cell (not the spacer)
        g.row_mut(0)[0].push_zerowidth('\u{0301}');
        assert_eq!(g.row(0)[0].zerowidth(), &['\u{0301}']);
        assert!(g.row(0)[1].zerowidth().is_empty());
    }
}
