//! Terminal cursor state and attribute template.

use vte::ansi::Color;

use crate::cell::{Cell, CellFlags};

/// Terminal cursor position and attribute template for newly written cells.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// Column position (0-based)
    pub col: usize,
    /// Row position (0-based)
    pub row: usize,
    /// Attribute template for new cells
    pub template: Cell,
    /// Wraparound pending flag
    pub input_needs_wrap: bool,
}

impl Cursor {
    /// Resets the cursor's attribute template to default colors and flags.
    pub fn reset_attrs(&mut self) {
        self.template.fg = Color::Named(vte::ansi::NamedColor::Foreground);
        self.template.bg = Color::Named(vte::ansi::NamedColor::Background);
        self.template.flags = CellFlags::empty();
        self.template.extra = None;
    }
}
