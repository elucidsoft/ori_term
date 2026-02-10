use crate::cell::CellFlags;
use crate::grid::Grid;

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

/// Find word boundaries around (`abs_row`, `col`) in the grid.
/// Returns (`start_col`, `end_col`) inclusive.
pub fn word_boundaries(grid: &Grid, abs_row: usize, col: usize) -> (usize, usize) {
    let row = match grid.absolute_row(abs_row) {
        Some(r) => r,
        None => return (col, col),
    };

    let cols = row.len();
    if cols == 0 || col >= cols {
        return (col, col);
    }

    let ch = row[col].c;
    let class = char_class(ch);

    // Scan left
    let mut start = col;
    while start > 0 && char_class(row[start - 1].c) == class {
        start -= 1;
    }

    // Scan right
    let mut end = col;
    while end + 1 < cols && char_class(row[end + 1].c) == class {
        end += 1;
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
    let mut r = abs_row;
    while r > 0 {
        if let Some(prev_row) = grid.absolute_row(r - 1) {
            let cols = prev_row.len();
            if cols > 0 && prev_row[cols - 1].flags.contains(CellFlags::WRAPLINE) {
                r -= 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    r
}

/// Walk forwards to find the end of a logical (soft-wrapped) line.
pub fn logical_line_end(grid: &Grid, abs_row: usize) -> usize {
    let total = grid.scrollback.len() + grid.lines;
    let mut r = abs_row;
    while let Some(row) = grid.absolute_row(r) {
        let cols = row.len();
        if cols > 0 && row[cols - 1].flags.contains(CellFlags::WRAPLINE) && r + 1 < total {
            r += 1;
        } else {
            break;
        }
    }
    r
}

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
                let is_wrapped = !row.is_empty()
                    && row[row.len() - 1].flags.contains(CellFlags::WRAPLINE);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_point_ordering() {
        let a = SelectionPoint { row: 0, col: 5, side: Side::Left };
        let b = SelectionPoint { row: 0, col: 5, side: Side::Right };
        let c = SelectionPoint { row: 1, col: 0, side: Side::Left };
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn selection_contains_single_row() {
        let sel = Selection {
            mode: SelectionMode::Char,
            anchor: SelectionPoint { row: 5, col: 2, side: Side::Left },
            pivot: SelectionPoint { row: 5, col: 2, side: Side::Left },
            end: SelectionPoint { row: 5, col: 8, side: Side::Right },
        };
        assert!(!sel.contains(5, 1));
        assert!(sel.contains(5, 2));
        assert!(sel.contains(5, 5));
        assert!(sel.contains(5, 8));
        assert!(!sel.contains(5, 9));
        assert!(!sel.contains(4, 5));
        assert!(!sel.contains(6, 5));
    }

    #[test]
    fn selection_contains_multi_row() {
        let sel = Selection {
            mode: SelectionMode::Char,
            anchor: SelectionPoint { row: 2, col: 5, side: Side::Left },
            pivot: SelectionPoint { row: 2, col: 5, side: Side::Left },
            end: SelectionPoint { row: 4, col: 3, side: Side::Right },
        };
        // Row 2: col >= 5
        assert!(!sel.contains(2, 4));
        assert!(sel.contains(2, 5));
        assert!(sel.contains(2, 100));
        // Row 3: fully selected
        assert!(sel.contains(3, 0));
        assert!(sel.contains(3, 100));
        // Row 4: col <= 3
        assert!(sel.contains(4, 0));
        assert!(sel.contains(4, 3));
        assert!(!sel.contains(4, 4));
    }

    #[test]
    fn selection_empty() {
        let sel = Selection::new_char(5, 10, Side::Left);
        assert!(sel.is_empty());

        let mut sel2 = Selection::new_char(5, 10, Side::Left);
        sel2.end.col = 12;
        assert!(!sel2.is_empty());
    }

    #[test]
    fn block_selection_contains() {
        let sel = Selection {
            mode: SelectionMode::Block,
            anchor: SelectionPoint { row: 2, col: 3, side: Side::Left },
            pivot: SelectionPoint { row: 2, col: 3, side: Side::Left },
            end: SelectionPoint { row: 5, col: 7, side: Side::Right },
        };
        assert!(sel.contains(3, 5));
        assert!(!sel.contains(3, 2));
        assert!(!sel.contains(3, 8));
        assert!(!sel.contains(1, 5));
        assert!(!sel.contains(6, 5));
    }

    #[test]
    fn word_boundaries_on_grid() {
        let mut grid = Grid::new(20, 1);
        // Write "hello world" into row 0
        for (i, c) in "hello world".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }
        // Test word boundary for 'e' (col 1)
        let (s, e) = word_boundaries(&grid, 0, 1);
        assert_eq!(s, 0);
        assert_eq!(e, 4);
        // Test word boundary for 'w' (col 6)
        let (s, e) = word_boundaries(&grid, 0, 6);
        assert_eq!(s, 6);
        assert_eq!(e, 10);
        // Test word boundary for space (col 5)
        let (s, e) = word_boundaries(&grid, 0, 5);
        assert_eq!(s, 5);
        assert_eq!(e, 5);
    }

    #[test]
    fn extract_text_simple() {
        let mut grid = Grid::new(10, 2);
        for (i, c) in "Hello".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }
        for (i, c) in "World".chars().enumerate() {
            grid.goto(1, i);
            grid.put_char(c);
        }

        let sel = Selection {
            mode: SelectionMode::Char,
            anchor: SelectionPoint { row: 0, col: 0, side: Side::Left },
            pivot: SelectionPoint { row: 0, col: 0, side: Side::Left },
            end: SelectionPoint { row: 1, col: 4, side: Side::Right },
        };
        let text = extract_text(&grid, &sel);
        assert_eq!(text, "Hello\nWorld");
    }
}
