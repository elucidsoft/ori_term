//! Terminal grid row.
//!
//! A `Row` is a contiguous array of `Cell`s representing one terminal line,
//! with occupancy tracking for efficient sparse-row operations.

use std::ops::{Index, IndexMut, Range};

use crate::cell::Cell;
use crate::index::Column;

/// One row of cells in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The cells in this row.
    inner: Vec<Cell>,
    /// Upper bound on cells modified since last `reset()`.
    ///
    /// After `reset()`, occ is 0. `IndexMut` bumps occ to track writes.
    /// The value may exceed the true occupancy (lazy dirty-tracking, matching
    /// Alacritty's pattern). Use `clamp_occ` / `set_occ` for O(1) adjustments.
    occ: usize,
}

impl Row {
    /// Create a new row of `cols` default cells.
    pub fn new(cols: usize) -> Self {
        Self {
            inner: vec![Cell::default(); cols],
            occ: 0,
        }
    }

    /// Reset all cells to the template, resizing if needed.
    ///
    /// Only iterates `[0..occ]` when the template background matches existing
    /// empty cells (the common case). When template bg differs (BCE), marks
    /// the entire row dirty first so all cells get the new background.
    pub fn reset(&mut self, cols: usize, template: &Cell) {
        // If template bg differs from what empty cells currently contain,
        // the entire row needs updating (BCE background change).
        if !self.inner.is_empty()
            && self.inner[self.inner.len() - 1].bg != template.bg
        {
            self.occ = self.inner.len();
        }

        self.inner.resize_with(cols, || template.clone());
        for cell in &mut self.inner[..self.occ.min(cols)] {
            cell.reset(template);
        }
        self.occ = 0;
    }

    /// Number of columns in this row.
    pub fn cols(&self) -> usize {
        self.inner.len()
    }

    /// Occupancy upper bound (see `occ` field docs).
    pub(crate) fn occ(&self) -> usize {
        self.occ
    }

    /// Clear cells in the given column range, resetting them to the template.
    pub fn clear_range(&mut self, range: Range<Column>, template: &Cell) {
        let start = range.start.0;
        let end = range.end.0.min(self.inner.len());
        for cell in &mut self.inner[start..end] {
            cell.reset(template);
        }
        // Existing occ is still a valid upper bound (cells replaced in-place,
        // no rightward shift). Leave loose — matches Alacritty/Ghostty pattern.
    }

    /// Clear from the given column to the end of the row.
    pub fn truncate(&mut self, col: Column, template: &Cell) {
        let start = col.0;
        for cell in &mut self.inner[start..] {
            cell.reset(template);
        }
        self.occ = self.occ.min(start);
    }

    /// Mutable access to the inner cell slice.
    ///
    /// # Occ contract
    ///
    /// Callers **must** maintain the occ invariant after mutation:
    /// either call `clamp_occ` / `set_occ`, or verify that the
    /// existing occ is still a valid upper bound.
    pub(crate) fn as_mut_slice(&mut self) -> &mut [Cell] {
        &mut self.inner
    }

    /// Write a cell at the given column, updating occupancy.
    #[cfg(test)]
    pub(crate) fn append(&mut self, col: Column, cell: &Cell) {
        let idx = col.0;
        if idx < self.inner.len() {
            self.inner[idx] = cell.clone();
            if !cell.is_empty() && idx + 1 > self.occ {
                self.occ = idx + 1;
            }
        }
    }

    /// Clamp occ to at most `max`, maintaining it as a valid upper bound.
    pub(crate) fn clamp_occ(&mut self, max: usize) {
        self.occ = self.occ.min(max);
    }

    /// Set occ to an explicit upper bound (must be valid).
    pub(crate) fn set_occ(&mut self, occ: usize) {
        self.occ = occ;
    }

}

impl Index<Column> for Row {
    type Output = Cell;

    fn index(&self, col: Column) -> &Cell {
        &self.inner[col.0]
    }
}

impl IndexMut<Column> for Row {
    fn index_mut(&mut self, col: Column) -> &mut Cell {
        let idx = col.0;
        if idx + 1 > self.occ {
            self.occ = idx + 1;
        }
        &mut self.inner[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::Row;
    use crate::cell::{Cell, CellFlags};
    use crate::index::Column;

    #[test]
    fn new_row_has_correct_length_and_defaults() {
        let row = Row::new(80);
        assert_eq!(row.cols(), 80);
        assert_eq!(row.occ(), 0);
        assert!(row[Column(0)].is_empty());
        assert!(row[Column(79)].is_empty());
    }

    #[test]
    fn writing_cell_updates_occ() {
        let mut row = Row::new(80);
        let mut cell = Cell::default();
        cell.ch = 'A';
        row.append(Column(5), &cell);
        assert_eq!(row.occ(), 6);
        assert_eq!(row[Column(5)].ch, 'A');
    }

    #[test]
    fn reset_clears_and_resets_occ() {
        let mut row = Row::new(80);
        let mut cell = Cell::default();
        cell.ch = 'X';
        row.append(Column(10), &cell);
        assert_eq!(row.occ(), 11);

        row.reset(80, &Cell::default());
        assert_eq!(row.occ(), 0);
        assert!(row[Column(10)].is_empty());
    }

    #[test]
    fn index_returns_correct_cell() {
        let mut row = Row::new(80);
        let mut cell = Cell::default();
        cell.ch = 'B';
        cell.flags = CellFlags::BOLD;
        row.append(Column(3), &cell);

        assert_eq!(row[Column(3)].ch, 'B');
        assert!(row[Column(3)].flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn index_mut_updates_occ() {
        let mut row = Row::new(80);
        row[Column(20)].ch = 'Z';
        // IndexMut bumps occ as an upper bound — it does not check emptiness.
        assert_eq!(row.occ(), 21);
    }

    #[test]
    fn clear_range_resets_columns() {
        let mut row = Row::new(80);
        let mut cell = Cell::default();
        cell.ch = 'X';
        for i in 0..10 {
            row.append(Column(i), &cell);
        }
        assert_eq!(row.occ(), 10);

        row.clear_range(Column(3)..Column(7), &Cell::default());
        assert!(row[Column(3)].is_empty());
        assert!(row[Column(6)].is_empty());
        assert_eq!(row[Column(2)].ch, 'X');
        assert_eq!(row[Column(7)].ch, 'X');
    }

    #[test]
    fn truncate_clears_from_column_to_end() {
        let mut row = Row::new(80);
        let mut cell = Cell::default();
        cell.ch = 'A';
        for i in 0..20 {
            row.append(Column(i), &cell);
        }
        assert_eq!(row.occ(), 20);

        row.truncate(Column(10), &Cell::default());
        assert_eq!(row.occ(), 10);
        assert_eq!(row[Column(9)].ch, 'A');
        assert!(row[Column(10)].is_empty());
    }

    #[test]
    fn reset_bce_across_consecutive_resets() {
        use vte::ansi::Color;

        let color1 = Color::Indexed(1);
        let color2 = Color::Indexed(2);
        let tmpl1 = Cell::from(color1);
        let tmpl2 = Cell::from(color2);

        let mut row = Row::new(10);

        // First reset: bg=color1 → all cells get color1, occ drops to 0.
        row.reset(10, &tmpl1);
        assert_eq!(row.occ(), 0);
        assert_eq!(row[Column(0)].bg, color1);
        assert_eq!(row[Column(9)].bg, color1);

        // Second reset with different bg: even though occ is 0, the BCE
        // guard must detect the bg mismatch and repaint all cells.
        row.reset(10, &tmpl2);
        assert_eq!(row.occ(), 0);
        assert_eq!(row[Column(0)].bg, color2);
        assert_eq!(row[Column(9)].bg, color2);
    }

    // --- Additional tests from reference repo gap analysis ---

    #[test]
    fn reset_resizes_row_larger() {
        let mut row = Row::new(10);
        assert_eq!(row.cols(), 10);
        row.reset(20, &Cell::default());
        assert_eq!(row.cols(), 20);
        assert_eq!(row.occ(), 0);
    }

    #[test]
    fn reset_shrinks_row() {
        let mut row = Row::new(20);
        let mut cell = Cell::default();
        cell.ch = 'A';
        row.append(Column(15), &cell);
        row.reset(10, &Cell::default());
        assert_eq!(row.cols(), 10);
        assert_eq!(row.occ(), 0);
    }

    #[test]
    fn clear_range_full_row() {
        let mut row = Row::new(10);
        let mut cell = Cell::default();
        cell.ch = 'X';
        for i in 0..10 {
            row.append(Column(i), &cell);
        }
        row.clear_range(Column(0)..Column(10), &Cell::default());
        for i in 0..10 {
            assert!(row[Column(i)].is_empty(), "Column {i} not empty");
        }
    }

    #[test]
    fn clear_range_with_bce() {
        use vte::ansi::Color;
        let mut row = Row::new(10);
        let mut cell = Cell::default();
        cell.ch = 'X';
        for i in 0..10 {
            row.append(Column(i), &cell);
        }
        let template = Cell::from(Color::Indexed(1));
        row.clear_range(Column(3)..Column(7), &template);
        assert_eq!(row[Column(3)].bg, Color::Indexed(1));
        assert_eq!(row[Column(6)].bg, Color::Indexed(1));
        assert_eq!(row[Column(3)].ch, ' ');
        // Cells outside range untouched.
        assert_eq!(row[Column(2)].ch, 'X');
        assert_eq!(row[Column(7)].ch, 'X');
    }

    #[test]
    fn truncate_at_col_zero_clears_entire_row() {
        let mut row = Row::new(10);
        let mut cell = Cell::default();
        cell.ch = 'X';
        for i in 0..10 {
            row.append(Column(i), &cell);
        }
        row.truncate(Column(0), &Cell::default());
        assert_eq!(row.occ(), 0);
        for i in 0..10 {
            assert!(row[Column(i)].is_empty());
        }
    }

    #[test]
    fn append_empty_cell_does_not_bump_occ() {
        let mut row = Row::new(10);
        row.append(Column(5), &Cell::default());
        assert_eq!(row.occ(), 0);
    }

    #[test]
    fn row_equality() {
        let row1 = Row::new(10);
        let row2 = Row::new(10);
        assert_eq!(row1, row2);

        let mut row3 = Row::new(10);
        let mut cell = Cell::default();
        cell.ch = 'A';
        row3.append(Column(0), &cell);
        assert_ne!(row1, row3);
    }
}
