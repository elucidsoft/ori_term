//! Text extraction from grid selection.

use crate::cell::CellFlags;
use crate::grid::Grid;

use super::{Selection, SelectionMode, Side};

/// Extract selected text from the grid.
pub fn extract_text(grid: &Grid, selection: &Selection) -> String {
    let (start, end) = selection.ordered();
    let mut result = String::new();

    if selection.mode == SelectionMode::Block {
        // Block selection: rectangular region
        let min_col = start.col.min(end.col);
        let max_col = start.col.max(end.col);

        for abs_row in start.row..=end.row {
            if let Some(row) = grid.absolute_row(abs_row) {
                let mut line = String::new();
                for col in min_col..=max_col.min(row.len().saturating_sub(1)) {
                    let cell = &row[col];
                    if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
                        || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
                    {
                        continue;
                    }
                    let c = if cell.c == '\0' { ' ' } else { cell.c };
                    line.push(c);
                    // Append zero-width chars
                    for &zw in cell.zerowidth() {
                        line.push(zw);
                    }
                }
                // Trim trailing spaces
                let trimmed = line.trim_end();
                result.push_str(trimmed);
            }
            if abs_row < end.row {
                result.push('\n');
            }
        }
    } else {
        // Normal selection
        for abs_row in start.row..=end.row {
            if let Some(row) = grid.absolute_row(abs_row) {
                let row_start = if abs_row == start.row {
                    if start.side == Side::Right {
                        start.col + 1
                    } else {
                        start.col
                    }
                } else {
                    0
                };
                let row_end = if abs_row == end.row {
                    if end.side == Side::Left && end.col > 0 {
                        end.col - 1
                    } else {
                        end.col
                    }
                } else {
                    row.len().saturating_sub(1)
                };

                let mut line = String::new();
                for col in row_start..=row_end.min(row.len().saturating_sub(1)) {
                    let cell = &row[col];
                    if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
                        || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
                    {
                        continue;
                    }
                    let c = if cell.c == '\0' { ' ' } else { cell.c };
                    line.push(c);
                    for &zw in cell.zerowidth() {
                        line.push(zw);
                    }
                }

                // Check if this row is soft-wrapped (continues on next row)
                let is_wrapped =
                    !row.is_empty() && row[row.len() - 1].flags.contains(CellFlags::WRAPLINE);

                if is_wrapped && abs_row < end.row {
                    // Soft wrap: don't trim, don't add newline
                    result.push_str(&line);
                } else {
                    // Hard break or end of selection: trim trailing spaces
                    let trimmed = line.trim_end();
                    result.push_str(trimmed);
                    if abs_row < end.row {
                        result.push('\n');
                    }
                }
            }
        }
    }

    result
}
