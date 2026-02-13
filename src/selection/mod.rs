//! Selection — 3-point model (anchor, pivot, end) with char/word/line/block modes.

mod boundaries;
#[cfg(test)]
mod tests;
mod text;

pub use boundaries::{logical_line_end, logical_line_start, word_boundaries};
pub use text::extract_text;

use crate::grid::StableRowIndex;

/// Sub-cell precision for selection boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// A point in stable grid coordinates.
///
/// Uses `StableRowIndex` so row identity survives scrollback eviction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: StableRowIndex,
    pub col: usize,
    pub side: Side,
}

impl SelectionPoint {
    /// The effective first column included in the selection at this point.
    /// When `side` is `Right`, the click landed on the right half of the cell,
    /// so the selection starts at the next column.
    pub fn effective_start_col(&self) -> usize {
        if self.side == Side::Right {
            self.col + 1
        } else {
            self.col
        }
    }

    /// The effective last column included in the selection at this point.
    /// When `side` is `Left`, the click landed on the left half of the cell,
    /// so the selection ends at the previous column.
    pub fn effective_end_col(&self) -> usize {
        if self.side == Side::Left && self.col > 0 {
            self.col - 1
        } else {
            self.col
        }
    }
}

impl Ord for SelectionPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.row
            .cmp(&other.row)
            .then(self.col.cmp(&other.col))
            .then(match (&self.side, &other.side) {
                (Side::Left, Side::Right) => std::cmp::Ordering::Less,
                (Side::Right, Side::Left) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    }
}

impl PartialOrd for SelectionPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Selection mode: character, word, line, or block (rectangular).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Char,
    Word,
    Line,
    Block,
}

/// A selection in the terminal grid.
///
/// Uses a 3-point model (anchor, pivot, end):
/// - `anchor`: where the click started
/// - `pivot`: other end of the initial unit (same as anchor for Char,
///   word boundary for Word, line boundary for Line)
/// - `end`: current drag position
pub struct Selection {
    pub mode: SelectionMode,
    pub anchor: SelectionPoint,
    pub pivot: SelectionPoint,
    pub end: SelectionPoint,
}

impl Selection {
    /// Create a new character-mode selection at a single point.
    pub fn new_char(row: StableRowIndex, col: usize, side: Side) -> Self {
        let point = SelectionPoint { row, col, side };
        Self {
            mode: SelectionMode::Char,
            anchor: point,
            pivot: point,
            end: point,
        }
    }

    /// Create a new word-mode selection.
    pub fn new_word(anchor: SelectionPoint, pivot: SelectionPoint) -> Self {
        Self {
            mode: SelectionMode::Word,
            anchor,
            pivot,
            end: anchor,
        }
    }

    /// Create a new line-mode selection.
    pub fn new_line(anchor: SelectionPoint, pivot: SelectionPoint) -> Self {
        Self {
            mode: SelectionMode::Line,
            anchor,
            pivot,
            end: anchor,
        }
    }

    /// Returns the normalized (start, end) range including the pivot.
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        let mut points = [self.anchor, self.pivot, self.end];
        points.sort();
        (points[0], points[2])
    }

    /// Test whether a cell at (`stable_row`, `col`) is within the selection.
    pub fn contains(&self, stable_row: StableRowIndex, col: usize) -> bool {
        let (start, end) = self.ordered();

        if self.mode == SelectionMode::Block {
            let min_col = start.col.min(end.col);
            let max_col = start.col.max(end.col);
            stable_row >= start.row
                && stable_row <= end.row
                && col >= min_col
                && col <= max_col
        } else {
            if stable_row < start.row || stable_row > end.row {
                return false;
            }
            let first = if stable_row == start.row {
                start.effective_start_col()
            } else {
                0
            };
            let last = if stable_row == end.row {
                end.effective_end_col()
            } else {
                usize::MAX
            };
            col >= first && col <= last
        }
    }

    /// Returns true if this selection has zero area (anchor == end for Char mode).
    pub fn is_empty(&self) -> bool {
        self.mode == SelectionMode::Char && self.anchor == self.end
    }
}
