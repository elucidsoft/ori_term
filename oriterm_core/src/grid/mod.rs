//! Terminal grid: 2D cell storage with cursor, scrollback, and dirty tracking.
//!
//! The `Grid` is the central data structure for terminal emulation. It stores
//! visible rows, manages cursor state, and tracks tab stops. Scrollback,
//! dirty tracking, and editing operations are added in submodules.

pub mod cursor;
pub mod editing;
pub mod navigation;
pub mod row;
pub mod scroll;

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
mod tests;
