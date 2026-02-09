use vte::ansi::Color;

use crate::cell::{Cell, CellFlags};

#[derive(Debug, Clone)]
pub struct Cursor {
    pub col: usize,
    pub row: usize,
    pub template: Cell,
    pub input_needs_wrap: bool,
}

impl Cursor {
    pub fn new(cols: usize, rows: usize) -> Self {
        let _ = (cols, rows);
        Self {
            col: 0,
            row: 0,
            template: Cell::default(),
            input_needs_wrap: false,
        }
    }

    pub fn reset_attrs(&mut self) {
        self.template.fg = Color::Named(vte::ansi::NamedColor::Foreground);
        self.template.bg = Color::Named(vte::ansi::NamedColor::Background);
        self.template.flags = CellFlags::empty();
        self.template.extra = None;
    }
}
