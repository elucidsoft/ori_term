//! Tab state: grid, PTY, VTE parser, and shell integration.

mod interceptor;
pub mod terminal_state;
mod types;

pub use terminal_state::TerminalState;
pub use types::{CharsetState, Notification, PromptState, SpawnConfig, TabId, TermEvent};

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use vte::ansi::{CursorShape, KeyboardModes};
use winit::event_loop::EventLoopProxy;

use crate::config::ColorConfig;
use crate::grid::Grid;
use crate::log;
use crate::palette::ColorScheme;
use crate::search::SearchState;
use crate::selection::{Selection, SelectionPoint};
use crate::shell_integration;
use crate::term_mode::TermMode;

pub struct Tab {
    pub id: TabId,
    /// Thread-shared terminal state behind a mutex.
    pub terminal: Arc<Mutex<TerminalState>>,

    // PTY handles (main thread for shutdown, reader thread for parsing)
    pub pty_writer: Option<Box<dyn Write + Send>>,
    pub pty_master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,

    // UI state (main thread only — never accessed by PTY thread)
    pub selection: Option<Selection>,
    pub search: Option<SearchState>,
    /// True when an inactive tab received a bell — shows badge in tab bar.
    pub has_bell_badge: bool,
}

impl Tab {
    pub fn spawn(cfg: SpawnConfig) -> Result<Self, Box<dyn std::error::Error>> {
        log(&format!("Tab::spawn start for {:?}", cfg.id));

        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(portable_pty::PtySize {
            rows: cfg.rows as u16,
            cols: cfg.cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        log("  pty opened");

        let shell_line = cfg.shell.unwrap_or_else(Self::default_shell);
        let mut parts = shell_line.split_whitespace();
        let shell_program = parts.next().unwrap_or("sh").to_owned();
        let shell_args: Vec<&str> = parts.collect();
        let mut cmd = portable_pty::CommandBuilder::new(&shell_program);
        for &arg in &shell_args {
            cmd.arg(arg);
        }

        let detected_shell = cfg
            .integration_dir
            .as_ref()
            .and_then(|_| shell_integration::detect_shell(&shell_program));

        // For WSL, CWD is passed via --cd (Linux paths don't work as Windows CWD).
        // For native shells, use cmd.cwd().
        let is_wsl = detected_shell == Some(shell_integration::Shell::Wsl);
        if let Some(ref dir) = cfg.cwd {
            if !is_wsl {
                cmd.cwd(dir);
            }
        }

        if let Some(ref integ_dir) = cfg.integration_dir {
            if let Some(shell_type) = detected_shell {
                let extra_arg = shell_integration::setup_injection(
                    &mut cmd,
                    shell_type,
                    integ_dir,
                    cfg.cwd.as_deref(),
                );
                if let Some(arg) = extra_arg {
                    cmd.arg(arg);
                }
            }
        }

        let child = pair.slave.spawn_command(cmd)?;
        log(&format!("  {} spawned", shell_program));
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        log("  reader/writer ready");

        spawn_reader_thread(cfg.id, reader, cfg.proxy.clone());
        #[cfg(target_os = "windows")]
        spawn_child_waiter(cfg.id, &*child, cfg.proxy);

        let initial_title = derive_initial_title(cfg.id, &shell_args, is_wsl);

        let terminal_state = TerminalState::new(
            cfg.cols,
            cfg.rows,
            cfg.max_scrollback,
            cfg.cursor_shape,
            initial_title,
            is_wsl,
        );

        log(&format!("Tab::spawn done for {:?}", cfg.id));
        Ok(Self {
            id: cfg.id,
            terminal: Arc::new(Mutex::new(terminal_state)),
            pty_writer: Some(writer),
            pty_master: pair.master,
            child,
            selection: None,
            search: None,
            has_bell_badge: false,
        })
    }

    /// Kill the child process and close PTY handles.
    /// Must be called before dropping to avoid `ClosePseudoConsole` blocking
    /// on Windows (the `ConPTY` reader thread holds a pipe handle).
    pub fn shutdown(&mut self) {
        // Close writer first so the child sees EOF on stdin
        self.pty_writer.take();
        // Kill the child process so ClosePseudoConsole returns quickly
        let _ = self.child.kill();
    }

    fn default_shell() -> String {
        #[cfg(target_os = "windows")]
        {
            "cmd.exe".to_owned()
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("SHELL").unwrap_or_else(|_| "sh".to_owned())
        }
    }

    // ── Grid access (convenience, locks internally) ────────────────────

    /// Lock the terminal and return a mapped guard to the active grid.
    ///
    /// The returned guard auto-derefs to `Grid`. The mutex is held for the
    /// guard's lifetime — callers must not call other lock-taking methods
    /// while this guard is alive.
    pub fn grid(&self) -> MappedMutexGuard<'_, Grid> {
        MutexGuard::map(self.terminal.lock(), |ts| ts.active_grid_mut())
    }

    /// Lock the terminal and return a mapped guard to the active grid (mutable).
    ///
    /// Identical to `grid()` since `Mutex` always gives exclusive access.
    pub fn grid_mut(&self) -> MappedMutexGuard<'_, Grid> {
        MutexGuard::map(self.terminal.lock(), |ts| ts.active_grid_mut())
    }

    // ── Copy-type property accessors ───────────────────────────────────

    /// Read the current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        self.terminal.lock().mode
    }

    /// Read the current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.terminal.lock().cursor_shape
    }

    /// Check whether the grid content has been modified since last render.
    pub fn grid_dirty(&self) -> bool {
        self.terminal.lock().grid_dirty
    }

    /// Set the grid dirty flag.
    pub fn set_grid_dirty(&self, dirty: bool) {
        self.terminal.lock().grid_dirty = dirty;
    }

    /// Read the bell start time (if bell is active).
    pub fn bell_start(&self) -> Option<Instant> {
        self.terminal.lock().bell_start
    }

    /// Read the keyboard mode stack.
    pub fn keyboard_mode_stack(&self) -> Vec<KeyboardModes> {
        self.terminal.lock().keyboard_mode_stack.clone()
    }

    // ── Clone-type property accessors ──────────────────────────────────

    /// Clone the current working directory.
    pub fn cwd(&self) -> Option<String> {
        self.terminal.lock().cwd.clone()
    }

    // ── Scroll operations ──────────────────────────────────────────────

    /// Scroll up by `page_size` lines (into history).
    pub fn scroll_page_up(&self, page_size: usize) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        let max = grid.scrollback.len();
        grid.display_offset = (grid.display_offset + page_size).min(max);
        term.grid_dirty = true;
    }

    /// Scroll down by `page_size` lines (toward live).
    pub fn scroll_page_down(&self, page_size: usize) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        grid.display_offset = grid.display_offset.saturating_sub(page_size);
        term.grid_dirty = true;
    }

    /// Scroll to the top of scrollback history.
    pub fn scroll_to_top(&self) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        grid.display_offset = grid.scrollback.len();
        term.grid_dirty = true;
    }

    /// Scroll to the live (bottom) position.
    pub fn scroll_to_bottom(&self) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        grid.display_offset = 0;
        term.grid_dirty = true;
    }

    /// Scroll by `delta` lines: positive = up (into history), negative = down.
    pub fn scroll_lines(&self, delta: i32) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        if delta > 0 {
            let max = grid.scrollback.len();
            grid.display_offset = (grid.display_offset + delta as usize).min(max);
        } else {
            grid.display_offset = grid.display_offset.saturating_sub((-delta) as usize);
        }
        term.grid_dirty = true;
    }

    // ── Selection operations ───────────────────────────────────────────

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        if self.selection.is_some() {
            self.selection = None;
            self.terminal.lock().grid_dirty = true;
        }
    }

    /// Replace the current selection.
    pub fn set_selection(&mut self, sel: Selection) {
        self.selection = Some(sel);
        self.terminal.lock().grid_dirty = true;
    }

    /// Update the end point of the current selection (drag tracking).
    pub fn update_selection_end(&mut self, end: SelectionPoint) {
        if let Some(ref mut sel) = self.selection {
            sel.end = end;
            self.terminal.lock().grid_dirty = true;
        }
    }

    // ── Prompt navigation ──────────────────────────────────────────────

    /// Navigate to the previous shell prompt (OSC 133 marker) in scrollback.
    pub fn navigate_to_previous_prompt(&self) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        let sb_len = grid.scrollback.len();
        // Current top of viewport as scrollback index.
        let viewport_top_sb = sb_len.saturating_sub(grid.display_offset);
        // Scan scrollback rows backwards from just above viewport top.
        let mut target_sb = None;
        for i in (0..viewport_top_sb).rev() {
            if grid.scrollback[i].prompt_start {
                target_sb = Some(i);
                break;
            }
        }
        if let Some(sb_idx) = target_sb {
            grid.display_offset = sb_len.saturating_sub(sb_idx);
            term.grid_dirty = true;
        }
    }

    /// Navigate to the next shell prompt (OSC 133 marker) below the viewport.
    pub fn navigate_to_next_prompt(&self) {
        let mut term = self.terminal.lock();
        let grid = term.active_grid_mut();
        let sb_len = grid.scrollback.len();
        // Current bottom of viewport as absolute index.
        let viewport_bottom_sb = sb_len.saturating_sub(grid.display_offset) + grid.lines;
        // Scan forward from below viewport.
        let total_rows = sb_len + grid.lines;
        let mut target_sb = None;
        for i in viewport_bottom_sb..total_rows {
            let has_prompt = if i < sb_len {
                grid.scrollback[i].prompt_start
            } else {
                grid.row(i - sb_len).prompt_start
            };
            if has_prompt {
                target_sb = Some(i);
                break;
            }
        }
        if let Some(idx) = target_sb {
            if idx < sb_len {
                grid.display_offset = sb_len.saturating_sub(idx);
            } else {
                grid.display_offset = 0;
            }
        } else {
            // No prompt below — scroll to live.
            grid.display_offset = 0;
        }
        term.grid_dirty = true;
    }

    // ── PTY I/O ────────────────────────────────────────────────────────

    /// Process raw PTY output through both VTE parsers.
    pub fn process_output(&mut self, data: &[u8]) {
        self.terminal
            .lock()
            .process_output(data, &mut self.pty_writer);
    }

    /// Write bytes to the PTY.
    pub fn send_pty(&mut self, data: &[u8]) {
        if let Some(writer) = self.pty_writer.as_mut() {
            match writer.write_all(data) {
                Ok(()) => {
                    let _ = writer.flush();
                }
                Err(e) => log(&format!("send_pty ERROR for tab {:?}: {e}", self.id)),
            }
        }
    }

    // ── Resize ─────────────────────────────────────────────────────────

    /// Resize grids and PTY to the given dimensions.
    pub fn resize(&self, cols: usize, rows: usize, pixel_width: u16, pixel_height: u16) {
        {
            let mut term = self.terminal.lock();
            // Alt screen never reflows (full-screen apps redraw themselves).
            let reflow = true;
            term.primary_grid.resize(cols, rows, reflow);
            term.alt_grid.resize(cols, rows, false);
        }
        let _ = self.pty_master.resize(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width,
            pixel_height,
        });
    }

    // ── Notifications ──────────────────────────────────────────────────

    /// Drain pending OSC 9/99/777 notifications, returning them to the caller.
    pub fn drain_notifications(&self) -> Vec<Notification> {
        self.terminal.lock().drain_notifications()
    }

    // ── Color / cursor config ──────────────────────────────────────────

    /// Apply color scheme, overrides, and bold-is-bright in one call.
    pub fn apply_color_config(
        &self,
        scheme: Option<&ColorScheme>,
        colors: &ColorConfig,
        bold_is_bright: bool,
    ) {
        self.terminal
            .lock()
            .apply_color_config(scheme, colors, bold_is_bright);
    }

    /// Set the cursor shape (block, underline, bar).
    pub fn set_cursor_shape(&self, shape: CursorShape) {
        self.terminal.lock().cursor_shape = shape;
    }

    // ── Search ─────────────────────────────────────────────────────────

    /// Open a new search session, replacing any existing one.
    pub fn open_search(&mut self) {
        self.search = Some(SearchState::new());
        self.terminal.lock().grid_dirty = true;
    }

    /// Close the active search session.
    pub fn close_search(&mut self) {
        self.search = None;
        self.terminal.lock().grid_dirty = true;
    }

    /// Update the search query. Handles the borrow-checker dance of
    /// borrowing the grid while mutating `self.search`.
    pub fn update_search_query(&mut self) {
        if let Some(mut search) = self.search.take() {
            let term = self.terminal.lock();
            search.update_query(term.active_grid());
            drop(term);
            self.search = Some(search);
            self.terminal.lock().grid_dirty = true;
        }
    }

    // ── Title ──────────────────────────────────────────────────────────

    /// Return the display title for the tab bar. If the shell explicitly set a
    /// title via OSC 0/2, use that. Otherwise derive a short path from CWD.
    ///
    /// Returns an owned `String` because the title data is behind a mutex.
    pub fn effective_title(&self) -> String {
        let term = self.terminal.lock();
        if term.has_explicit_title {
            return term.title.clone();
        }
        if let Some(ref cwd) = term.cwd {
            return short_path(cwd);
        }
        term.title.clone()
    }
}

/// Spawn the PTY reader thread that forwards output to the event loop.
fn spawn_reader_thread(
    id: TabId,
    mut reader: Box<dyn Read + Send>,
    proxy: EventLoopProxy<TermEvent>,
) {
    thread::spawn(move || {
        log(&format!("reader thread started for tab {:?}", id));
        let mut buf = vec![0u8; 65536];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    log(&format!("reader: eof for tab {:?}", id));
                    let _ = proxy.send_event(TermEvent::PtyExited(id));
                    break;
                }
                Err(e) => {
                    log(&format!("reader error for tab {:?}: {e}", id));
                    let _ = proxy.send_event(TermEvent::PtyExited(id));
                    break;
                }
                Ok(n) => {
                    let _ = proxy.send_event(TermEvent::PtyOutput(id, buf[..n].to_vec()));
                }
            }
        }
    });
}

/// Spawn a thread that waits for the child process to exit.
///
/// Windows/ConPTY does not reliably deliver EOF to the reader when the child
/// exits, so we wait on the process handle directly (like Alacritty, `WezTerm`,
/// and Ghostty all do).
#[cfg(target_os = "windows")]
fn spawn_child_waiter(
    id: TabId,
    child: &dyn portable_pty::Child,
    proxy: EventLoopProxy<TermEvent>,
) {
    if let Some(pid) = child.process_id() {
        thread::spawn(move || {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::{INFINITE, OpenProcess, WaitForSingleObject};
            const SYNCHRONIZE: u32 = 0x0010_0000;

            log(&format!(
                "child waiter thread started for tab {:?} (pid {})",
                id, pid
            ));
            #[allow(unsafe_code)]
            let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
            if !handle.is_null() {
                #[allow(unsafe_code)]
                unsafe {
                    WaitForSingleObject(handle, INFINITE)
                };
                #[allow(unsafe_code)]
                unsafe {
                    CloseHandle(handle)
                };
                log(&format!("child exited (waiter) for tab {:?}", id));
                let _ = proxy.send_event(TermEvent::PtyExited(id));
            }
        });
    }
}

/// Derive a good initial title. For WSL, use the distro name.
fn derive_initial_title(id: TabId, shell_args: &[&str], is_wsl: bool) -> String {
    if is_wsl {
        shell_args
            .iter()
            .zip(shell_args.iter().skip(1))
            .find(|&(&flag, _)| flag == "-d" || flag == "--distribution")
            .map_or_else(|| "WSL".to_owned(), |(_, &name)| name.to_owned())
    } else {
        format!("Tab {}", id.0)
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
