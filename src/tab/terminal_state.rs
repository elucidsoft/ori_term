//! Thread-shared terminal state — grids, VTE parsers, and terminal properties.
//!
//! `TerminalState` contains everything that both the PTY reader thread and the
//! main/UI thread need access to. It is wrapped in `Arc<parking_lot::Mutex<>>`
//! inside `Tab`.

use std::path::Path;
use std::time::Instant;

use vte::ansi::{CursorShape, KeyboardModes};

use crate::config::ColorConfig;
use crate::grid::Grid;
use crate::palette::{ColorScheme, Palette};
use crate::term_handler::{GraphemeState, TermHandler};
use crate::term_mode::TermMode;

use super::interceptor::RawInterceptor;
use super::types::{CharsetState, Notification, PromptState, PtyWriter};

/// Terminal state shared between the PTY reader thread and the main thread.
///
/// Contains grids, VTE parsers, palette, mode flags, title, and other state
/// that VTE parsing mutates and rendering reads.
#[allow(clippy::struct_excessive_bools, reason = "terminal state needs multiple flag fields")]
pub struct TerminalState {
    // Grids
    pub primary_grid: Grid,
    pub alt_grid: Grid,
    pub active_is_alt: bool,

    // VTE parsers (PTY thread uses these; they live with grid state)
    processor: vte::ansi::Processor,
    raw_parser: vte::Parser,
    grapheme_state: GraphemeState,

    // Terminal state that VTE parsing mutates AND rendering reads
    pub palette: Palette,
    pub mode: TermMode,
    pub cursor_shape: CursorShape,
    pub charset: CharsetState,
    pub title: String,
    pub title_stack: Vec<String>,
    pub has_explicit_title: bool,
    pub suppress_title: bool,
    pub keyboard_mode_stack: Vec<KeyboardModes>,
    inactive_keyboard_mode_stack: Vec<KeyboardModes>,

    // Bell state (PTY sets, renderer reads)
    pub bell_start: Option<Instant>,

    // Shell integration (PTY sets, main thread reads)
    pub cwd: Option<String>,
    pub prompt_state: PromptState,
    pub pending_notifications: Vec<Notification>,
    prompt_mark_pending: bool,

    /// Set by VTE parsing when title or CWD changes; cleared by main thread.
    pub title_dirty: bool,
}

impl TerminalState {
    /// Create a new terminal state with the given grid dimensions.
    pub fn new(
        cols: usize,
        rows: usize,
        max_scrollback: usize,
        cursor_shape: CursorShape,
        initial_title: String,
        suppress_title: bool,
    ) -> Self {
        Self {
            primary_grid: Grid::with_max_scrollback(cols, rows, max_scrollback),
            alt_grid: Grid::new(cols, rows),
            active_is_alt: false,
            processor: vte::ansi::Processor::new(),
            raw_parser: vte::Parser::new(),
            grapheme_state: GraphemeState::default(),
            palette: Palette::new(),
            mode: TermMode::default(),
            cursor_shape,
            charset: CharsetState::default(),
            title: initial_title,
            title_stack: Vec::new(),
            has_explicit_title: false,
            suppress_title,
            keyboard_mode_stack: Vec::new(),
            inactive_keyboard_mode_stack: Vec::new(),
            bell_start: None,
            cwd: None,
            prompt_state: PromptState::default(),
            pending_notifications: Vec::new(),
            prompt_mark_pending: false,
            title_dirty: false,
        }
    }

    /// Returns a reference to the active grid (primary or alternate).
    pub fn active_grid(&self) -> &Grid {
        if self.active_is_alt {
            &self.alt_grid
        } else {
            &self.primary_grid
        }
    }

    /// Returns a mutable reference to the active grid.
    pub fn active_grid_mut(&mut self) -> &mut Grid {
        if self.active_is_alt {
            &mut self.alt_grid
        } else {
            &mut self.primary_grid
        }
    }

    /// Process raw PTY output through both the raw interceptor and VTE processor.
    ///
    /// `pty_writer` is the shared writer for VTE responses (DA, DECRPM, etc.).
    pub fn process_output(&mut self, data: &[u8], pty_writer: &PtyWriter) {
        let old_title = self.effective_title();

        // Run the raw interceptor first to capture OSC 7/133/9/99/777/XTVERSION
        // (sequences that vte::ansi::Processor silently drops).
        let mut interceptor = RawInterceptor {
            pty_writer,
            cwd: &mut self.cwd,
            prompt_state: &mut self.prompt_state,
            pending_notifications: &mut self.pending_notifications,
            prompt_mark_pending: &mut self.prompt_mark_pending,
            has_explicit_title: &mut self.has_explicit_title,
            suppress_title: &mut self.suppress_title,
        };
        self.raw_parser.advance(&mut interceptor, data);

        // Then run the normal high-level Processor for everything else.
        let mut handler = TermHandler::new(
            &mut self.primary_grid,
            &mut self.alt_grid,
            &mut self.mode,
            &mut self.palette,
            &mut self.title,
            pty_writer,
            &mut self.active_is_alt,
            &mut self.cursor_shape,
            &mut self.charset,
            &mut self.title_stack,
            &mut self.grapheme_state,
            &mut self.keyboard_mode_stack,
            &mut self.inactive_keyboard_mode_stack,
            &mut self.bell_start,
            &mut self.has_explicit_title,
            &mut self.suppress_title,
        );
        self.processor.advance(&mut handler, data);

        // Mark the cursor row as a prompt start after both parsers have updated.
        if self.prompt_mark_pending {
            self.prompt_mark_pending = false;
            let row = self.active_grid().cursor.row;
            self.active_grid_mut().row_mut(row).prompt_start = true;
        }

        // Detect title/CWD changes (covers both OSC 0/2 and OSC 7).
        if self.effective_title() != old_title {
            self.title_dirty = true;
        }
    }

    /// Return the display title. If the shell explicitly set a title via
    /// OSC 0/2, use that. Otherwise derive a short path from CWD, falling
    /// back to the raw title string.
    pub fn effective_title(&self) -> String {
        if self.has_explicit_title {
            return self.title.clone();
        }
        if let Some(ref cwd) = self.cwd {
            return short_path(cwd);
        }
        self.title.clone()
    }

    /// Drain pending OSC 9/99/777 notifications, returning them to the caller.
    pub fn drain_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.pending_notifications)
    }

    /// Apply color scheme, overrides, and bold-is-bright in one call.
    pub fn apply_color_config(
        &mut self,
        scheme: Option<&ColorScheme>,
        colors: &ColorConfig,
        bold_is_bright: bool,
    ) {
        if let Some(scheme) = scheme {
            self.palette.set_scheme(scheme);
        }
        self.palette.apply_overrides(colors);
        self.palette.bold_is_bright = bold_is_bright;
    }
}

/// Shorten a path for tab bar display: `~` for home, last component otherwise.
fn short_path(path: &str) -> String {
    // Try to replace home directory with ~.
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            let relative = if rest.is_empty() {
                ""
            } else {
                rest.trim_start_matches('/')
            };
            if relative.is_empty() {
                return "~".to_owned();
            }
            return format!("~/{relative}");
        }
    }
    #[cfg(target_os = "windows")]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if let Some(rest) = path.strip_prefix(&profile) {
            let relative = if rest.is_empty() {
                ""
            } else {
                rest.trim_start_matches('\\').trim_start_matches('/')
            };
            if relative.is_empty() {
                return "~".to_owned();
            }
            return format!("~\\{relative}");
        }
    }
    // Fall back to last path component.
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_owned()
}
