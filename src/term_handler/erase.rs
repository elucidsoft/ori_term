//! Erase and editing operations.

use vte::ansi::{ClearMode, LineClearMode, TabulationClearMode};

use super::TermHandler;

impl TermHandler<'_> {
    pub(super) fn handle_clear_screen(&mut self, mode: ClearMode) {
        self.grapheme.after_zwj = false;
        self.active_grid().erase_display(mode);
    }

    pub(super) fn handle_clear_line(&mut self, mode: LineClearMode) {
        self.grapheme.after_zwj = false;
        self.active_grid().erase_line(mode);
    }

    pub(super) fn handle_clear_tabs(&mut self, mode: TabulationClearMode) {
        self.active_grid().clear_tab_stops(mode);
    }

    pub(super) fn handle_erase_chars(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().erase_chars(count);
    }

    pub(super) fn handle_delete_chars(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().delete_chars(count);
    }

    pub(super) fn handle_insert_blank(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().insert_blank_chars(count);
    }

    pub(super) fn handle_insert_blank_lines(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().insert_lines(count);
    }

    pub(super) fn handle_delete_lines(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        self.active_grid().delete_lines(count);
    }
}
