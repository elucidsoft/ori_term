//! Word and line boundary detection for selection.

use crate::cell::CellFlags;
use crate::grid::{Grid, WrapDetection};

/// Find word boundaries around (`abs_row`, `col`) in the grid.
/// Returns (`start_col`, `end_col`) inclusive.
///
/// Wide char spacers are redirected to their base cell and skipped
/// during scanning so that double-clicking a CJK character selects
/// the full character including its spacer column.
pub fn word_boundaries(grid: &Grid, abs_row: usize, col: usize) -> (usize, usize) {
    let row = match grid.absolute_row(abs_row) {
        Some(r) => r,
        None => return (col, col),
    };

    let cols = row.len();
    if cols == 0 || col >= cols {
        return (col, col);
    }

    // If clicked on a wide char spacer, redirect to the base cell
    let click_col = if row[col].flags.contains(CellFlags::WIDE_CHAR_SPACER) && col > 0 {
        col - 1
    } else {
        col
    };

    let ch = row[click_col].c;
    let class = char_class(ch);

    // Scan left, skipping wide char spacers
    let mut start = click_col;
    while start > 0 {
        let prev = start - 1;
        if row[prev].flags.contains(CellFlags::WIDE_CHAR_SPACER) && prev > 0 {
            // Spacer: check the base cell before it
            if char_class(row[prev - 1].c) == class {
                start = prev - 1;
            } else {
                break;
            }
        } else if char_class(row[prev].c) == class {
            start = prev;
        } else {
            break;
        }
    }

    // Scan right, skipping wide char spacers
    let mut end = click_col;
    while end + 1 < cols {
        let next = end + 1;
        if row[next].flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            // Spacer belongs to the wide char at `end` — include it
            end = next;
            continue;
        }
        if char_class(row[next].c) == class {
            end = next;
        } else {
            break;
        }
    }

    (start, end)
}

/// Classify characters for word boundary detection.
fn char_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0 // Word characters
    } else if c == ' ' || c == '\0' {
        1 // Whitespace
    } else {
        2 // Punctuation / other
    }
}

/// Walk backwards to find the start of a logical (soft-wrapped) line.
pub fn logical_line_start(grid: &Grid, abs_row: usize) -> usize {
    grid.logical_line_start(abs_row, WrapDetection::WrapFlag)
}

/// Walk forwards to find the end of a logical (soft-wrapped) line.
pub fn logical_line_end(grid: &Grid, abs_row: usize) -> usize {
    grid.logical_line_end(abs_row, WrapDetection::WrapFlag)
}
