//! Terminal cursor state.
//!
//! Tracks the current write position and the "template cell" used for
//! newly written characters. Also tracks cursor shape for rendering.

use crate::cell::Cell;
use crate::index::Column;

/// Cursor shape for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
    HollowBlock,
}

/// Terminal cursor: position, template cell, and shape.
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
    /// Visual cursor shape.
    ///
    /// Intentionally `pub` — set by DECSCUSR (CSI Ps SP q) in the VTE
    /// handler, read by the renderer to choose the cursor glyph.
    pub shape: CursorShape,
}

impl Cursor {
    /// Create a cursor at (0, 0) with default template and block shape.
    pub fn new() -> Self {
        Self {
            line: 0,
            col: Column(0),
            template: Cell::default(),
            shape: CursorShape::Block,
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
mod tests {
    use super::{Cursor, CursorShape};
    use crate::index::Column;

    #[test]
    fn default_cursor_at_origin_with_block_shape() {
        let cursor = Cursor::new();
        assert_eq!(cursor.line(), 0);
        assert_eq!(cursor.col(), Column(0));
        assert_eq!(cursor.shape, CursorShape::Block);
    }

    #[test]
    fn set_line_and_col() {
        let mut cursor = Cursor::new();
        cursor.set_line(5);
        cursor.set_col(Column(10));
        assert_eq!(cursor.line(), 5);
        assert_eq!(cursor.col(), Column(10));
    }

    #[test]
    fn default_shape_is_block() {
        assert_eq!(CursorShape::default(), CursorShape::Block);
    }
}
