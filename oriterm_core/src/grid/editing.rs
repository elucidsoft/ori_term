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
mod tests;
