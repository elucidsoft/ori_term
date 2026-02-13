//! A single row in the terminal grid, storing cells and metadata.

use std::ops::{Index, IndexMut};

use crate::cell::{Cell, CellFlags};

#[derive(Debug, Clone)]
pub struct Row {
    inner: Vec<Cell>,
    pub occ: usize,
    /// True if this row is the start of a shell prompt (OSC 133;A).
    pub prompt_start: bool,
}

impl Row {
    /// Creates a new row with the given number of columns.
    pub fn new(cols: usize) -> Self {
        Self {
            inner: vec![Cell::default(); cols],
            occ: 0,
            prompt_start: false,
        }
    }

    /// Returns the number of columns in this row.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if this row has zero columns.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns an iterator over the cells in this row.
    pub fn iter(&self) -> std::slice::Iter<'_, Cell> {
        self.inner.iter()
    }

    /// Returns a mutable iterator over the cells in this row.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Cell> {
        self.inner.iter_mut()
    }

    /// Direct access to the inner cell vector.
    pub fn cells(&self) -> &[Cell] {
        &self.inner
    }

    /// Direct mutable access to the inner cell vector.
    pub fn cells_mut(&mut self) -> &mut Vec<Cell> {
        &mut self.inner
    }

    /// Resets all cells in this row to the template cell, clearing content.
    pub fn reset(&mut self, template: &Cell) {
        for cell in &mut self.inner {
            cell.reset(template);
        }
        self.occ = 0;
    }

    /// Truncates the row to the specified number of columns.
    pub fn truncate(&mut self, cols: usize) {
        self.inner.truncate(cols);
        self.occ = self.occ.min(cols);
    }

    /// Grows the row to the specified number of columns, filling with default cells.
    pub fn grow(&mut self, cols: usize) {
        self.inner.resize(cols, Cell::default());
    }

    /// Resizes the row to the specified number of columns.
    pub fn resize(&mut self, cols: usize) {
        if cols < self.inner.len() {
            self.truncate(cols);
        } else {
            self.grow(cols);
        }
    }

    /// Returns the rightmost non-blank column index + 1 (content length).
    /// A blank cell is one with char ' ' or '\0' and no flags of interest.
    pub fn content_len(&self) -> usize {
        for i in (0..self.inner.len()).rev() {
            let c = &self.inner[i];
            if c.c != ' ' && c.c != '\0' {
                return i + 1;
            }
            if c.flags.intersects(
                CellFlags::WIDE_CHAR
                    | CellFlags::WIDE_CHAR_SPACER
                    | CellFlags::LEADING_WIDE_CHAR_SPACER,
            ) {
                return i + 1;
            }
        }
        0
    }

    /// Splits off cells from `at` onward, returning them. The row is truncated.
    pub fn split_off(&mut self, at: usize) -> Vec<Cell> {
        if at >= self.inner.len() {
            return Vec::new();
        }
        let split = self.inner.split_off(at);
        self.occ = self.occ.min(at);
        split
    }

    /// Appends cells to the end of the row.
    pub fn append(&mut self, cells: &[Cell]) {
        let start = self.inner.len();
        self.inner.extend_from_slice(cells);
        for (i, c) in cells.iter().enumerate() {
            if c.c != ' ' && c.c != '\0' {
                self.occ = self.occ.max(start + i + 1);
            }
        }
    }
}

impl<'a> IntoIterator for &'a Row {
    type Item = &'a Cell;
    type IntoIter = std::slice::Iter<'a, Cell>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Row {
    type Item = &'a mut Cell;
    type IntoIter = std::slice::IterMut<'a, Cell>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl Index<usize> for Row {
    type Output = Cell;

    fn index(&self, idx: usize) -> &Cell {
        &self.inner[idx]
    }
}

impl IndexMut<usize> for Row {
    fn index_mut(&mut self, idx: usize) -> &mut Cell {
        &mut self.inner[idx]
    }
}
