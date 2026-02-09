use std::io::Write;

use crate::grid::Grid;
use crate::log;

pub struct Performer<'a> {
    pub grid: &'a mut Grid,
    pub writer: &'a mut Option<Box<dyn Write + Send>>,
}

impl vte::Perform for Performer<'_> {
    fn print(&mut self, c: char) {
        self.grid.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.grid.newline(),
            b'\r' => self.grid.carriage_return(),
            0x08 => self.grid.backspace(),
            b'\t' => {
                let next = (self.grid.cursor_col / 8 + 1) * 8;
                self.grid.cursor_col = next.min(self.grid.cols - 1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let ps: Vec<u16> = params.iter().flat_map(|p| p.iter().copied()).collect();
        let p0 = ps.first().copied().unwrap_or(0);
        let p1 = ps.get(1).copied().unwrap_or(0);

        match action {
            'H' | 'f' => {
                let row = if p0 == 0 { 0 } else { (p0 - 1) as usize };
                let col = if p1 == 0 { 0 } else { (p1 - 1) as usize };
                self.grid.cursor_row = row.min(self.grid.rows - 1);
                self.grid.cursor_col = col.min(self.grid.cols - 1);
            }
            'A' => {
                let n = p0.max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = p0.max(1) as usize;
                self.grid.cursor_row = (self.grid.cursor_row + n).min(self.grid.rows - 1);
            }
            'C' => {
                let n = p0.max(1) as usize;
                self.grid.cursor_col = (self.grid.cursor_col + n).min(self.grid.cols - 1);
            }
            'D' => {
                let n = p0.max(1) as usize;
                self.grid.cursor_col = self.grid.cursor_col.saturating_sub(n);
            }
            'J' => {
                if p0 == 2 || p0 == 3 {
                    self.grid.clear();
                }
            }
            'K' => {
                if p0 == 0 {
                    self.grid.erase_line_from_cursor();
                }
            }
            'n' => {
                if p0 == 6 {
                    let response = format!(
                        "\x1b[{};{}R",
                        self.grid.cursor_row + 1,
                        self.grid.cursor_col + 1
                    );
                    if let Some(w) = self.writer.as_mut() {
                        let _ = w.write_all(response.as_bytes());
                        let _ = w.flush();
                    }
                    log(&format!("DSR reply: {:?}", response));
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn hook(
        &mut self,
        _params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
}
