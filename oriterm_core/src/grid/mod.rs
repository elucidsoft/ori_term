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
        debug_assert!(lines >= 1 && cols >= 1, "Grid dimensions must be >= 1 (got {lines}x{cols})");
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

    /// Immutable reference to tab stops.
    #[cfg(test)]
    pub(crate) fn tab_stops(&self) -> &[bool] {
        &self.tab_stops
    }

    /// Initialize tab stops every 8 columns.
    fn init_tab_stops(cols: usize) -> Vec<bool> {
        (0..cols).map(|c| c % 8 == 0).collect()
    }
}

impl Index<Line> for Grid {
    type Output = Row;

    fn index(&self, line: Line) -> &Row {
        &self.rows[usize::try_from(line.0)
            .expect("negative Line index used on Grid without scrollback")]
    }
}

impl IndexMut<Line> for Grid {
    fn index_mut(&mut self, line: Line) -> &mut Row {
        &mut self.rows[usize::try_from(line.0)
            .expect("negative Line index used on Grid without scrollback")]
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

    // --- Additional tests from reference repo gap analysis ---

    #[test]
    fn grid_1x1_minimum_dimensions() {
        let grid = Grid::new(1, 1);
        assert_eq!(grid.lines(), 1);
        assert_eq!(grid.cols(), 1);
        assert!(grid[Line(0)][Column(0)].is_empty());
    }

    #[test]
    fn scroll_region_defaults_to_full_grid() {
        let grid = Grid::new(24, 80);
        assert_eq!(grid.scroll_region, 0..24);
    }

    #[test]
    fn saved_cursor_starts_as_none() {
        let grid = Grid::new(24, 80);
        assert!(grid.saved_cursor.is_none());
    }

    #[test]
    fn tab_stops_for_narrow_grid() {
        // Grid narrower than 8 columns: only col 0 is a stop.
        let grid = Grid::new(1, 5);
        let stops = grid.tab_stops();
        assert!(stops[0]);
        assert!(!stops[1]);
        assert!(!stops[4]);
    }

    #[test]
    fn all_rows_initialized_empty() {
        let grid = Grid::new(5, 10);
        for line in 0..5 {
            let row = &grid[Line(line as i32)];
            assert_eq!(row.cols(), 10);
            for col in 0..10 {
                assert!(row[Column(col)].is_empty());
            }
        }
    }
}
