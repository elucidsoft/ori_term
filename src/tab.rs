use std::io::{Read, Write};
use std::path::Path;
use std::thread;
use std::time::Instant;

use vte::Perform;
use vte::ansi::{CharsetIndex, CursorShape, KeyboardModes, StandardCharset};
use winit::event_loop::EventLoopProxy;

use crate::grid::Grid;
use crate::log;
use crate::palette::Palette;
use crate::search::SearchState;
use crate::selection::Selection;
use crate::shell_integration;
use crate::term_handler::{GraphemeState, TermHandler};
use crate::term_mode::TermMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// Charset state: 4 slots (G0-G3) and an active index.
#[derive(Debug, Clone)]
pub struct CharsetState {
    pub charsets: [StandardCharset; 4],
    pub active: CharsetIndex,
}

impl Default for CharsetState {
    fn default() -> Self {
        Self {
            charsets: [StandardCharset::Ascii; 4],
            active: CharsetIndex::G0,
        }
    }
}

impl CharsetState {
    pub fn map(&self, c: char) -> char {
        let idx = match self.active {
            CharsetIndex::G0 => 0,
            CharsetIndex::G1 => 1,
            CharsetIndex::G2 => 2,
            CharsetIndex::G3 => 3,
        };
        self.charsets[idx].map(c)
    }
}

/// OSC 133 semantic prompt state.
///
/// Shell integration uses these markers to distinguish prompt, command input,
/// and command output regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptState {
    /// No prompt markers received yet or after command output completes.
    #[default]
    None,
    /// `OSC 133;A` — prompt has started (user sees the prompt).
    PromptStart,
    /// `OSC 133;B` — command input has started (user is typing).
    CommandStart,
    /// `OSC 133;C` — command output has started (command is running).
    OutputStart,
}

/// A desktop notification from OSC 9, 99, or 777.
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// Raw VTE `Perform` implementation that intercepts sequences the high-level
/// `vte::ansi::Processor` drops: OSC 7 (CWD), OSC 133 (prompt markers),
/// OSC 9/99/777 (notifications), and XTVERSION (CSI > q).
struct RawInterceptor<'a> {
    pty_writer: &'a mut Option<Box<dyn Write + Send>>,
    cwd: &'a mut Option<String>,
    prompt_state: &'a mut PromptState,
    pending_notifications: &'a mut Vec<Notification>,
    prompt_mark_pending: &'a mut bool,
    has_explicit_title: &'a mut bool,
    suppress_title: &'a mut bool,
}

impl Perform for RawInterceptor<'_> {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() || params[0].is_empty() {
            return;
        }
        match params[0] {
            // OSC 7 — Current working directory.
            // Format: OSC 7 ; file://hostname/path ST
            b"7" => {
                if params.len() >= 2 {
                    let uri = std::str::from_utf8(params[1]).unwrap_or_default();
                    // Strip file:// prefix and optional hostname to get the path.
                    let path = uri.strip_prefix("file://").map_or(uri, |rest| {
                        // Skip hostname (everything before the next /)
                        if let Some(slash) = rest.find('/') {
                            rest.split_at(slash).1
                        } else {
                            rest
                        }
                    });
                    if !path.is_empty() {
                        *self.cwd = Some(path.to_owned());
                        // CWD-based title should override ConPTY's auto-generated
                        // process title (e.g. C:\WINDOWS\system32\wsl.exe).
                        *self.has_explicit_title = false;
                        *self.suppress_title = false;
                    }
                }
            }
            // OSC 133 — Semantic prompt markers.
            // Format: OSC 133 ; <type>[;extras] ST
            b"133" => {
                if params.len() >= 2 && !params[1].is_empty() {
                    match params[1][0] {
                        b'A' => {
                            *self.prompt_state = PromptState::PromptStart;
                            *self.prompt_mark_pending = true;
                            *self.suppress_title = false;
                        }
                        b'B' => *self.prompt_state = PromptState::CommandStart,
                        b'C' => *self.prompt_state = PromptState::OutputStart,
                        b'D' => *self.prompt_state = PromptState::None,
                        _ => {}
                    }
                }
            }
            // OSC 9 — iTerm2 simple notification: ESC]9;body ST
            // OSC 99 — Kitty notification protocol: ESC]99;body ST
            b"9" | b"99" => {
                let body = if params.len() >= 2 {
                    String::from_utf8_lossy(params[1]).into_owned()
                } else {
                    String::new()
                };
                self.pending_notifications.push(Notification {
                    title: String::new(),
                    body,
                });
            }
            // OSC 777 — rxvt-unicode notification: ESC]777;notify;title;body ST
            b"777" => {
                if params.len() >= 2 {
                    let action = std::str::from_utf8(params[1]).unwrap_or_default();
                    if action == "notify" {
                        let title = params
                            .get(2)
                            .map(|p| String::from_utf8_lossy(p).into_owned())
                            .unwrap_or_default();
                        let body = params
                            .get(3)
                            .map(|p| String::from_utf8_lossy(p).into_owned())
                            .unwrap_or_default();
                        self.pending_notifications
                            .push(Notification { title, body });
                    }
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        _params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // XTVERSION: CSI > q — report terminal name and version.
        if action == 'q' && intermediates == [b'>'] {
            let version = env!("CARGO_PKG_VERSION");
            let build = include_str!("../BUILD_NUMBER").trim();
            // Response: DCS > | terminal-name(version) ST
            let response = format!("\x1bP>|oriterm({version} build {build})\x1b\\");
            if let Some(w) = self.pty_writer.as_mut() {
                let _ = w.write_all(response.as_bytes());
                let _ = w.flush();
            }
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct Tab {
    pub id: TabId,
    primary_grid: Grid,
    alt_grid: Grid,
    active_is_alt: bool,
    pub pty_writer: Option<Box<dyn Write + Send>>,
    processor: vte::ansi::Processor,
    pub title: String,
    pub palette: Palette,
    pub mode: TermMode,
    pub cursor_shape: CursorShape,
    pub charset: CharsetState,
    pub title_stack: Vec<String>,
    pub cwd: Option<String>,
    pub prompt_state: PromptState,
    pub selection: Option<Selection>,
    pub search: Option<SearchState>,
    raw_parser: vte::Parser,
    pub pty_master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    grapheme_state: GraphemeState,
    pub keyboard_mode_stack: Vec<KeyboardModes>,
    inactive_keyboard_mode_stack: Vec<KeyboardModes>,
    /// When the bell (BEL 0x07) last rang — drives visual bell flash decay.
    pub bell_start: Option<Instant>,
    /// True when an inactive tab received a bell — shows badge in tab bar.
    pub has_bell_badge: bool,
    /// Notifications received via OSC 9/99/777.
    pub pending_notifications: Vec<Notification>,
    /// True when the grid content has changed and needs a GPU rebuild.
    pub grid_dirty: bool,
    /// True when OSC 0/2 explicitly set the title (suppresses CWD-based title).
    pub has_explicit_title: bool,
    /// Set by `RawInterceptor` when OSC 133;A is received; consumed after processing.
    prompt_mark_pending: bool,
    /// Suppress OSC 0/2 title changes until the shell sends CWD or prompt markers.
    /// Prevents `ConPTY`'s auto-generated process-path title from flashing.
    pub suppress_title: bool,
}

#[derive(Debug)]
pub enum TermEvent {
    PtyOutput(TabId, Vec<u8>),
    PtyExited(TabId),
    ConfigReload,
}

impl Tab {
    pub fn spawn(
        id: TabId,
        cols: usize,
        rows: usize,
        proxy: EventLoopProxy<TermEvent>,
        shell: Option<&str>,
        max_scrollback: usize,
        initial_cursor_shape: CursorShape,
        integration_dir: Option<&Path>,
        cwd: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log(&format!("Tab::spawn start for {:?}", id));

        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        log("  pty opened");

        let shell_line = shell.map_or_else(Self::default_shell, String::from);
        let mut parts = shell_line.split_whitespace();
        let shell_program = parts.next().unwrap_or("sh").to_owned();
        let shell_args: Vec<&str> = parts.collect();
        let mut cmd = portable_pty::CommandBuilder::new(&shell_program);
        for &arg in &shell_args {
            cmd.arg(arg);
        }

        let detected_shell =
            integration_dir.and_then(|_| shell_integration::detect_shell(&shell_program));

        // For WSL, CWD is passed via --cd (Linux paths don't work as Windows CWD).
        // For native shells, use cmd.cwd().
        let is_wsl = detected_shell == Some(shell_integration::Shell::Wsl);
        if let Some(dir) = cwd {
            if !is_wsl {
                cmd.cwd(dir);
            }
        }

        if let Some(integ_dir) = integration_dir {
            if let Some(shell_type) = detected_shell {
                let extra_arg =
                    shell_integration::setup_injection(&mut cmd, shell_type, integ_dir, cwd);
                if let Some(arg) = extra_arg {
                    cmd.arg(arg);
                }
            }
        }

        let child = pair.slave.spawn_command(cmd)?;
        log(&format!("  {} spawned", shell_program));
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        log("  reader/writer ready");

        // On Windows, clone proxy for the child waiter thread (see below).
        #[cfg(target_os = "windows")]
        let wait_proxy = proxy.clone();

        let tab_id = id;
        thread::spawn(move || {
            log(&format!("reader thread started for tab {:?}", tab_id));
            let mut buf = vec![0u8; 65536];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        log(&format!("reader: eof for tab {:?}", tab_id));
                        let _ = proxy.send_event(TermEvent::PtyExited(tab_id));
                        break;
                    }
                    Err(e) => {
                        log(&format!("reader error for tab {:?}: {e}", tab_id));
                        let _ = proxy.send_event(TermEvent::PtyExited(tab_id));
                        break;
                    }
                    Ok(n) => {
                        let _ = proxy.send_event(TermEvent::PtyOutput(tab_id, buf[..n].to_vec()));
                    }
                }
            }
        });

        // Windows/ConPTY doesn't reliably deliver EOF to the reader when
        // the child exits, so we wait on the process handle directly
        // (like Alacritty, WezTerm, and Ghostty all do).
        #[cfg(target_os = "windows")]
        {
            if let Some(pid) = child.process_id() {
                let wait_id = id;
                thread::spawn(move || {
                    use windows_sys::Win32::Foundation::CloseHandle;
                    use windows_sys::Win32::System::Threading::{
                        INFINITE, OpenProcess, WaitForSingleObject,
                    };
                    const SYNCHRONIZE: u32 = 0x0010_0000;

                    log(&format!(
                        "child waiter thread started for tab {:?} (pid {})",
                        wait_id, pid
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
                        log(&format!("child exited (waiter) for tab {:?}", wait_id));
                        let _ = wait_proxy.send_event(TermEvent::PtyExited(wait_id));
                    }
                });
            }
        }

        // Derive a good initial title. For WSL, use the distro name.
        let initial_title = if is_wsl {
            shell_args
                .iter()
                .zip(shell_args.iter().skip(1))
                .find(|&(&flag, _)| flag == "-d" || flag == "--distribution")
                .map_or_else(|| "WSL".to_owned(), |(_, &name)| name.to_owned())
        } else {
            format!("Tab {}", id.0)
        };

        log(&format!("Tab::spawn done for {:?}", id));
        Ok(Self {
            id,
            primary_grid: Grid::with_max_scrollback(cols, rows, max_scrollback),
            alt_grid: Grid::new(cols, rows), // alt screen has no scrollback
            active_is_alt: false,
            pty_writer: Some(writer),
            processor: vte::ansi::Processor::new(),
            title: initial_title,
            palette: Palette::new(),
            mode: TermMode::default(),
            cursor_shape: initial_cursor_shape,
            charset: CharsetState::default(),
            title_stack: Vec::new(),
            cwd: None,
            prompt_state: PromptState::default(),
            selection: None,
            search: None,
            raw_parser: vte::Parser::new(),
            pty_master: pair.master,
            child,
            grapheme_state: GraphemeState::default(),
            keyboard_mode_stack: Vec::new(),
            inactive_keyboard_mode_stack: Vec::new(),
            bell_start: None,
            has_bell_badge: false,
            pending_notifications: Vec::new(),
            grid_dirty: true,
            has_explicit_title: false,
            prompt_mark_pending: false,
            suppress_title: is_wsl,
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

    pub fn grid(&self) -> &Grid {
        if self.active_is_alt {
            &self.alt_grid
        } else {
            &self.primary_grid
        }
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.active_is_alt {
            &mut self.alt_grid
        } else {
            &mut self.primary_grid
        }
    }

    pub fn process_output(&mut self, data: &[u8]) {
        self.grid_dirty = true;

        // Run the raw interceptor first to capture OSC 7/133/9/99/777/XTVERSION
        // (sequences that vte::ansi::Processor silently drops).
        let mut interceptor = RawInterceptor {
            pty_writer: &mut self.pty_writer,
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
            &mut self.pty_writer,
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
            let row = self.grid().cursor.row;
            self.grid_mut().row_mut(row).prompt_start = true;
        }
    }

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

    pub fn resize(&mut self, cols: usize, rows: usize, pixel_width: u16, pixel_height: u16) {
        // Alt screen never reflows (full-screen apps redraw themselves).
        let reflow = true;
        self.primary_grid.resize(cols, rows, reflow);
        self.alt_grid.resize(cols, rows, false);
        let _ = self.pty_master.resize(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width,
            pixel_height,
        });
    }

    /// Return the display title for the tab bar. If the shell explicitly set a
    /// title via OSC 0/2, use that. Otherwise derive a short path from CWD.
    pub fn effective_title(&self) -> String {
        if self.has_explicit_title {
            return self.title.clone();
        }
        if let Some(ref cwd) = self.cwd {
            return short_path(cwd);
        }
        self.title.clone()
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
