//! VTE handler implementation for `Term<T>`.
//!
//! Implements `vte::ansi::Handler` to process escape sequences, control
//! characters, and printable input. Each method delegates to the
//! appropriate grid/cursor/mode operation.

use vte::ansi::{CharsetIndex, Handler};

use crate::event::{Event, EventListener};
use crate::index::Column;

use super::Term;

impl<T: EventListener> Handler for Term<T> {
    /// Print a character to the terminal.
    ///
    /// Translates through the active charset, then writes via `grid.put_char`.
    #[inline]
    fn input(&mut self, c: char) {
        let c = self.charset.translate(c);
        self.grid_mut().put_char(c);
    }

    /// Move cursor left by one column, clearing the wrap-pending state.
    ///
    /// The wrap-pending state is when the cursor has advanced past the last
    /// column (`col == cols`) after a character write. Backspace resets
    /// this to the last column position.
    fn backspace(&mut self) {
        let grid = self.grid_mut();
        let col = grid.cursor().col().0;
        let cols = grid.cols();

        if col >= cols {
            // Wrap-pending: snap to last column.
            grid.cursor_mut().set_col(Column(cols - 1));
        } else if col > 0 {
            grid.cursor_mut().set_col(Column(col - 1));
        } else {
            // Already at column 0: no-op.
        }
    }

    /// Advance cursor to the next tab stop (or end of line).
    fn put_tab(&mut self, count: u16) {
        for _ in 0..count {
            self.grid_mut().tab();
        }
    }

    /// Move cursor down one line, scrolling if at the bottom of the scroll
    /// region.
    #[inline]
    fn linefeed(&mut self) {
        self.grid_mut().linefeed();
    }

    /// Move cursor to column 0.
    #[inline]
    fn carriage_return(&mut self) {
        self.grid_mut().carriage_return();
    }

    /// Ring the bell — send `Event::Bell` to the listener.
    #[inline]
    fn bell(&mut self) {
        self.event_listener.send_event(Event::Bell);
    }

    /// SUB: treated as a space character per ECMA-48.
    fn substitute(&mut self) {
        self.input(' ');
    }

    /// Switch the active charset slot (SO → G1, SI → G0).
    #[inline]
    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.charset.set_active(index);
    }
}

#[cfg(test)]
mod tests;
