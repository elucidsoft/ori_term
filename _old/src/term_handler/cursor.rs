//! Cursor movement and save/restore.

use super::TermHandler;

impl TermHandler<'_> {
    pub(super) fn handle_goto(&mut self, line: i32, col: usize) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto(row, col);
    }

    pub(super) fn handle_goto_line(&mut self, line: i32) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto_line(row);
    }

    pub(super) fn handle_goto_col(&mut self, col: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().goto_col(col);
    }

    pub(super) fn handle_move_up(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().move_up(n);
    }

    pub(super) fn handle_move_down(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().move_down(n);
    }

    pub(super) fn handle_move_forward(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().move_forward(n);
    }

    pub(super) fn handle_move_backward(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().move_backward(n);
    }

    pub(super) fn handle_move_down_and_cr(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        grid.move_down(n);
        grid.carriage_return();
    }

    pub(super) fn handle_move_up_and_cr(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        grid.move_up(n);
        grid.carriage_return();
    }

    pub(super) fn handle_save_cursor_position(&mut self) {
        self.active_grid().save_cursor();
    }

    pub(super) fn handle_restore_cursor_position(&mut self) {
        self.active_grid().restore_cursor();
    }
}
