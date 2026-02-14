//! Terminal grid: 2D cell storage with cursor, scrollback, and dirty tracking.
//!
//! The `Grid` is the central data structure for terminal emulation. It stores
//! visible rows, manages cursor state, and tracks tab stops. Scrollback,
//! dirty tracking, and editing operations are added in submodules.

pub mod cursor;
pub mod editing;
pub mod navigation;
pub mod row;

use std::ops::{Index, IndexMut, Range};

use crate::index::Line;

pub use cursor::{Cursor, CursorShape};
pub use editing::EraseMode;
pub use navigation::TabClearMode;
pub use row::Row;

/// The 2D terminal cell grid.
///
/// Stores visible rows indexed `0..lines` (top to bottom), a cursor,
/// and tab stops. Scrollback and dirty tracking are added in later
/// subsections.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Visible rows (index 0 = top of screen).
    rows: Vec<Row>,
    /// Number of columns.
    cols: usize,
    /// Number of visible lines.
    lines: usize,
    /// Current cursor position and template.
    cursor: Cursor,
    /// DECSC/DECRC saved cursor.
    saved_cursor: Option<Cursor>,
    /// Tab stop at each column (true = stop).
    tab_stops: Vec<bool>,
    /// DECSTBM scroll region: top (inclusive) .. bottom (exclusive).
    scroll_region: Range<usize>,
}

impl Grid {
    /// Create a new grid with the given dimensions.
    ///
    /// Initializes all rows as empty, cursor at (0, 0), and tab stops
    /// every 8 columns.
    pub fn new(lines: usize, cols: usize) -> Self {
        let rows = (0..lines).map(|_| Row::new(cols)).collect();
        let tab_stops = Self::init_tab_stops(cols);

        Self {
            rows,
            cols,
            lines,
            cursor: Cursor::new(),
            saved_cursor: None,
            tab_stops,
            scroll_region: 0..lines,
        }
    }

    /// Number of visible lines.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Immutable reference to the cursor.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Mutable reference to the cursor.
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Immutable reference to the saved cursor.
    pub fn saved_cursor(&self) -> Option<&Cursor> {
        self.saved_cursor.as_ref()
    }

    /// Immutable reference to tab stops.
    pub fn tab_stops(&self) -> &[bool] {
        &self.tab_stops
    }

    /// Immutable reference to the scroll region.
    pub fn scroll_region(&self) -> &Range<usize> {
        &self.scroll_region
    }

    /// Initialize tab stops every 8 columns.
    fn init_tab_stops(cols: usize) -> Vec<bool> {
        (0..cols).map(|c| c % 8 == 0).collect()
    }
}

impl Index<Line> for Grid {
    type Output = Row;

    fn index(&self, line: Line) -> &Row {
        &self.rows[line.0 as usize]
    }
}

impl IndexMut<Line> for Grid {
    fn index_mut(&mut self, line: Line) -> &mut Row {
        &mut self.rows[line.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::Grid;
    use crate::index::{Column, Line};

    #[test]
    fn new_grid_has_correct_dimensions() {
        let grid = Grid::new(24, 80);
        assert_eq!(grid.lines(), 24);
        assert_eq!(grid.cols(), 80);
    }

    #[test]
    fn tab_stops_every_8_columns() {
        let grid = Grid::new(24, 80);
        let stops = grid.tab_stops();
        assert!(stops[0]);  // Column 0.
        assert!(!stops[1]);
        assert!(stops[8]);
        assert!(stops[16]);
        assert!(!stops[79]);
        assert!(stops[72]);
    }

    #[test]
    fn index_by_line_returns_correct_row() {
        let grid = Grid::new(24, 80);
        let row = &grid[Line(0)];
        assert_eq!(row.cols(), 80);
        let row_last = &grid[Line(23)];
        assert_eq!(row_last.cols(), 80);
    }

    #[test]
    fn cursor_starts_at_origin() {
        let grid = Grid::new(24, 80);
        assert_eq!(grid.cursor().line(), 0);
        assert_eq!(grid.cursor().col(), Column(0));
    }
}
