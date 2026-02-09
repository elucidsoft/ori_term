pub const BG: u32 = 0x00000000;
pub const FG: u32 = 0x00cdd6f4;
pub const CURSOR_COLOR: u32 = 0x00f5e0dc;

#[derive(Clone)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', fg: FG }
    }
}

pub struct Grid {
    pub cells: Vec<Cell>,
    pub cols: usize,
    pub rows: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cells: vec![Cell::default(); cols * rows],
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
        }
    }

    pub fn put_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
        }
        if self.cursor_row >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows - 1;
        }
        let idx = self.cursor_row * self.cols + self.cursor_col;
        self.cells[idx] = Cell { ch, fg: FG };
        self.cursor_col += 1;
    }

    pub fn newline(&mut self) {
        self.cursor_row += 1;
        if self.cursor_row >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows - 1;
        }
    }

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn scroll_up(&mut self) {
        self.cells.drain(..self.cols);
        self.cells
            .extend(std::iter::repeat_n(Cell::default(), self.cols));
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    pub fn erase_line_from_cursor(&mut self) {
        for col in self.cursor_col..self.cols {
            self.cells[self.cursor_row * self.cols + col] = Cell::default();
        }
    }
}
