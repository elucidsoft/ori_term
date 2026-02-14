//! Fixed-capacity ring buffer for viewport rows.
//!
//! Provides O(1) rotation for full-screen scroll operations instead of
//! O(lines) memmove from `Vec::remove(0)` + `Vec::insert(bottom)`.

use std::ops::{Index, IndexMut};

use super::row::Row;

/// Fixed-capacity ring buffer for viewport rows.
///
/// Logical row 0 maps to physical index `zero` via modular arithmetic.
/// `rotate_up()` advances `zero` without moving memory, making
/// full-screen scroll O(1) per line.
#[derive(Debug, Clone)]
pub struct ViewportRing {
    rows: Vec<Row>,
    /// Physical index of logical row 0.
    zero: usize,
    /// Number of rows (always == `rows.len()`).
    len: usize,
}

impl ViewportRing {
    /// Creates a new ring buffer with `lines` rows of `cols` columns each.
    pub fn new(cols: usize, lines: usize) -> Self {
        let rows = (0..lines).map(|_| Row::new(cols)).collect();
        Self {
            rows,
            zero: 0,
            len: lines,
        }
    }

    /// Creates a ring buffer from an existing `Vec` of rows.
    pub fn from_vec(rows: Vec<Row>) -> Self {
        let len = rows.len();
        Self { rows, zero: 0, len }
    }

    /// Number of rows in the viewport.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the viewport is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Logical index to physical index.
    fn physical(&self, logical: usize) -> usize {
        (self.zero + logical) % self.len
    }

    /// Rotate the ring up: logical row 0 becomes the bottom row.
    ///
    /// Returns a mutable reference to the old row 0 (now at logical bottom)
    /// so the caller can clone/take it for scrollback before clearing it.
    pub fn rotate_up(&mut self) -> &mut Row {
        let old_zero = self.zero;
        self.zero = (self.zero + 1) % self.len;
        &mut self.rows[old_zero]
    }

    /// Rotate the ring down: bottom row becomes logical row 0.
    ///
    /// Returns a mutable reference to the new row 0 (was at logical bottom).
    pub fn rotate_down(&mut self) -> &mut Row {
        self.zero = if self.zero == 0 {
            self.len - 1
        } else {
            self.zero - 1
        };
        let phys = self.zero;
        &mut self.rows[phys]
    }

    /// Access a row by logical index.
    pub fn row(&self, line: usize) -> &Row {
        &self.rows[self.physical(line)]
    }

    /// Mutably access a row by logical index.
    pub fn row_mut(&mut self, line: usize) -> &mut Row {
        let idx = self.physical(line);
        &mut self.rows[idx]
    }

    /// Remove a row at logical index and insert a fresh row at another.
    ///
    /// Used for scroll region operations where ring rotation doesn't help
    /// (when `top > 0`). Falls back to `O(region_size)` move.
    pub fn remove_insert(&mut self, remove_at: usize, insert_at: usize, cols: usize) {
        // Collect into logical order, do the remove/insert, then redistribute.
        // This is the slow path — only used for scroll regions.
        let mut logical: Vec<Row> = (0..self.len).map(|i| self.row(i).clone()).collect();
        logical.remove(remove_at);
        logical.insert(insert_at, Row::new(cols));
        for (i, r) in logical.into_iter().enumerate() {
            let phys = (self.zero + i) % self.len;
            self.rows[phys] = r;
        }
    }

    /// Resize the ring to a new number of lines. Returns removed rows
    /// (from the top, in logical order) when shrinking.
    pub fn resize(&mut self, new_lines: usize, cols: usize) -> Vec<Row> {
        if new_lines == self.len {
            return Vec::new();
        }
        // Linearize into logical order.
        let mut logical: Vec<Row> = (0..self.len).map(|i| self.row(i).clone()).collect();
        let removed = if new_lines < self.len {
            let excess = self.len - new_lines;
            logical.drain(..excess).collect()
        } else {
            let extra = new_lines - self.len;
            for _ in 0..extra {
                logical.push(Row::new(cols));
            }
            Vec::new()
        };
        self.rows = logical;
        self.zero = 0;
        self.len = new_lines;
        removed
    }

    /// Drain all rows in logical order (used by reflow).
    pub fn drain_logical(&mut self) -> Vec<Row> {
        let rows: Vec<Row> = (0..self.len).map(|i| self.row(i).clone()).collect();
        self.zero = 0;
        rows
    }

    /// Replace all rows from a vec (used by reflow). Resets zero offset.
    pub fn replace_from_vec(&mut self, mut rows: Vec<Row>, cols: usize) {
        while rows.len() < self.len {
            rows.push(Row::new(cols));
        }
        rows.truncate(self.len);
        self.rows = rows;
        self.zero = 0;
    }

    /// Resize all rows to a new column width.
    pub fn resize_cols(&mut self, cols: usize) {
        for row in &mut self.rows {
            row.resize(cols);
        }
    }

    /// Iterator over logical rows 0..len.
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        (0..self.len).map(move |i| &self.rows[self.physical(i)])
    }
}

impl Index<usize> for ViewportRing {
    type Output = Row;
    fn index(&self, idx: usize) -> &Row {
        self.row(idx)
    }
}

impl IndexMut<usize> for ViewportRing {
    fn index_mut(&mut self, idx: usize) -> &mut Row {
        self.row_mut(idx)
    }
}
