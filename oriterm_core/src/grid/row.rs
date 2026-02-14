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
    /// Index of last non-empty cell + 1 (0 = row is entirely empty).
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
    pub fn reset(&mut self, cols: usize, template: &Cell) {
        self.inner.resize(cols, Cell::default());
        for cell in &mut self.inner {
            cell.reset(template);
        }
        self.occ = 0;
    }

    /// Number of columns in this row.
    pub fn cols(&self) -> usize {
        self.inner.len()
    }

    /// Occupancy: index of last non-empty cell + 1.
    pub fn occ(&self) -> usize {
        self.occ
    }

    /// Clear cells in the given column range, resetting them to the template.
    pub fn clear_range(&mut self, range: Range<Column>, template: &Cell) {
        let start = range.start.0;
        let end = range.end.0.min(self.inner.len());
        for cell in &mut self.inner[start..end] {
            cell.reset(template);
        }
        self.recalculate_occ();
    }

    /// Clear from the given column to the end of the row.
    pub fn truncate(&mut self, col: Column) {
        let start = col.0;
        let template = Cell::default();
        for cell in &mut self.inner[start..] {
            cell.reset(&template);
        }
        self.occ = self.occ.min(start);
    }

    /// Write a cell at the given column, updating occupancy.
    pub fn append(&mut self, col: Column, cell: &Cell) {
        let idx = col.0;
        if idx < self.inner.len() {
            self.inner[idx] = cell.clone();
            if !cell.is_empty() && idx + 1 > self.occ {
                self.occ = idx + 1;
            }
        }
    }

    /// Recalculate occupancy by scanning from the end.
    fn recalculate_occ(&mut self) {
        self.occ = self
            .inner
            .iter()
            .rposition(|c| !c.is_empty())
            .map_or(0, |i| i + 1);
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
        // IndexMut should update occ even though is_empty isn't checked here.
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

        row.truncate(Column(10));
        assert_eq!(row.occ(), 10);
        assert_eq!(row[Column(9)].ch, 'A');
        assert!(row[Column(10)].is_empty());
    }
}
