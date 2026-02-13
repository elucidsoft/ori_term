//! Text extraction from grid selection.

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::grid::row::Row;

use super::{Selection, SelectionMode};

/// Extract selected text from the grid.
pub fn extract_text(grid: &Grid, selection: &Selection) -> String {
    let (start, end) = selection.ordered();
    let mut result = String::new();

    if selection.mode == SelectionMode::Block {
        let min_col = start.col.min(end.col);
        let max_col = start.col.max(end.col);

        for abs_row in start.row..=end.row {
            if let Some(row) = grid.absolute_row(abs_row) {
                let line = cells_to_text(row, min_col, max_col);
                result.push_str(line.trim_end());
            }
            if abs_row < end.row {
                result.push('\n');
            }
        }
    } else {
        for abs_row in start.row..=end.row {
            if let Some(row) = grid.absolute_row(abs_row) {
                let row_start = if abs_row == start.row {
                    start.effective_start_col()
                } else {
                    0
                };
                let row_end = if abs_row == end.row {
                    end.effective_end_col()
                } else {
                    row.len().saturating_sub(1)
                };

                let line = cells_to_text(row, row_start, row_end);

                // Soft-wrapped rows continue without a newline or trailing-space trim.
                let is_wrapped =
                    !row.is_empty() && row[row.len() - 1].flags.contains(CellFlags::WRAPLINE);

                if is_wrapped && abs_row < end.row {
                    result.push_str(&line);
                } else {
                    result.push_str(line.trim_end());
                    if abs_row < end.row {
                        result.push('\n');
                    }
                }
            }
        }
    }

    result
}

/// Collect visible cell characters from `col_start..=col_end` into a string.
/// Skips wide-char spacers and replaces null chars with spaces.
fn cells_to_text(row: &Row, col_start: usize, col_end: usize) -> String {
    let mut text = String::new();
    let last = col_end.min(row.len().saturating_sub(1));
    for col in col_start..=last {
        let cell = &row[col];
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
            || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let c = if cell.c == '\0' { ' ' } else { cell.c };
        text.push(c);
        for &zw in cell.zerowidth() {
            text.push(zw);
        }
    }
    text
}
