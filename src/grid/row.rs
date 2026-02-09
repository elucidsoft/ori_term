use std::ops::{Index, IndexMut};

use crate::cell::Cell;

#[derive(Debug, Clone)]
pub struct Row {
    inner: Vec<Cell>,
    pub occ: usize,
}

impl Row {
    pub fn new(cols: usize) -> Self {
        Self {
            inner: vec![Cell::default(); cols],
            occ: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn reset(&mut self, template: &Cell) {
        for cell in &mut self.inner {
            cell.reset(template);
        }
        self.occ = 0;
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Cell> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Cell> {
        self.inner.iter_mut()
    }

    pub fn truncate(&mut self, cols: usize) {
        self.inner.truncate(cols);
        self.occ = self.occ.min(cols);
    }

    pub fn grow(&mut self, cols: usize) {
        self.inner.resize(cols, Cell::default());
    }

    pub fn resize(&mut self, cols: usize) {
        if cols < self.inner.len() {
            self.truncate(cols);
        } else {
            self.grow(cols);
        }
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
