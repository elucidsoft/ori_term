use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

use crate::clipboard;
use crate::search::SearchState;
use crate::url_detect::{UrlDetectCache, UrlSegment};
use crate::config::{self, Config};
use crate::config_monitor::ConfigMonitor;
use crate::keybindings::{self, Action, KeyBinding};
use crate::drag::{DragPhase, DragState, DRAG_START_THRESHOLD, TEAR_OFF_THRESHOLD};
use crate::key_encoding::{self, KeyEventType, Modifiers};
use crate::gpu::renderer::{FrameParams, GpuRenderer, GpuState};
use crate::log;
use crate::palette::{self, ColorScheme, BUILTIN_SCHEMES};
use crate::render::FontSet;
use crate::selection::{
    self, Selection, SelectionMode, SelectionPoint, Side,
};
use crate::tab::{Tab, TabId, TermEvent};
use crate::tab_bar::{
    TabBarHit, TabBarLayout, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, NEW_TAB_BUTTON_WIDTH,
    GRID_PADDING_LEFT, GRID_PADDING_BOTTOM,
};
use crate::term_mode::TermMode;
use crate::window::TermWindow;

// Resize border thickness in pixels
const RESIZE_BORDER: f64 = 8.0;

// Double-click detection threshold in milliseconds
const DOUBLE_CLICK_MS: u128 = 400;

// Scroll lines per mouse wheel tick
const SCROLL_LINES: usize = 3;

pub struct App {
    config: Config,
    windows: HashMap<WindowId, TermWindow>,
    tabs: HashMap<TabId, Tab>,
    glyphs: FontSet,
    gpu: Option<GpuState>,
    renderer: Option<GpuRenderer>,
    drag: Option<DragState>,
    next_tab_id: u64,
    proxy: EventLoopProxy<TermEvent>,
    cursor_pos: HashMap<WindowId, PhysicalPosition<f64>>,
    hover_hit: HashMap<WindowId, TabBarHit>,
    modifiers: ModifiersState,
    first_window_created: bool,
    last_click_time: Option<Instant>,
    last_click_window: Option<WindowId>,
    // Selection state
    left_mouse_down: bool,
    last_grid_click_pos: Option<(usize, usize)>,
    click_count: u8,
    // Mouse reporting dedup
    last_mouse_cell: Option<(usize, usize)>,
    // Search
    search_active: Option<WindowId>,
    // Hyperlink hover
    hover_hyperlink: Option<(WindowId, String)>,
    // Implicit URL detection
    url_cache: UrlDetectCache,
    hover_url_range: Option<Vec<UrlSegment>>,
    // Dropdown menu & settings
    dropdown_open: Option<WindowId>,
    settings_window: Option<WindowId>,
    active_scheme: &'static str,
    _config_monitor: Option<ConfigMonitor>,
    bindings: Vec<KeyBinding>,
    scale_factor: f64,
}

impl App {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        std::panic::set_hook(Box::new(|info| {
            let _ = std::fs::write("oriterm_panic.log", format!("{info}"));
        }));

        let _ = std::fs::remove_file(crate::log_path());
        log("starting");

        let config = Config::load();
        log(&format!(
            "config: font_size={}, scheme={}, shell={:?}, scrollback={}, cols={}, rows={}, \
             opacity={}, tab_bar_opacity={}, blur={}, cursor={}, copy_on_select={}, bold_is_bright={}",
            config.font.size, config.colors.scheme, config.terminal.shell,
            config.terminal.scrollback, config.window.columns, config.window.rows,
            config.window.effective_opacity(), config.window.effective_tab_bar_opacity(),
            config.window.blur,
            config.terminal.cursor_style, config.behavior.copy_on_select,
            config.behavior.bold_is_bright,
        ));

        let glyphs = FontSet::load(config.font.size, config.font.family.as_deref());
        log(&format!(
            "font loaded: cell={}x{}, baseline={}, size={}",
            glyphs.cell_width, glyphs.cell_height, glyphs.baseline, glyphs.size
        ));

        let bindings = keybindings::merge_bindings(&config.keybind);

        let active_scheme = palette::find_scheme(&config.colors.scheme)
            .map_or("Catppuccin Mocha", |s| s.name);

        let event_loop = EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("event loop");
        let proxy = event_loop.create_proxy();

        let config_monitor = ConfigMonitor::new(proxy.clone());

        let mut app = Self {
            config,
            windows: HashMap::new(),
            tabs: HashMap::new(),
            glyphs,
            gpu: None,
            renderer: None,
            drag: None,
            next_tab_id: 1,
            proxy,
            cursor_pos: HashMap::new(),
            hover_hit: HashMap::new(),
            modifiers: ModifiersState::empty(),
            first_window_created: false,
            last_click_time: None,
            last_click_window: None,
            left_mouse_down: false,
            last_grid_click_pos: None,
            click_count: 0,
            last_mouse_cell: None,
            search_active: None,
            hover_hyperlink: None,
            url_cache: UrlDetectCache::default(),
            hover_url_range: None,
            dropdown_open: None,
            settings_window: None,
            active_scheme,
            _config_monitor: config_monitor,
            bindings,
            scale_factor: 1.0,
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }

    fn alloc_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    fn grid_dims_for_size(&self, width: u32, height: u32) -> (usize, usize) {
        let cw = self.glyphs.cell_width;
        let ch = self.glyphs.cell_height;
        let grid_w = (width as usize).saturating_sub(GRID_PADDING_LEFT);
        let grid_h = (height as usize).saturating_sub(TAB_BAR_HEIGHT + 10 + GRID_PADDING_BOTTOM);
        let cols = if cw > 0 { grid_w / cw } else { self.config.window.columns };
        let rows = if ch > 0 { grid_h / ch } else { self.config.window.rows };
        (cols.max(2), rows.max(1))
    }

    fn change_font_size(&mut self, window_id: WindowId, delta: f32) {
        let new_size = self.glyphs.size + delta * self.scale_factor as f32;
        self.glyphs = self.glyphs.resize(new_size);
        log(&format!(
            "font resize: size={}, cell={}x{}",
            self.glyphs.size, self.glyphs.cell_width, self.glyphs.cell_height
        ));
        // Rebuild atlas for new font size
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
            renderer.rebuild_atlas(gpu, &mut self.glyphs);
        }
        self.resize_all_tabs_in_window(window_id);
    }

    fn reset_font_size(&mut self, window_id: WindowId) {
        let scaled_size = self.config.font.size * self.scale_factor as f32;
        self.glyphs = self.glyphs.resize(scaled_size);
        log(&format!(
            "font reset: size={}, cell={}x{}",
            self.glyphs.size, self.glyphs.cell_width, self.glyphs.cell_height
        ));
        // Rebuild atlas for new font size
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
            renderer.rebuild_atlas(gpu, &mut self.glyphs);
        }
        self.resize_all_tabs_in_window(window_id);
    }

    fn resize_all_tabs_in_window(&mut self, window_id: WindowId) {
        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };
        let size = tw.window.inner_size();
        let (cols, rows) = self.grid_dims_for_size(size.width, size.height);
        let pixel_w = size.width as u16;
        let pixel_h = size.height as u16;

        for &tab_id in &tw.tabs.clone() {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.selection = None;
                tab.resize(cols, rows, pixel_w, pixel_h);
            }
        }
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    /// Execute a keybinding action. Returns `true` if consumed, `false` to
    /// fall through to the PTY (e.g. `SmartCopy` with no selection).
    fn execute_action(
        &mut self,
        action: &Action,
        window_id: WindowId,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        match action {
            Action::ZoomIn => {
                self.change_font_size(window_id, 1.0);
            }
            Action::ZoomOut => {
                self.change_font_size(window_id, -1.0);
            }
            Action::ZoomReset => {
                self.reset_font_size(window_id);
            }
            Action::NewTab => {
                self.new_tab_in_window(window_id);
            }
            Action::CloseTab => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    self.close_tab(tid, event_loop);
                }
            }
            Action::NextTab => {
                if let Some(tw) = self.windows.get_mut(&window_id) {
                    let n = tw.tabs.len();
                    if n > 1 {
                        tw.active_tab = (tw.active_tab + 1) % n;
                        tw.window.request_redraw();
                    }
                }
            }
            Action::PrevTab => {
                if let Some(tw) = self.windows.get_mut(&window_id) {
                    let n = tw.tabs.len();
                    if n > 1 {
                        tw.active_tab = (tw.active_tab + n - 1) % n;
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollPageUp => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let page = grid.lines;
                        let max = grid.scrollback.len();
                        grid.display_offset = (grid.display_offset + page).min(max);
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollPageDown => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let page = grid.lines;
                        grid.display_offset = grid.display_offset.saturating_sub(page);
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollToTop => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        grid.display_offset = grid.scrollback.len();
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollToBottom => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        tab.grid_mut().display_offset = 0;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::Copy => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get(&tid) {
                        if let Some(ref sel) = tab.selection {
                            let text = selection::extract_text(tab.grid(), sel);
                            if !text.is_empty() {
                                clipboard::set_text(&text);
                            }
                        }
                    }
                }
            }
            Action::Paste | Action::SmartPaste => {
                self.paste_from_clipboard(window_id);
            }
            Action::SmartCopy => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    let has_selection = self.tabs.get(&tid)
                        .is_some_and(|t| t.selection.is_some());
                    if has_selection {
                        if let Some(tab) = self.tabs.get(&tid) {
                            if let Some(ref sel) = tab.selection {
                                let text = selection::extract_text(tab.grid(), sel);
                                if !text.is_empty() {
                                    clipboard::set_text(&text);
                                }
                            }
                        }
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.selection = None;
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    } else {
                        // No selection — fall through to PTY.
                        return false;
                    }
                }
            }
            Action::ReloadConfig => {
                self.apply_config_reload();
            }
            Action::OpenSearch => {
                self.open_search(window_id);
            }
            Action::SendText(text) => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        tab.send_pty(text.as_bytes());
                    }
                }
            }
            Action::None => {
                // Explicitly unbound — should not appear after merge, but
                // consume the key if it does.
            }
        }
        true
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Option<WindowId> {
        let win_w = (self.glyphs.cell_width * self.config.window.columns) as u32 + GRID_PADDING_LEFT as u32;
        let win_h = (self.glyphs.cell_height * self.config.window.rows) as u32 + TAB_BAR_HEIGHT as u32 + 10 + GRID_PADDING_BOTTOM as u32;

        let use_transparent = self.config.window.effective_opacity() < 1.0;

        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title("oriterm")
            .with_inner_size(winit::dpi::PhysicalSize::new(win_w, win_h))
            .with_decorations(false)
            .with_visible(false)
            .with_transparent(use_transparent);

        // On Windows, DX12 + DirectComposition needs WS_EX_NOREDIRECTIONBITMAP
        // so the compositor reads alpha from our swapchain.
        #[cfg(target_os = "windows")]
        if use_transparent {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_no_redirection_bitmap(true);
        }

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("failed to create window: {e}"));
                return None;
            }
        };

        // Initialize GPU on first window creation
        if self.gpu.is_none() {
            let gpu = GpuState::new(&window, use_transparent);
            let renderer = GpuRenderer::new(&gpu, &mut self.glyphs);
            self.gpu = Some(gpu);
            self.renderer = Some(renderer);
        }

        let gpu = self.gpu.as_ref().expect("GPU initialized");
        let id = window.id();

        let tw = TermWindow::new(window.clone(), gpu)?;

        // Render a dark clear frame before showing the window
        if let Some(renderer) = &self.renderer {
            let s = crate::gpu::renderer::srgb_to_linear;
            let bg = [s(0x1e as f32 / 255.0), s(0x1e as f32 / 255.0), s(0x2e as f32 / 255.0), 1.0];
            renderer.clear_surface(gpu, &tw.surface, bg, self.config.window.effective_opacity());
        }

        // Apply compositor blur/vibrancy + layered window transparency
        apply_window_effects(&window, &self.config.window);

        // Now show the window with the dark frame already rendered
        window.set_visible(true);

        self.windows.insert(id, tw);
        log(&format!("created window {id:?}"));
        Some(id)
    }

    fn new_tab_in_window(
        &mut self,
        window_id: WindowId,
    ) -> Option<TabId> {
        // Compute grid size from window
        let default_cols = self.config.window.columns;
        let default_rows = self.config.window.rows;
        let (cols, rows) = self.windows.get(&window_id)
            .map_or((default_cols, default_rows), |tw| {
                let size = tw.window.inner_size();
                self.grid_dims_for_size(size.width, size.height)
            });

        let tab_id = self.alloc_tab_id();
        let cursor_shape = config::parse_cursor_style(&self.config.terminal.cursor_style);
        let tab = match Tab::spawn(
            tab_id,
            cols,
            rows,
            self.proxy.clone(),
            self.config.terminal.shell.as_deref(),
            self.config.terminal.scrollback,
            cursor_shape,
        ) {
            Ok(t) => t,
            Err(e) => {
                log(&format!("failed to spawn tab: {e}"));
                return None;
            }
        };

        self.tabs.insert(tab_id, tab);

        // Apply the active color scheme and behavior settings to the new tab
        if let Some(t) = self.tabs.get_mut(&tab_id) {
            if let Some(scheme) = palette::find_scheme(self.active_scheme) {
                t.palette.set_scheme(scheme);
            }
            t.palette.bold_is_bright = self.config.behavior.bold_is_bright;
        }

        if let Some(tw) = self.windows.get_mut(&window_id) {
            tw.add_tab(tab_id);
            tw.window.request_redraw();
        }

        log(&format!("new tab {tab_id:?} in window {window_id:?}"));
        Some(tab_id)
    }

    fn close_tab(&mut self, tab_id: TabId, _event_loop: &ActiveEventLoop) {
        // Remove the tab from its window first
        let mut empty_windows = Vec::new();
        for (wid, tw) in &mut self.windows {
            if tw.remove_tab(tab_id) {
                empty_windows.push(*wid);
            } else {
                tw.window.request_redraw();
            }
        }

        // If this leaves a window with no tabs AND it's the last one, force-exit
        // BEFORE dropping the Tab (ClosePseudoConsole would block).
        for wid in &empty_windows {
            if self.windows.len() <= 1 {
                self.exit_app();
            }
            self.windows.remove(wid);
        }

        // Safe to drop now — only reached for non-last windows
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.shutdown();
        }
        self.tabs.remove(&tab_id);
    }

    fn close_window(&mut self, window_id: WindowId, _event_loop: &ActiveEventLoop) {
        // If closing the settings window, just remove it — don't exit
        if self.settings_window == Some(window_id) {
            self.close_settings_window();
            return;
        }

        // Check if this is the last terminal window BEFORE dropping anything.
        let other_terminal_windows = self.windows.keys()
            .any(|wid| *wid != window_id && self.settings_window != Some(*wid));

        if !other_terminal_windows {
            // Last terminal window — force-exit before Tab drops block.
            self.exit_app();
        }

        // Non-last window: safe to do normal cleanup
        let tab_ids: Vec<TabId> = self
            .windows
            .get(&window_id)
            .map(|tw| tw.tabs.clone())
            .unwrap_or_default();
        for tid in &tab_ids {
            if let Some(tab) = self.tabs.get_mut(tid) {
                tab.shutdown();
            }
        }
        for tid in tab_ids {
            self.tabs.remove(&tid);
        }
        self.windows.remove(&window_id);
    }

    /// Shut everything down and exit the process immediately.
    /// `ClosePseudoConsole` (`ConPTY` cleanup) blocks indefinitely when the
    /// PTY reader thread still holds a pipe handle, so we kill children and
    /// force-exit before any `Tab` drop runs. Same approach as Alacritty.
    fn exit_app(&mut self) {
        for tab in self.tabs.values_mut() {
            tab.shutdown();
        }
        // Don't join threads — process::exit will clean them up.
        std::process::exit(0);
    }

    fn render_window(&mut self, window_id: WindowId) {
        // Settings window has its own renderer path
        if self.is_settings_window(window_id) {
            self.render_settings_window(window_id);
            return;
        }

        // Extract all info we need before borrowing mutably.
        let (phys, tab_info, active_idx, active_tab_id, is_maximized) = {
            let tw = match self.windows.get(&window_id) {
                Some(tw) => tw,
                None => return,
            };
            let phys = tw.window.inner_size();
            let tab_info: Vec<(TabId, String)> = tw
                .tabs
                .iter()
                .map(|id| {
                    let title = self
                        .tabs
                        .get(id).map_or_else(|| "?".to_string(), |t| t.title.clone());
                    (*id, title)
                })
                .collect();
            let active_idx = tw.active_tab;
            let active_tab_id = tw.active_tab_id();
            let is_maximized = tw.is_maximized;
            (phys, tab_info, active_idx, active_tab_id, is_maximized)
        };

        if phys.width == 0 || phys.height == 0 {
            return;
        }

        let hover = self.hover_hit.get(&window_id).copied().unwrap_or(TabBarHit::None);
        let dropdown_open = self.dropdown_open == Some(window_id);

        // Build FrameParams — need the active tab's grid
        let frame_params = active_tab_id
            .and_then(|tab_id| self.tabs.get(&tab_id))
            .map(|tab| FrameParams {
                width: phys.width,
                height: phys.height,
                grid: tab.grid(),
                palette: &tab.palette,
                mode: tab.mode,
                cursor_shape: tab.cursor_shape,
                selection: tab.selection.as_ref(),
                search: tab.search.as_ref(),
                tab_info: &tab_info,
                active_tab: active_idx,
                hover_hit: hover,
                is_maximized,
                dropdown_open,
                opacity: self.config.window.effective_opacity(),
                tab_bar_opacity: self.config.window.effective_tab_bar_opacity(),
                hover_hyperlink: self.hover_hyperlink.as_ref()
                    .filter(|(wid, _)| *wid == window_id)
                    .map(|(_, uri)| uri.as_str()),
                hover_url_range: self.hover_url_range.as_deref(),
            });

        let Some(frame_params) = frame_params else {
            return;
        };

        let gpu = match &self.gpu {
            Some(g) => g,
            None => return,
        };

        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };

        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };
        renderer.draw_frame(
            gpu,
            &tw.surface,
            &tw.surface_config,
            &frame_params,
            &mut self.glyphs,
        );
    }

    /// Detect if cursor is in the resize border zone. Returns the resize direction
    /// if so, or None if the cursor is in the client area.
    fn resize_direction_at(
        &self,
        window_id: WindowId,
        pos: PhysicalPosition<f64>,
    ) -> Option<ResizeDirection> {
        let tw = self.windows.get(&window_id)?;
        if tw.is_maximized {
            return None; // No resize when maximized
        }

        let size = tw.window.inner_size();
        let w = size.width as f64;
        let h = size.height as f64;
        let x = pos.x;
        let y = pos.y;

        let left = x < RESIZE_BORDER;
        let right = x >= w - RESIZE_BORDER;
        let top = y < RESIZE_BORDER;
        let bottom = y >= h - RESIZE_BORDER;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn handle_resize(&mut self, window_id: WindowId, width: u32, height: u32) {
        // Settings window doesn't have tabs to resize
        if self.is_settings_window(window_id) {
            if let (Some(tw), Some(gpu)) = (self.windows.get_mut(&window_id), self.gpu.as_ref()) {
                tw.resize_surface(&gpu.device, width, height);
            }
            return;
        }

        let (cols, rows) = self.grid_dims_for_size(width, height);
        log(&format!(
            "handle_resize: window={width}x{height} cell={}x{} cols={cols} rows={rows}",
            self.glyphs.cell_width, self.glyphs.cell_height
        ));

        // Resize the wgpu surface
        if let (Some(tw), Some(gpu)) = (self.windows.get_mut(&window_id), self.gpu.as_ref()) {
            tw.resize_surface(&gpu.device, width, height);
        }

        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let pixel_w = width as u16;
        let pixel_h = height as u16;

        for &tab_id in &tw.tabs {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.selection = None;
                tab.resize(cols, rows, pixel_w, pixel_h);
            }
        }
    }

    fn handle_scale_factor_changed(&mut self, window_id: WindowId, scale_factor: f64) {
        if (scale_factor - self.scale_factor).abs() < 0.01 {
            return;
        }
        log(&format!(
            "scale_factor changed: {:.2} -> {:.2}",
            self.scale_factor, scale_factor
        ));
        self.scale_factor = scale_factor;

        // Reload fonts at scaled size
        let scaled_size = self.config.font.size * scale_factor as f32;
        self.glyphs = FontSet::load(scaled_size, self.config.font.family.as_deref());
        log(&format!(
            "dpi reload: font size={}, cell={}x{}",
            self.glyphs.size, self.glyphs.cell_width, self.glyphs.cell_height
        ));

        // Rebuild atlas for new font size
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
            renderer.rebuild_atlas(gpu, &mut self.glyphs);
        }

        // Resize grids in all windows (winit sends Resized after ScaleFactorChanged,
        // so surface reconfiguration happens automatically)
        let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for wid in window_ids {
            if !self.is_settings_window(wid) {
                self.resize_all_tabs_in_window(wid);
            }
        }

        // Redraw the affected window
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    /// Convert pixel coordinates to grid cell (col, `viewport_line`).
    /// Returns None if outside the grid area.
    fn pixel_to_cell(&self, pos: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        let x = pos.x as usize;
        let y = pos.y as usize;
        let grid_top = TAB_BAR_HEIGHT + 10;
        if y < grid_top || x < GRID_PADDING_LEFT {
            return None;
        }
        let cw = self.glyphs.cell_width;
        let ch = self.glyphs.cell_height;
        if cw == 0 || ch == 0 {
            return None;
        }
        let col = (x - GRID_PADDING_LEFT) / cw;
        let line = (y - grid_top) / ch;
        Some((col, line))
    }

    /// Determine which side of the cell the cursor is on.
    fn pixel_to_side(&self, pos: PhysicalPosition<f64>) -> Side {
        let x = pos.x as usize;
        let cw = self.glyphs.cell_width;
        if cw == 0 {
            return Side::Left;
        }
        let cell_x = (x.saturating_sub(GRID_PADDING_LEFT)) % cw;
        if cell_x < cw / 2 { Side::Left } else { Side::Right }
    }

    /// Convert a viewport line to an absolute row index.
    fn viewport_to_absolute(grid: &crate::grid::Grid, line: usize) -> usize {
        grid.scrollback.len().saturating_sub(grid.display_offset) + line
    }

    /// Open a URL in the default browser. Only allows safe schemes.
    ///
    /// On Windows, uses `ShellExecuteW` directly (like Windows Terminal and
    /// `WezTerm`) instead of `cmd /C start` which mangles `&` and `%` in URLs.
    #[allow(unsafe_code)]
    fn open_url(uri: &str) {
        let allowed = uri.starts_with("http://")
            || uri.starts_with("https://")
            || uri.starts_with("ftp://")
            || uri.starts_with("file://");
        if !allowed {
            log(&format!("hyperlink: blocked URI with disallowed scheme: {uri}"));
            return;
        }
        log(&format!("hyperlink: opening ({} chars) {uri}", uri.len()));
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStrExt;
            let wide_open: Vec<u16> = std::ffi::OsStr::new("open")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_uri: Vec<u16> = std::ffi::OsStr::new(uri)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: ShellExecuteW is a standard Windows API call with
            // null-terminated wide strings. No memory safety concerns.
            unsafe {
                windows_sys::Win32::UI::Shell::ShellExecuteW(
                    std::ptr::null_mut(),
                    wide_open.as_ptr(),
                    wide_uri.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(uri)
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(uri)
                .spawn();
        }
    }

    /// Detect click count (1=char, 2=word, 3=line), cycling on rapid clicks.
    fn detect_click_count(&mut self, window_id: WindowId, col: usize, line: usize) -> u8 {
        let now = Instant::now();
        let same_pos = self.last_grid_click_pos == Some((col, line));
        let same_window = self.last_click_window == Some(window_id);
        let within_time = self.last_click_time
            .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS);

        let count = if same_pos && same_window && within_time {
            match self.click_count {
                1 => 2,
                2 => 3,
                _ => 1,
            }
        } else {
            1
        };

        self.last_click_time = Some(now);
        self.last_click_window = Some(window_id);
        self.last_grid_click_pos = Some((col, line));
        self.click_count = count;
        count
    }

    /// Encode and send a mouse report to the PTY.
    ///
    /// `button` is the base button code (0=left, 1=middle, 2=right, 3=release,
    /// 64=scroll-up, 65=scroll-down; add 32 for motion events).
    fn send_mouse_report(
        &mut self,
        tab_id: TabId,
        button: u8,
        col: usize,
        line: usize,
        pressed: bool,
    ) {
        let tab = match self.tabs.get_mut(&tab_id) {
            Some(t) => t,
            None => return,
        };

        // Add modifier bits
        let mut code = button;
        if self.modifiers.shift_key() {
            code += 4;
        }
        if self.modifiers.alt_key() {
            code += 8;
        }
        if self.modifiers.control_key() {
            code += 16;
        }

        if tab.mode.contains(TermMode::SGR_MOUSE) {
            // SGR encoding: CSI < code ; col+1 ; line+1 M/m
            let suffix = if pressed { 'M' } else { 'm' };
            let seq = format!("\x1b[<{code};{};{}{suffix}", col + 1, line + 1);
            tab.send_pty(seq.as_bytes());
        } else if tab.mode.contains(TermMode::UTF8_MOUSE) {
            // UTF-8 encoding: like normal but coordinates are UTF-8 encoded
            let encode_utf8 = |v: u32| -> Vec<u8> {
                let mut buf = [0u8; 4];
                let c = char::from_u32(v).unwrap_or(' ');
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            };
            let mut seq = vec![0x1b, b'[', b'M'];
            seq.extend_from_slice(&encode_utf8(u32::from(code) + 32));
            seq.extend_from_slice(&encode_utf8(col as u32 + 1 + 32));
            seq.extend_from_slice(&encode_utf8(line as u32 + 1 + 32));
            tab.send_pty(&seq);
        } else {
            // Normal encoding: ESC [ M Cb Cx Cy (clamp coords to 223 max)
            let cb = 32 + code;
            let cx = ((col + 1).min(223) + 32) as u8;
            let cy = ((line + 1).min(223) + 32) as u8;
            let seq = [0x1b, b'[', b'M', cb, cx, cy];
            tab.send_pty(&seq);
        }
    }

    fn handle_mouse_input(
        &mut self,
        window_id: WindowId,
        state: ElementState,
        button: MouseButton,
        event_loop: &ActiveEventLoop,
    ) {
        let pos = self
            .cursor_pos
            .get(&window_id)
            .copied()
            .unwrap_or(PhysicalPosition::new(0.0, 0.0));

        let x = pos.x as usize;
        let y = pos.y as usize;

        // Mouse reporting: if any mouse mode is active and Shift is NOT held,
        // report to PTY and skip normal handling (Shift overrides mouse reporting
        // so the user can still select text).
        if !self.modifiers.shift_key() && !self.is_settings_window(window_id) {
            let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
            if let Some(tid) = tab_id {
                let mouse_active = self.tabs.get(&tid)
                    .is_some_and(|t| t.mode.intersects(TermMode::ANY_MOUSE));
                if mouse_active {
                    if let Some((col, line)) = self.pixel_to_cell(pos) {
                        let btn_code = match button {
                            MouseButton::Left => 0u8,
                            MouseButton::Middle => 1,
                            MouseButton::Right => 2,
                            _ => return,
                        };
                        let pressed = state == ElementState::Pressed;
                        let report_code = if pressed { btn_code } else { 3 };
                        // Reset motion dedup on press/release
                        self.last_mouse_cell = if pressed { Some((col, line)) } else { None };
                        // Track left_mouse_down for motion reporting
                        if button == MouseButton::Left {
                            self.left_mouse_down = pressed;
                        }
                        self.send_mouse_report(tid, report_code, col, line, pressed);
                    }
                    return;
                }
            }
        }

        // Right-click: copy if selection exists, paste if not
        if button == MouseButton::Right {
            if state == ElementState::Released {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    let has_selection = self.tabs.get(&tid)
                        .is_some_and(|t| t.selection.is_some());
                    if has_selection {
                        // Right-click with selection: always copy (explicit action)
                        if let Some(tab) = self.tabs.get(&tid) {
                            if let Some(ref sel) = tab.selection {
                                let text = selection::extract_text(tab.grid(), sel);
                                if !text.is_empty() {
                                    clipboard::set_text(&text);
                                }
                            }
                        }
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.selection = None;
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    } else {
                        // No selection — paste
                        self.paste_from_clipboard(window_id);
                    }
                }
            }
            return;
        }

        if button != MouseButton::Left {
            return;
        }

        match state {
            ElementState::Pressed => {
                // If clicking in settings window, handle separately
                if self.is_settings_window(window_id) {
                    self.handle_settings_mouse(window_id, x, y);
                    return;
                }

                // If dropdown is open, handle menu click first
                if self.dropdown_open.is_some() && self.handle_dropdown_click(window_id, x, y, event_loop) {
                    return;
                }

                // Check resize borders first
                if let Some(direction) = self.resize_direction_at(window_id, pos) {
                    if let Some(tw) = self.windows.get(&window_id) {
                        let _ = tw.window.drag_resize_window(direction);
                    }
                    return;
                }

                if y < TAB_BAR_HEIGHT {
                    let tw = match self.windows.get(&window_id) {
                        Some(tw) => tw,
                        None => return,
                    };
                    let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
                    let hit = layout.hit_test(x, y);

                    match hit {
                        TabBarHit::NewTab => {
                            self.new_tab_in_window(window_id);
                        }
                        TabBarHit::DropdownButton => {
                            if self.dropdown_open == Some(window_id) {
                                self.dropdown_open = None;
                            } else {
                                self.dropdown_open = Some(window_id);
                            }
                            if let Some(tw) = self.windows.get(&window_id) {
                                tw.window.request_redraw();
                            }
                        }
                        TabBarHit::CloseTab(idx) => {
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                self.close_tab(tab_id, event_loop);
                            }
                        }
                        TabBarHit::Minimize => {
                            if let Some(tw) = self.windows.get(&window_id) {
                                tw.window.set_minimized(true);
                            }
                        }
                        TabBarHit::Maximize => {
                            if let Some(tw) = self.windows.get_mut(&window_id) {
                                let new_max = !tw.is_maximized;
                                tw.is_maximized = new_max;
                                tw.window.set_maximized(new_max);
                                tw.window.request_redraw();
                            }
                        }
                        TabBarHit::CloseWindow => {
                            self.close_window(window_id, event_loop);
                        }
                        TabBarHit::DragArea => {
                            // Check for double-click to toggle maximize
                            let now = Instant::now();
                            let is_double = self.last_click_time
                                .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS)
                                && self.last_click_window == Some(window_id);

                            if is_double {
                                // Double-click: toggle maximize
                                self.last_click_time = None;
                                self.last_click_window = None;
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    let new_max = !tw.is_maximized;
                                    tw.is_maximized = new_max;
                                    tw.window.set_maximized(new_max);
                                    tw.window.request_redraw();
                                }
                            } else {
                                // Single click: start window drag
                                self.last_click_time = Some(now);
                                self.last_click_window = Some(window_id);
                                if let Some(tw) = self.windows.get(&window_id) {
                                    let _ = tw.window.drag_window();
                                }
                            }
                        }
                        TabBarHit::Tab(idx) => {
                            // Start potential drag
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                self.drag = Some(DragState::new(tab_id, window_id, pos, idx));
                                // Also select this tab
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    tw.active_tab = idx;
                                    tw.window.request_redraw();
                                }
                            }
                        }
                        TabBarHit::None => {}
                    }
                } else {
                    // Click in grid area — handle selection
                    self.handle_grid_press(window_id, pos);
                }
            }
            ElementState::Released => {
                // Finalize selection and auto-copy
                if self.left_mouse_down {
                    self.left_mouse_down = false;
                    let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            if tab.selection.as_ref().is_some_and(Selection::is_empty) {
                                tab.selection = None;
                            } else if self.config.behavior.copy_on_select {
                                if let Some(ref sel) = tab.selection {
                                    let text = selection::extract_text(tab.grid(), sel);
                                    if !text.is_empty() {
                                        clipboard::set_text(&text);
                                    }
                                }
                            } else {
                                // copy_on_select disabled — keep selection visible
                            }
                        }
                    }
                }

                if let Some(drag) = self.drag.take() {
                    // drag.source_window was updated to the torn-off window
                    // during tear_off_tab — use it, not window_id
                    let torn_wid = drag.source_window;
                    match drag.phase {
                        DragPhase::TornOff => {
                            // Check if we're over another window's tab bar
                            if let Some(target_wid) = self.find_window_at_cursor(torn_wid) {
                                self.reattach_tab(drag.tab_id, torn_wid, target_wid, pos);
                            }
                            // Window is already created — nothing else needed
                        }
                        DragPhase::DraggingInBar | DragPhase::Pending => {
                            // Tab reorder finalized / was just a click — nothing extra needed
                        }
                    }
                }
            }
        }
    }

    fn handle_grid_press(&mut self, window_id: WindowId, pos: PhysicalPosition<f64>) {
        let (col, line) = match self.pixel_to_cell(pos) {
            Some(c) => c,
            None => return,
        };
        let side = self.pixel_to_side(pos);

        let tab_id = match self.windows.get(&window_id).and_then(TermWindow::active_tab_id) {
            Some(id) => id,
            None => return,
        };

        // Clamp col/line to grid bounds
        let (grid_cols, grid_lines) = match self.tabs.get(&tab_id) {
            Some(tab) => (tab.grid().cols, tab.grid().lines),
            None => return,
        };
        let col = col.min(grid_cols.saturating_sub(1));
        let line = line.min(grid_lines.saturating_sub(1));

        let abs_row = match self.tabs.get(&tab_id) {
            Some(tab) => Self::viewport_to_absolute(tab.grid(), line),
            None => return,
        };

        // Ctrl+click: open hyperlink URL (OSC 8 first, then implicit URL)
        if self.modifiers.control_key() {
            let uri: Option<String> = self.tabs.get(&tab_id).and_then(|tab| {
                let row = tab.grid().absolute_row(abs_row)?;
                if col >= row.len() { return None; }
                row[col].hyperlink().map(|h| h.uri.clone())
            });
            if let Some(ref uri) = uri {
                Self::open_url(uri);
                return;
            }
            // Fall through to implicit URL detection
            let implicit_url: Option<String> = self.tabs.get(&tab_id).and_then(|tab| {
                let grid = tab.grid();
                let hit = self.url_cache.url_at(grid, abs_row, col)?;
                Some(hit.url)
            });
            if let Some(ref url) = implicit_url {
                Self::open_url(url);
                return;
            }
        }

        let click_count = self.detect_click_count(window_id, col, line);
        let shift = self.modifiers.shift_key();
        let alt = self.modifiers.alt_key();

        // Shift+click: extend existing selection
        if shift {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                if tab.selection.is_some() {
                    if let Some(ref mut sel) = tab.selection {
                        sel.end = SelectionPoint { row: abs_row, col, side };
                    }
                    self.left_mouse_down = true;
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                    return;
                }
            }
        }

        // Create new selection based on click count
        let new_selection = match click_count {
            2 => {
                // Double-click: word selection
                if let Some(tab) = self.tabs.get(&tab_id) {
                    let (word_start, word_end) = selection::word_boundaries(tab.grid(), abs_row, col);
                    let anchor = SelectionPoint { row: abs_row, col: word_start, side: Side::Left };
                    let pivot = SelectionPoint { row: abs_row, col: word_end, side: Side::Right };
                    Some(Selection::new_word(anchor, pivot))
                } else {
                    None
                }
            }
            3 => {
                // Triple-click: line selection
                if let Some(tab) = self.tabs.get(&tab_id) {
                    let line_start_row = selection::logical_line_start(tab.grid(), abs_row);
                    let line_end_row = selection::logical_line_end(tab.grid(), abs_row);
                    let anchor = SelectionPoint { row: line_start_row, col: 0, side: Side::Left };
                    let pivot = SelectionPoint {
                        row: line_end_row,
                        col: grid_cols.saturating_sub(1),
                        side: Side::Right,
                    };
                    Some(Selection::new_line(anchor, pivot))
                } else {
                    None
                }
            }
            _ => {
                // Single click: char selection (or block if Alt held)
                let mut sel = Selection::new_char(abs_row, col, side);
                if alt {
                    sel.mode = SelectionMode::Block;
                }
                Some(sel)
            }
        };

        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.selection = new_selection;
        }
        self.left_mouse_down = true;

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    fn paste_from_clipboard(&mut self, window_id: WindowId) {
        let tab_id = match self.windows.get(&window_id).and_then(TermWindow::active_tab_id) {
            Some(id) => id,
            None => return,
        };
        if let Some(text) = clipboard::get_text() {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                if tab.mode.contains(TermMode::BRACKETED_PASTE) {
                    tab.send_pty(b"\x1b[200~");
                    tab.send_pty(text.as_bytes());
                    tab.send_pty(b"\x1b[201~");
                } else {
                    tab.send_pty(text.as_bytes());
                }
            }
        }
    }

    fn handle_mouse_wheel(&mut self, window_id: WindowId, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                if y > 0.0 { SCROLL_LINES as i32 } else { -(SCROLL_LINES as i32) }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                let cell_h = self.glyphs.cell_height as f64;
                if cell_h > 0.0 {
                    (pos.y / cell_h).round() as i32
                } else {
                    0
                }
            }
        };

        if lines == 0 {
            return;
        }

        let tab_id = self.windows.get(&window_id)
            .and_then(TermWindow::active_tab_id);
        let Some(tid) = tab_id else { return };

        // Mouse reporting: scroll events sent to PTY when mouse mode active
        if !self.modifiers.shift_key() {
            let mouse_active = self.tabs.get(&tid)
                .is_some_and(|t| t.mode.intersects(TermMode::ANY_MOUSE));
            if mouse_active {
                let pos = self.cursor_pos.get(&window_id)
                    .copied()
                    .unwrap_or(PhysicalPosition::new(0.0, 0.0));
                if let Some((col, line)) = self.pixel_to_cell(pos) {
                    let btn = if lines > 0 { 64u8 } else { 65 };
                    let count = lines.unsigned_abs() as usize;
                    for _ in 0..count {
                        self.send_mouse_report(tid, btn, col, line, true);
                    }
                }
                return;
            }
        }

        // Alternate scroll: convert scroll to arrow keys in alt screen
        if !self.modifiers.shift_key() {
            let alt_scroll = self.tabs.get(&tid).is_some_and(|t| {
                t.mode.contains(TermMode::ALT_SCREEN)
                    && t.mode.contains(TermMode::ALTERNATE_SCROLL)
            });
            if alt_scroll {
                let app_cursor = self.tabs.get(&tid)
                    .is_some_and(|t| t.mode.contains(TermMode::APP_CURSOR));
                let (up, down) = if app_cursor {
                    (b"\x1bOA" as &[u8], b"\x1bOB" as &[u8])
                } else {
                    (b"\x1b[A" as &[u8], b"\x1b[B" as &[u8])
                };
                let seq = if lines > 0 { up } else { down };
                let count = lines.unsigned_abs() as usize;
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    for _ in 0..count {
                        tab.send_pty(seq);
                    }
                }
                return;
            }
        }

        // Normal scrollback
        if let Some(tab) = self.tabs.get_mut(&tid) {
            let grid = tab.grid_mut();
            if lines > 0 {
                // Scroll up (into history)
                let max = grid.scrollback.len();
                grid.display_offset = (grid.display_offset + lines as usize).min(max);
            } else {
                // Scroll down (toward live)
                grid.display_offset = grid.display_offset.saturating_sub((-lines) as usize);
            }
        }
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    fn handle_cursor_moved(&mut self, window_id: WindowId, position: PhysicalPosition<f64>, event_loop: &ActiveEventLoop) {
        self.cursor_pos.insert(window_id, position);

        // Update cursor icon for resize borders and hyperlink hover
        let cursor_icon = if let Some(dir) = self.resize_direction_at(window_id, position) {
            dir.into()
        } else {
            CursorIcon::Default
        };

        // Hyperlink hover: detect when Ctrl is held and mouse is over a hyperlinked cell
        let (cursor_icon, new_hover, new_url_range) = if cursor_icon == CursorIcon::Default
            && self.modifiers.control_key()
            && !self.is_settings_window(window_id)
        {
            if let Some((col, line)) = self.pixel_to_cell(position) {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                // Check OSC 8 hyperlink first
                let osc8_uri: Option<String> = tab_id.and_then(|tid| {
                    let tab = self.tabs.get(&tid)?;
                    let grid = tab.grid();
                    let abs_row = Self::viewport_to_absolute(grid, line);
                    let row = grid.absolute_row(abs_row)?;
                    if col >= row.len() { return None; }
                    row[col].hyperlink().map(|h| h.uri.clone())
                });
                if let Some(ref uri) = osc8_uri {
                    (CursorIcon::Pointer, Some((window_id, uri.clone())), None)
                } else {
                    // Fall through to implicit URL detection
                    let implicit = tab_id.and_then(|tid| {
                        let tab = self.tabs.get(&tid)?;
                        let grid = tab.grid();
                        let abs_row = Self::viewport_to_absolute(grid, line);
                        let row = grid.absolute_row(abs_row)?;
                        if col >= row.len() {
                            return None;
                        }
                        let hit = self.url_cache.url_at(grid, abs_row, col)?;
                        Some((hit.url, hit.segments))
                    });
                    if let Some((url, segments)) = implicit {
                        (CursorIcon::Pointer, Some((window_id, url)), Some(segments))
                    } else {
                        (cursor_icon, None, None)
                    }
                }
            } else {
                (cursor_icon, None, None)
            }
        } else {
            (cursor_icon, None, None)
        };

        let hover_changed = self.hover_hyperlink != new_hover
            || self.hover_url_range != new_url_range;
        self.hover_hyperlink = new_hover;
        self.hover_url_range = new_url_range;

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.set_cursor(cursor_icon);
            if hover_changed {
                tw.window.request_redraw();
            }
        }

        // Mouse motion reporting (before selection drag).
        // When mouse reporting is active, motion in the grid is sent to PTY
        // and selection drag is suppressed. Tab bar hover and drag still work.
        let mut mouse_motion_reported = false;
        if !self.is_settings_window(window_id) && !self.modifiers.shift_key() {
            let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
            if let Some(tid) = tab_id {
                let report_all = self.tabs.get(&tid)
                    .is_some_and(|t| t.mode.contains(TermMode::MOUSE_ALL));
                let report_motion = self.tabs.get(&tid)
                    .is_some_and(|t| t.mode.contains(TermMode::MOUSE_MOTION));

                if report_all || (report_motion && self.left_mouse_down) {
                    if let Some((col, line)) = self.pixel_to_cell(position) {
                        let cell = (col, line);
                        if self.last_mouse_cell != Some(cell) {
                            self.last_mouse_cell = Some(cell);
                            // Motion code: 32 + button (32 for left drag, 35 for no button)
                            let code = if self.left_mouse_down { 32 } else { 35 };
                            self.send_mouse_report(tid, code, col, line, true);
                        }
                        mouse_motion_reported = true;
                    }
                }
            }
        }

        // Selection drag: update selection end point (skip when mouse reporting handled motion)
        if self.left_mouse_down && !mouse_motion_reported {
            if let Some((col, line)) = self.pixel_to_cell(position) {
                let side = self.pixel_to_side(position);
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid_cols = tab.grid().cols;
                        let grid_lines = tab.grid().lines;
                        let col = col.min(grid_cols.saturating_sub(1));
                        let line = line.min(grid_lines.saturating_sub(1));
                        let abs_row = Self::viewport_to_absolute(tab.grid(), line);

                        // Compute new end point based on selection mode.
                        // Pre-compute boundary data from grid before mutating selection.
                        let sel_mode = tab.selection.as_ref().map(|s| s.mode);
                        let sel_anchor_row = tab.selection.as_ref().map(|s| s.anchor.row);
                        let new_end = match sel_mode {
                            Some(SelectionMode::Word) => {
                                let (w_start, w_end) = selection::word_boundaries(tab.grid(), abs_row, col);
                                let anchor = tab.selection.as_ref().map(|s| s.anchor);
                                let start_pt = SelectionPoint { row: abs_row, col: w_start, side: Side::Left };
                                let end_pt = SelectionPoint { row: abs_row, col: w_end, side: Side::Right };
                                if anchor.is_some_and(|a| start_pt < a) {
                                    Some(start_pt)
                                } else {
                                    Some(end_pt)
                                }
                            }
                            Some(SelectionMode::Line) => {
                                let drag_line_start = selection::logical_line_start(tab.grid(), abs_row);
                                let drag_line_end = selection::logical_line_end(tab.grid(), abs_row);
                                if sel_anchor_row.is_some_and(|ar| abs_row < ar) {
                                    Some(SelectionPoint { row: drag_line_start, col: 0, side: Side::Left })
                                } else {
                                    Some(SelectionPoint {
                                        row: drag_line_end,
                                        col: grid_cols.saturating_sub(1),
                                        side: Side::Right,
                                    })
                                }
                            }
                            Some(_) => Some(SelectionPoint { row: abs_row, col, side }),
                            None => None,
                        };

                        if let (Some(new_end), Some(sel)) = (new_end, &mut tab.selection) {
                            sel.end = new_end;
                        }
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            } else {
                // Mouse outside grid — auto-scroll
                let y = position.y as usize;
                let grid_top = TAB_BAR_HEIGHT + 10;
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if y < grid_top {
                        // Above grid: scroll up into history
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            let grid = tab.grid_mut();
                            let max = grid.scrollback.len();
                            grid.display_offset = (grid.display_offset + 1).min(max);
                        }
                    }
                    // Below grid: scroll down toward live
                    // (Only if display_offset > 0)
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let ch = self.glyphs.cell_height;
                        let grid_bottom = grid_top + tab.grid().lines * ch;
                        if y >= grid_bottom && tab.grid().display_offset > 0 {
                            tab.grid_mut().display_offset = tab.grid().display_offset.saturating_sub(1);
                        }
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
        }

        // Update hover state for tab bar
        let y = position.y as usize;
        let x = position.x as usize;

        if y < TAB_BAR_HEIGHT {
            if let Some(tw) = self.windows.get(&window_id) {
                let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
                let hit = layout.hit_test(x, y);
                let prev = self.hover_hit.insert(window_id, hit);
                if prev != Some(hit) {
                    tw.window.request_redraw();
                }
            }
        } else {
            let prev = self.hover_hit.insert(window_id, TabBarHit::None);
            if prev != Some(TabBarHit::None) {
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
        }

        // Handle drag — extract values to avoid borrow conflicts with self
        let drag_action = self.drag.as_ref().map(|drag| {
            (drag.phase, drag.tab_id, drag.source_window, drag.grab_offset,
             drag.distance_from_origin(position), drag.vertical_distance(position))
        });

        if let Some((phase, tab_id, source_wid, grab_offset, dist, vert_dist)) = drag_action {
            match phase {
                DragPhase::Pending => {
                    if dist >= DRAG_START_THRESHOLD {
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::DraggingInBar;
                        }
                        log("drag: pending -> dragging in bar");
                    }
                }
                DragPhase::DraggingInBar => {
                    if vert_dist >= TEAR_OFF_THRESHOLD {
                        log("drag: tearing off!");
                        self.tear_off_tab(tab_id, source_wid, position, event_loop);
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::TornOff;
                        }
                    } else {
                        self.reorder_tab_in_bar(window_id, position);
                    }
                }
                DragPhase::TornOff => {
                    // Convert cursor to screen coordinates using the window
                    // that actually sent this CursorMoved event.
                    let screen_cursor = self.windows.get(&window_id)
                        .and_then(|tw| tw.window.inner_position().ok())
                        .map(|ip| (ip.x as f64 + position.x, ip.y as f64 + position.y));

                    if let Some((sx, sy)) = screen_cursor {
                        // Position torn-off window so cursor stays at grab_offset
                        let torn_wid = self.window_containing_tab(tab_id);
                        if let Some(wid) = torn_wid {
                            if let Some(tw) = self.windows.get(&wid) {
                                let new_x = sx - grab_offset.x;
                                let new_y = sy - grab_offset.y;
                                tw.window.set_outer_position(
                                    PhysicalPosition::new(new_x as i32, new_y as i32),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn tear_off_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        cursor: PhysicalPosition<f64>,
        event_loop: &ActiveEventLoop,
    ) {
        // Remove tab from source window
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
            tw.window.request_redraw();
        }

        // Compute screen-space cursor position from source window
        let screen_cursor = self
            .windows
            .get(&source_wid)
            .and_then(|tw| tw.window.outer_position().ok())
            .map(|wp| (wp.x + cursor.x as i32, wp.y + cursor.y as i32));

        // The grab offset: where the cursor will be within the new window.
        // Put it at a comfortable spot — center-ish of a tab, vertically in tab bar.
        let grab_x = 75.0; // roughly half a tab width
        let grab_y = (TAB_BAR_HEIGHT / 2) as f64;

        // Create new frameless window at cursor position
        if let Some(new_wid) = self.create_window(event_loop) {
            if let Some(tw) = self.windows.get_mut(&new_wid) {
                tw.add_tab(tab_id);
                // Position so cursor is at grab_offset within the client area.
                // No title bar offset needed — frameless window has no OS decoration.
                if let Some((sx, sy)) = screen_cursor {
                    let win_x = sx - grab_x as i32;
                    let win_y = sy - grab_y as i32;
                    tw.window
                        .set_outer_position(PhysicalPosition::new(win_x, win_y));
                }
                tw.window.request_redraw();

                // Hand off to the OS native move loop. This is blocking —
                // Windows handles the drag with full Aero Snap support.
                // When the user releases the mouse, this returns.
                let _ = tw.window.drag_window();
            }

            // Native drag finished (mouse released). Clear our drag state.
            self.drag = None;
        }

        // If source window is empty, close it
        let source_empty = self
            .windows
            .get(&source_wid)
            .is_some_and(|tw| tw.tabs.is_empty());
        if source_empty {
            self.windows.remove(&source_wid);
        }
    }

    fn reattach_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        target_wid: WindowId,
        _cursor: PhysicalPosition<f64>,
    ) {
        // Remove from source window
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
        }

        // Add to target window
        if let Some(tw) = self.windows.get_mut(&target_wid) {
            tw.add_tab(tab_id);
            tw.window.request_redraw();
        }

        // Close empty source window
        let source_empty = self
            .windows
            .get(&source_wid)
            .is_some_and(|tw| tw.tabs.is_empty());
        if source_empty {
            self.windows.remove(&source_wid);
        }
    }

    fn reorder_tab_in_bar(&mut self, window_id: WindowId, position: PhysicalPosition<f64>) {
        let drag = match &self.drag {
            Some(d) => d,
            None => return,
        };
        let tab_id = drag.tab_id;

        let tw = match self.windows.get_mut(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
        let new_idx = (position.x as usize / layout.tab_width).min(tw.tabs.len().saturating_sub(1));

        if let Some(current_idx) = tw.tab_index(tab_id) {
            if current_idx != new_idx {
                tw.tabs.remove(current_idx);
                tw.tabs.insert(new_idx, tab_id);
                tw.active_tab = new_idx;
                tw.window.request_redraw();
            }
        }
    }

    fn window_containing_tab(&self, tab_id: TabId) -> Option<WindowId> {
        for (wid, tw) in &self.windows {
            if tw.tabs.contains(&tab_id) {
                return Some(*wid);
            }
        }
        None
    }

    fn find_window_at_cursor(&self, _exclude: WindowId) -> Option<WindowId> {
        // For Phase 4: check if cursor is in another window's tab bar.
        // Simplified for now — would need screen coordinates.
        None
    }

    // --- Search helpers ---

    fn open_search(&mut self, window_id: WindowId) {
        let tab_id = match self.windows.get(&window_id).and_then(TermWindow::active_tab_id) {
            Some(id) => id,
            None => return,
        };
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.search = Some(SearchState::new());
        }
        self.search_active = Some(window_id);
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    fn close_search(&mut self, window_id: WindowId) {
        let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
        if let Some(tid) = tab_id {
            if let Some(tab) = self.tabs.get_mut(&tid) {
                tab.search = None;
            }
        }
        self.search_active = None;
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    fn handle_search_key(&mut self, window_id: WindowId, event: &winit::event::KeyEvent) {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.close_search(window_id);
            }
            Key::Named(NamedKey::Enter) => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            if self.modifiers.shift_key() {
                                search.prev_match();
                            } else {
                                search.next_match();
                            }
                        }
                    }
                    self.scroll_to_search_match(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            Key::Named(NamedKey::Backspace) => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            search.query.pop();
                        }
                    }
                    self.update_search(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            Key::Character(c) => {
                let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    let text = c.as_str().to_owned();
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            search.query.push_str(&text);
                        }
                    }
                    self.update_search(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn update_search(&mut self, tab_id: TabId) {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            // Take search out temporarily to avoid borrow conflict with grid
            if let Some(mut search) = tab.search.take() {
                search.update_query(tab.grid());
                tab.search = Some(search);
            }
        }
        self.scroll_to_search_match(tab_id);
    }

    fn scroll_to_search_match(&mut self, tab_id: TabId) {
        // Read the focused match position first
        let match_row = self.tabs.get(&tab_id).and_then(|tab| {
            tab.search.as_ref()?.focused_match().map(|m| m.start_row)
        });

        if let Some(target_row) = match_row {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                let grid = tab.grid_mut();
                let sb_len = grid.scrollback.len();
                let lines = grid.lines;

                // Check if target_row is visible in the current viewport
                let viewport_start = sb_len.saturating_sub(grid.display_offset);
                let viewport_end = viewport_start + lines;

                if target_row < viewport_start || target_row >= viewport_end {
                    // Scroll so the match is roughly centered in the viewport
                    let center_offset = sb_len.saturating_sub(target_row).saturating_sub(lines / 2);
                    grid.display_offset = center_offset.min(sb_len);
                }
            }
        }
    }

    // --- Dropdown menu helpers ---

    /// Compute the dropdown menu rectangle (x, y, w, h) in the given window.
    fn dropdown_menu_rect(&self, window_id: WindowId) -> Option<(usize, usize, usize, usize)> {
        let tw = self.windows.get(&window_id)?;
        let tab_count = tw.tabs.len();
        let bar_w = tw.window.inner_size().width as usize;
        let layout = TabBarLayout::compute(tab_count, bar_w);
        let tabs_end = TAB_LEFT_MARGIN + tab_count * layout.tab_width;
        let dropdown_x = tabs_end + NEW_TAB_BUTTON_WIDTH;
        let menu_w: usize = 140;
        let menu_h: usize = 32;
        let menu_x = dropdown_x;
        let menu_y = TAB_BAR_HEIGHT;
        Some((menu_x, menu_y, menu_w, menu_h))
    }

    /// Handle a click when the dropdown menu is open.
    /// Returns true if the click was consumed (inside menu or dismiss).
    fn handle_dropdown_click(
        &mut self,
        window_id: WindowId,
        x: usize,
        y: usize,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let dropdown_wid = match self.dropdown_open {
            Some(wid) => wid,
            None => return false,
        };

        // Only handle clicks in the window that has the dropdown open
        if window_id != dropdown_wid {
            self.dropdown_open = None;
            if let Some(tw) = self.windows.get(&dropdown_wid) {
                tw.window.request_redraw();
            }
            return false;
        }

        if let Some((mx, my, mw, mh)) = self.dropdown_menu_rect(window_id) {
            if x >= mx && x < mx + mw && y >= my && y < my + mh {
                // Clicked on "Settings" menu item
                self.dropdown_open = None;
                self.open_settings_window(event_loop);
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
                return true;
            }
        }

        // Clicked outside menu — dismiss
        self.dropdown_open = None;
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
        // Don't consume if the click is on tab bar buttons — let it propagate
        y >= TAB_BAR_HEIGHT
    }

    // --- Settings window ---

    fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        // If already open, focus it
        if let Some(wid) = self.settings_window {
            if let Some(tw) = self.windows.get(&wid) {
                tw.window.focus_window();
                return;
            }
            // Window was closed externally
            self.settings_window = None;
        }

        let win_w: u32 = 300;
        let win_h: u32 = 350;

        let attrs = Window::default_attributes()
            .with_title("Settings")
            .with_inner_size(winit::dpi::PhysicalSize::new(win_w, win_h))
            .with_decorations(false)
            .with_resizable(false);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("failed to create settings window: {e}"));
                return;
            }
        };

        let gpu = match &self.gpu {
            Some(g) => g,
            None => return,
        };

        let id = window.id();

        let Some(tw) = TermWindow::new(window.clone(), gpu) else {
            log("failed to create settings window surface");
            return;
        };

        // Render a dark clear frame (settings window is always opaque)
        if let Some(renderer) = &self.renderer {
            let s = crate::gpu::renderer::srgb_to_linear;
            let bg = [s(0x1e as f32 / 255.0), s(0x1e as f32 / 255.0), s(0x2e as f32 / 255.0), 1.0];
            renderer.clear_surface(gpu, &tw.surface, bg, 1.0);
        }

        window.set_visible(true);
        self.windows.insert(id, tw);
        self.settings_window = Some(id);
        log(&format!("created settings window {id:?}"));
    }

    fn close_settings_window(&mut self) {
        if let Some(wid) = self.settings_window.take() {
            self.windows.remove(&wid);
        }
    }

    fn is_settings_window(&self, window_id: WindowId) -> bool {
        self.settings_window == Some(window_id)
    }

    fn render_settings_window(&mut self, window_id: WindowId) {
        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let phys = tw.window.inner_size();
        if phys.width == 0 || phys.height == 0 {
            return;
        }

        let gpu = match &self.gpu {
            Some(g) => g,
            None => return,
        };

        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };

        // Get palette from any tab (for color derivation)
        let palette = self.tabs.values().next().map(|t| &t.palette);

        renderer.draw_settings_frame(
            gpu,
            &tw.surface,
            &tw.surface_config,
            phys.width,
            phys.height,
            self.active_scheme,
            palette,
            &mut self.glyphs,
        );
    }

    fn handle_settings_mouse(&mut self, window_id: WindowId, x: usize, y: usize) {
        // Close button (top-right 30x30) — check first
        let w = self.windows.get(&window_id)
            .map_or(0, |tw| tw.window.inner_size().width as usize);
        if x >= w.saturating_sub(30) && y < 30 {
            self.close_settings_window();
            return;
        }

        let title_h: usize = 50;
        let row_h: usize = 40;

        if y < title_h {
            return;
        }

        let row_idx = (y - title_h) / row_h;
        if row_idx < BUILTIN_SCHEMES.len() {
            let scheme = BUILTIN_SCHEMES[row_idx];
            self.apply_scheme_to_all_tabs(scheme);
        }
    }

    fn apply_scheme_to_all_tabs(&mut self, scheme: &'static ColorScheme) {
        self.active_scheme = scheme.name;
        for tab in self.tabs.values_mut() {
            tab.palette.set_scheme(scheme);
        }
        // Persist the scheme change
        scheme.name.clone_into(&mut self.config.colors.scheme);
        self.config.save();
        // Redraw all windows
        for tw in self.windows.values() {
            tw.window.request_redraw();
        }
    }

    fn apply_config_reload(&mut self) {
        let new_config = match Config::try_load() {
            Ok(c) => c,
            Err(e) => {
                log(&format!("config reload: {e}"));
                return;
            }
        };

        // Color scheme
        if new_config.colors.scheme != self.config.colors.scheme {
            if let Some(scheme) = palette::find_scheme(&new_config.colors.scheme) {
                self.active_scheme = scheme.name;
                for tab in self.tabs.values_mut() {
                    tab.palette.set_scheme(scheme);
                }
            }
        }

        // Font size or family change
        let font_changed = (new_config.font.size - self.config.font.size).abs() > f32::EPSILON
            || new_config.font.family != self.config.font.family;
        if font_changed {
            let scaled_size = new_config.font.size * self.scale_factor as f32;
            self.glyphs = FontSet::load(scaled_size, new_config.font.family.as_deref());
            log(&format!(
                "config reload: font size={}, cell={}x{}",
                self.glyphs.size, self.glyphs.cell_width, self.glyphs.cell_height
            ));
            if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
                renderer.rebuild_atlas(gpu, &mut self.glyphs);
            }
            let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
            for wid in window_ids {
                if !self.is_settings_window(wid) {
                    self.resize_all_tabs_in_window(wid);
                }
            }
        }

        // Cursor style
        let new_cursor = config::parse_cursor_style(&new_config.terminal.cursor_style);
        if new_config.terminal.cursor_style != self.config.terminal.cursor_style {
            for tab in self.tabs.values_mut() {
                tab.cursor_shape = new_cursor;
            }
        }

        // Bold is bright
        if new_config.behavior.bold_is_bright != self.config.behavior.bold_is_bright {
            for tab in self.tabs.values_mut() {
                tab.palette.bold_is_bright = new_config.behavior.bold_is_bright;
            }
        }

        // Keybindings
        self.bindings = keybindings::merge_bindings(&new_config.keybind);

        self.config = new_config;

        // Redraw all windows
        for tw in self.windows.values() {
            tw.window.request_redraw();
        }
        log("config reload: applied successfully");
    }
}

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.first_window_created {
            return;
        }
        self.first_window_created = true;

        if let Some(wid) = self.create_window(event_loop) {
            // Query actual DPI scale factor and reload fonts if needed
            if let Some(tw) = self.windows.get(&wid) {
                let sf = tw.window.scale_factor();
                if (sf - self.scale_factor).abs() > 0.01 {
                    self.handle_scale_factor_changed(wid, sf);
                }
            }
            self.new_tab_in_window(wid);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::PtyOutput(tab_id, data) => {
                log(&format!("pty_output: tab={tab_id:?} len={}", data.len()));

                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    tab.process_output(&data);
                }
                self.url_cache.invalidate();
                // Redraw the window containing this tab
                if let Some(wid) = self.window_containing_tab(tab_id) {
                    // Only redraw if this is the active tab in that window
                    if let Some(tw) = self.windows.get(&wid) {
                        if tw.active_tab_id() == Some(tab_id) {
                            tw.window.request_redraw();
                        }
                    }
                }
            }
            TermEvent::ConfigReload => {
                self.apply_config_reload();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }

            WindowEvent::RedrawRequested => {
                self.render_window(window_id);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(window_id, size.width, size.height);
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.handle_scale_factor_changed(window_id, scale_factor);
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(window_id, state, button, event_loop);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(window_id, delta);
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(window_id, position, event_loop);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Allow key release through only when Kitty REPORT_EVENT_TYPES is active.
                if event.state != ElementState::Pressed {
                    let has_kitty_events = self.windows.get(&window_id)
                        .and_then(TermWindow::active_tab_id)
                        .and_then(|tid| self.tabs.get(&tid))
                        .is_some_and(|tab| tab.mode.contains(TermMode::REPORT_EVENT_TYPES));
                    if !has_kitty_events {
                        return;
                    }
                }

                let is_pressed = event.state == ElementState::Pressed;

                // Settings window: Escape closes it, all other keys ignored
                if is_pressed && self.is_settings_window(window_id) {
                    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                        self.close_settings_window();
                    }
                    return;
                }

                // Search mode: intercept all keys when search is active
                if self.search_active == Some(window_id) {
                    if is_pressed {
                        self.handle_search_key(window_id, &event);
                    }
                    return;
                }

                // Escape: close dropdown if open
                if is_pressed && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    if self.dropdown_open.is_some() {
                        let wid = self.dropdown_open.take();
                        if let Some(wid) = wid {
                            if let Some(tw) = self.windows.get(&wid) {
                                tw.window.request_redraw();
                            }
                        }
                        return;
                    }
                }

                // Handle Escape during drag
                if is_pressed && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    if self.drag.is_some() {
                        // Cancel drag — revert to original state
                        self.drag = None;
                        // Redraw all windows
                        for tw in self.windows.values() {
                            tw.window.request_redraw();
                        }
                        return;
                    }
                }

                // Keybinding lookup
                let mods = build_modifiers(self.modifiers);
                if let Some(binding_key) = keybindings::key_to_binding_key(&event.logical_key) {
                    if let Some(action) = keybindings::find_binding(&self.bindings, &binding_key, mods) {
                        let action = action.clone();
                        if is_pressed {
                            if self.execute_action(&action, window_id, event_loop) {
                                return;
                            }
                            // SmartCopy with no selection — fall through to PTY
                        } else {
                            return; // matched binding on key release — consume
                        }
                    }
                }

                // Any keyboard input to PTY — scroll to live and clear selection
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    // Scroll to live on press (not release).
                    if is_pressed {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.grid_mut().display_offset = 0;
                            if tab.selection.is_some() {
                                tab.selection = None;
                            }
                        }
                    }

                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let mods = build_modifiers(self.modifiers);
                        let evt = if event.repeat {
                            KeyEventType::Repeat
                        } else if event.state == ElementState::Pressed {
                            KeyEventType::Press
                        } else {
                            KeyEventType::Release
                        };
                        let bytes = key_encoding::encode_key(
                            &event.logical_key,
                            mods,
                            tab.mode,
                            event.text.as_ref().map(winit::keyboard::SmolStr::as_str),
                            event.location,
                            evt,
                        );
                        if !bytes.is_empty() {
                            tab.send_pty(&bytes);
                        }
                    }
                }
            }

            WindowEvent::Focused(focused) => {
                // Skip settings window — no PTY to send to
                if self.is_settings_window(window_id) {
                    return;
                }
                if let Some(tid) = self.windows.get(&window_id).and_then(TermWindow::active_tab_id) {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if tab.mode.contains(TermMode::FOCUS_IN_OUT) {
                            let seq = if focused { b"\x1b[I" as &[u8] } else { b"\x1b[O" };
                            tab.send_pty(seq);
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

/// Apply compositor blur/vibrancy when opacity < 1.0.
/// With DX12 + `DirectComposition` (`DxgiFromVisual`), the swapchain supports
/// `PreMultiplied` alpha — the compositor reads our alpha channel directly.
/// Acrylic/vibrancy provides the frosted glass blur behind transparent areas.
fn apply_window_effects(window: &Window, wc: &config::WindowConfig) {
    let opacity = wc.effective_opacity();
    if opacity >= 1.0 {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        if wc.blur {
            let alpha = (opacity * 255.0) as u8;
            let color = Some((30_u8, 30, 46, alpha));
            if let Err(e) = window_vibrancy::apply_acrylic(window, color) {
                log(&format!("vibrancy: acrylic failed: {e}"));
            } else {
                log("vibrancy: acrylic applied");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = window_vibrancy::apply_vibrancy(
            window,
            window_vibrancy::NSVisualEffectMaterial::HudWindow,
            None,
            None,
        ) {
            log(&format!("vibrancy: macOS vibrancy failed: {e}"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        window.set_blur(true);
    }
}

fn build_modifiers(m: ModifiersState) -> Modifiers {
    let mut mods = Modifiers::empty();
    if m.shift_key() {
        mods |= Modifiers::SHIFT;
    }
    if m.alt_key() {
        mods |= Modifiers::ALT;
    }
    if m.control_key() {
        mods |= Modifiers::CONTROL;
    }
    if m.super_key() {
        mods |= Modifiers::SUPER;
    }
    mods
}
