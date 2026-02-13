//! Character input pipeline.

use unicode_width::UnicodeWidthChar;

use crate::cell::CellFlags;

use super::{TermHandler, prev_base_col};

impl TermHandler<'_> {
    pub(super) fn handle_input(&mut self, c: char) {
        // Apply charset mapping (e.g., DEC Special Graphics for box-drawing)
        let c = self.charset.map(c);
        let grid = if *self.active_is_alt {
            &mut *self.alt_grid
        } else {
            &mut *self.grid
        };
        let width = UnicodeWidthChar::width(c);

        // ZWJ continuation: after a ZWJ, the next printable char joins the cluster
        // (e.g., family emoji: 👩 + ZWJ + 👩 + ZWJ + 👧 + ZWJ + 👦)
        if self.grapheme.after_zwj {
            if let Some(w) = width {
                if w > 0 {
                    self.grapheme.after_zwj = false;
                    let row = self.grapheme.base_row;
                    let col = self.grapheme.base_col;
                    if row < grid.lines && col < grid.cols {
                        grid.row_mut(row)[col].push_zerowidth(c);
                    }
                    return;
                }
            } else {
                // None width (control char) — abandon ZWJ state
                self.grapheme.after_zwj = false;
            }
        }

        // Emoji skin tone modifiers (U+1F3FB-U+1F3FF): attach to previous wide
        // char (emoji) as zerowidth rather than occupying a new cell.
        if matches!(c, '\u{1F3FB}'..='\u{1F3FF}') {
            if let Some(prev_col) = prev_base_col(grid) {
                let row = grid.cursor.row;
                if grid.row(row)[prev_col].flags.contains(CellFlags::WIDE_CHAR) {
                    grid.row_mut(row)[prev_col].push_zerowidth(c);
                    return;
                }
            }
            // Not following a wide char — fall through to normal handling
        }

        match width {
            Some(2) => grid.put_wide_char(c),
            Some(0) => {
                // Zero-width: attach to previous cell, skipping wide char spacers.
                // When input_needs_wrap is true, cursor.col points at the cell
                // we just wrote (it was clamped back after advancing past the end).
                if let Some(col) = prev_base_col(grid) {
                    let row = grid.cursor.row;
                    grid.row_mut(row)[col].push_zerowidth(c);

                    // Track ZWJ for grapheme cluster continuation
                    if c == '\u{200D}' {
                        self.grapheme.after_zwj = true;
                        self.grapheme.base_row = row;
                        self.grapheme.base_col = col;
                    }
                }
            }
            _ => grid.put_char(c),
        }
    }
}
