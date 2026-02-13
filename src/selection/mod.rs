//! Selection — 3-point model (anchor, pivot, end) with char/word/line/block modes.

mod boundaries;
#[cfg(test)]
mod tests;
mod text;

pub use boundaries::{logical_line_end, logical_line_start, word_boundaries};
pub use text::extract_text;

/// Sub-cell precision for selection boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// A point in absolute grid coordinates (scrollback row 0 = oldest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: usize,
    pub col: usize,
    pub side: Side,
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
    pub fn new_char(row: usize, col: usize, side: Side) -> Self {
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

    /// Test whether a cell at (`abs_row`, `col`) is within the selection.
    pub fn contains(&self, abs_row: usize, col: usize) -> bool {
        let (start, end) = self.ordered();

        if self.mode == SelectionMode::Block {
            // Block selection: rectangular region
            let min_col = start.col.min(end.col);
            let max_col = start.col.max(end.col);
            abs_row >= start.row && abs_row <= end.row && col >= min_col && col <= max_col
        } else {
            // Normal selection: spans rows
            if abs_row < start.row || abs_row > end.row {
                return false;
            }
            if abs_row == start.row && abs_row == end.row {
                // Single row
                let start_col = if start.side == Side::Right {
                    start.col + 1
                } else {
                    start.col
                };
                let end_col = if end.side == Side::Left && end.col > 0 {
                    end.col - 1
                } else {
                    end.col
                };
                return col >= start_col && col <= end_col;
            }
            if abs_row == start.row {
                let start_col = if start.side == Side::Right {
                    start.col + 1
                } else {
                    start.col
                };
                return col >= start_col;
            }
            if abs_row == end.row {
                let end_col = if end.side == Side::Left && end.col > 0 {
                    end.col - 1
                } else {
                    end.col
                };
                return col <= end_col;
            }
            // Middle rows are fully selected
            true
        }
    }

    /// Returns true if this selection has zero area (anchor == end for Char mode).
    pub fn is_empty(&self) -> bool {
        self.mode == SelectionMode::Char && self.anchor == self.end
    }
}
