//! Terminal cursor state.
//!
//! Tracks the current write position and the "template cell" used for
//! newly written characters.

use crate::cell::Cell;
use crate::index::Column;

/// Cursor shape for rendering.
///
/// DECSCUSR sets cursor shape globally (not per-screen), so this is stored
/// on `Term`, not on `Cursor`. Kept in this module because it's a cursor
/// concept re-exported through `grid::CursorShape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
    HollowBlock,
}

/// Terminal cursor: position and template cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Line index into visible rows (0-based).
    line: usize,
    /// Column index (0-based).
    col: Column,
    /// Template cell applied to new characters (fg, bg, flags).
    ///
    /// Intentionally `pub` — the VTE handler sets SGR attributes directly
    /// on this cell, and Grid editing methods read it for character writes
    /// and BCE (Background Color Erase) operations.
    pub template: Cell,
}

impl Cursor {
    /// Create a cursor at (0, 0) with default template.
    pub fn new() -> Self {
        Self {
            line: 0,
            col: Column(0),
            template: Cell::default(),
        }
    }

    /// Current line (row index into visible area).
    pub fn line(&self) -> usize {
        self.line
    }

    /// Current column.
    pub fn col(&self) -> Column {
        self.col
    }

    /// Set the cursor line.
    pub fn set_line(&mut self, line: usize) {
        self.line = line;
    }

    /// Set the cursor column.
    pub fn set_col(&mut self, col: Column) {
        self.col = col;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
