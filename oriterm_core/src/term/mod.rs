//! Terminal state machine.
//!
//! `Term<T: EventListener>` owns two grids (primary + alternate), mode flags,
//! color palette, charset state, and processes escape sequences via the
//! `vte::ansi::Handler` trait. Generic over `EventListener` for decoupling
//! from the UI layer.

pub mod charset;
pub mod mode;

pub use charset::CharsetState;
pub use mode::TermMode;

use crate::color::Palette;
use crate::event::EventListener;
use crate::grid::{CursorShape, Grid};

/// The terminal state machine.
///
/// Owns two grids (primary + alternate screen), terminal mode flags, color
/// palette, charset state, title, and keyboard mode stacks. Generic over
/// `T: EventListener` so tests can use `VoidListener` while the real app
/// routes events through winit.
#[derive(Debug)]
pub struct Term<T: EventListener> {
    /// Primary grid (active when not in alt screen).
    grid: Grid,
    /// Alternate grid (active during alt screen; no scrollback).
    alt_grid: Grid,
    /// Which grid is currently active.
    active_is_alt: bool,
    /// Terminal mode flags (DECSET/DECRST).
    mode: TermMode,
    /// Color palette (270 entries).
    palette: Palette,
    /// Character set translation state (G0–G3).
    charset: CharsetState,
    /// Window title (set by OSC 0/2).
    title: String,
    /// Pushed title stack (xterm extension).
    title_stack: Vec<String>,
    /// Cursor shape for rendering.
    cursor_shape: CursorShape,
    /// Kitty keyboard enhancement mode stack (active screen).
    keyboard_mode_stack: Vec<u8>,
    /// Kitty keyboard enhancement mode stack (inactive screen).
    inactive_keyboard_mode_stack: Vec<u8>,
    /// Event sink for terminal events.
    event_listener: T,
}

impl<T: EventListener> Term<T> {
    /// Create a new terminal with the given dimensions and scrollback capacity.
    pub fn new(lines: usize, cols: usize, scrollback: usize, listener: T) -> Self {
        Self {
            grid: Grid::with_scrollback(lines, cols, scrollback),
            alt_grid: Grid::with_scrollback(lines, cols, 0),
            active_is_alt: false,
            mode: TermMode::default(),
            palette: Palette::default(),
            charset: CharsetState::default(),
            title: String::new(),
            title_stack: Vec::new(),
            cursor_shape: CursorShape::default(),
            keyboard_mode_stack: Vec::new(),
            inactive_keyboard_mode_stack: Vec::new(),
            event_listener: listener,
        }
    }

    /// Reference to the active grid.
    pub fn grid(&self) -> &Grid {
        if self.active_is_alt { &self.alt_grid } else { &self.grid }
    }

    /// Mutable reference to the active grid.
    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.active_is_alt { &mut self.alt_grid } else { &mut self.grid }
    }

    /// Current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        self.mode
    }

    /// Reference to the color palette.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Current window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Reference to the charset state.
    pub fn charset(&self) -> &CharsetState {
        &self.charset
    }

    /// Reference to the event listener.
    pub fn event_listener(&self) -> &T {
        &self.event_listener
    }

    /// The title stack (xterm push/pop title).
    pub fn title_stack(&self) -> &[String] {
        &self.title_stack
    }

    /// Switch between primary and alternate screen.
    ///
    /// Saves/restores cursor, toggles `active_is_alt`, swaps keyboard mode
    /// stacks, and marks all lines dirty.
    pub fn swap_alt(&mut self) {
        if self.active_is_alt {
            // Switching back to primary: save alt cursor, restore primary cursor.
            self.alt_grid.save_cursor();
            self.grid.restore_cursor();
        } else {
            // Switching to alt: save primary cursor, restore alt cursor.
            self.grid.save_cursor();
            self.alt_grid.restore_cursor();
        }

        self.active_is_alt = !self.active_is_alt;
        std::mem::swap(&mut self.keyboard_mode_stack, &mut self.inactive_keyboard_mode_stack);
        self.grid_mut().dirty_mut().mark_all();
    }
}

#[cfg(test)]
mod tests;
