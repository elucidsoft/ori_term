//! Scrolling, line feed, carriage return, and tab movement.

use super::TermHandler;
use crate::term_mode::TermMode;

impl TermHandler<'_> {
    pub(super) fn handle_scroll_up(&mut self, count: usize) {
        self.active_grid().scroll_up(count);
    }

    pub(super) fn handle_scroll_down(&mut self, count: usize) {
        self.active_grid().scroll_down(count);
    }

    pub(super) fn handle_set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        let grid = self.active_grid();
        grid.set_scroll_region(top, bottom);
        // Cursor moves to home after DECSTBM
        grid.goto(0, 0);
    }

    pub(super) fn handle_reverse_index(&mut self) {
        self.active_grid().reverse_index();
    }

    pub(super) fn handle_linefeed(&mut self) {
        self.grapheme.after_zwj = false;
        let lf_newline = self.mode.contains(TermMode::LINE_FEED_NEW_LINE);
        let grid = self.active_grid();
        grid.linefeed();
        if lf_newline {
            grid.carriage_return();
        }
    }

    pub(super) fn handle_carriage_return(&mut self) {
        self.grapheme.after_zwj = false;
        self.active_grid().carriage_return();
    }

    pub(super) fn handle_backspace(&mut self) {
        self.grapheme.after_zwj = false;
        self.active_grid().backspace();
    }

    pub(super) fn handle_newline(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        grid.linefeed();
        grid.carriage_return();
    }

    pub(super) fn handle_put_tab(&mut self, count: u16) {
        self.active_grid().advance_tab(count);
    }

    pub(super) fn handle_move_forward_tabs(&mut self, count: u16) {
        self.active_grid().advance_tab(count);
    }

    pub(super) fn handle_move_backward_tabs(&mut self, count: u16) {
        self.active_grid().backward_tab(count);
    }

    pub(super) fn handle_set_horizontal_tabstop(&mut self) {
        self.active_grid().set_tab_stop();
    }
}
