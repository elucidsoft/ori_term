//! Application state, startup, and module dispatch.

mod config_reload;
mod cursor_hover;
mod event_loop;
mod hover_url;
mod input_keyboard;
mod input_mouse;
mod mouse_coord;
mod mouse_report;
mod mouse_selection;
mod render_coord;
mod search_ui;
mod settings_ui;
mod tab_drag;
mod tab_management;
mod window_management;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::clipboard;
use crate::config::{self, Config};
use crate::config::monitor::ConfigMonitor;
use crate::context_menu::MenuOverlay;
use crate::drag::DragState;
use crate::gpu::{GpuRenderer, GpuState};
use crate::key_encoding::Modifiers;
use crate::keybindings::{self, KeyBinding};
use crate::log;
use crate::palette;
use crate::render::FontSet;
use crate::selection;
use crate::tab::{Tab, TabId, TermEvent};
use crate::tab_bar::TabBarHit;
use crate::term_mode::TermMode;
use crate::url_detect::{UrlDetectCache, UrlSegment};
use crate::window::TermWindow;

/// Resize border thickness in pixels.
pub(super) const RESIZE_BORDER: f64 = 8.0;

/// Double-click detection threshold in milliseconds.
pub(super) const DOUBLE_CLICK_MS: u128 = 400;

/// Scroll lines per mouse wheel tick.
pub(super) const SCROLL_LINES: usize = 3;

/// UI font scale relative to grid font (tab bar, search bar, menus).
pub(super) const UI_FONT_SCALE: f32 = 0.75;

#[allow(clippy::struct_excessive_bools, reason = "App state needs multiple flag fields")]
pub struct App {
    pub(super) config: Config,
    pub(super) windows: HashMap<WindowId, TermWindow>,
    pub(super) tabs: HashMap<TabId, Tab>,
    pub(super) glyphs: FontSet,
    pub(super) ui_glyphs: FontSet,
    pub(super) gpu: Option<GpuState>,
    pub(super) renderer: Option<GpuRenderer>,
    pub(super) drag: Option<DragState>,
    pub(super) next_tab_id: u64,
    pub(super) proxy: EventLoopProxy<TermEvent>,
    pub(super) cursor_pos: HashMap<WindowId, PhysicalPosition<f64>>,
    pub(super) hover_hit: HashMap<WindowId, TabBarHit>,
    pub(super) modifiers: ModifiersState,
    pub(super) first_window_created: bool,
    pub(super) last_click_time: Option<Instant>,
    pub(super) last_click_window: Option<WindowId>,
    // Selection state
    pub(super) left_mouse_down: bool,
    pub(super) last_grid_click_pos: Option<(usize, usize)>,
    pub(super) click_count: u8,
    // Mouse reporting dedup
    pub(super) last_mouse_cell: Option<(usize, usize)>,
    // Search
    pub(super) search_active: Option<WindowId>,
    // Hyperlink hover
    pub(super) hover_hyperlink: Option<(WindowId, String)>,
    // Implicit URL detection
    pub(super) url_cache: UrlDetectCache,
    pub(super) hover_url_range: Option<Vec<UrlSegment>>,
    // Context menus & settings
    pub(super) context_menu: Option<MenuOverlay>,
    pub(super) settings_window: Option<WindowId>,
    pub(super) active_scheme: &'static str,
    pub(super) _config_monitor: Option<ConfigMonitor>,
    pub(super) bindings: Vec<KeyBinding>,
    pub(super) scale_factor: f64,
    /// Per-tab X offsets for dodge animation, keyed by window.
    pub(super) tab_anim_offsets: HashMap<WindowId, Vec<f32>>,
    /// Last animation tick time (for time-based decay).
    pub(super) last_anim_time: Instant,
    /// Pixel X position of the dragged tab within its window, for rendering.
    pub(super) drag_visual_x: Option<(WindowId, f32)>,
    /// When the cursor blink timer was last reset (keystroke, PTY output, focus).
    pub(super) cursor_blink_reset: Instant,
    /// True when the tab bar needs to be rebuilt (hover, tab add/remove, etc.).
    pub(super) tab_bar_dirty: bool,
    /// Chrome-style tab width lock: after closing a tab via close button,
    /// tabs keep their current width until the mouse leaves the tab bar.
    pub(super) tab_width_lock: Option<(WindowId, usize)>,
    /// Previous cursor visibility state for dirty tracking.
    pub(super) prev_cursor_visible: bool,
    /// Deferred redraw requests: coalesces multiple PTY events into one
    /// `request_redraw()` per window, drained in `about_to_wait`.
    pub(super) pending_redraw: HashSet<WindowId>,
    /// Path to written shell integration scripts (None when disabled).
    pub(super) shell_integration_dir: Option<PathBuf>,
    /// Cached tab bar data — rebuilt only when `tab_bar_dirty`.
    pub(super) cached_tab_info: Vec<(TabId, String)>,
    pub(super) cached_bell_badges: Vec<bool>,
    /// Torn-off tab pending OS drag completion for post-drag merge check.
    #[cfg(target_os = "windows")]
    /// Torn-off tab pending OS drag completion for post-drag merge check.
    /// Fields: `(window_id, tab_id, mouse_offset_in_tab)`.
    pub(super) torn_off_pending: Option<(WindowId, TabId, f64)>,
}

impl App {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        std::panic::set_hook(Box::new(|info| {
            let _ = std::fs::write("oriterm_panic.log", format!("{info}"));
        }));

        let _ = std::fs::remove_file(crate::log_path());
        let startup = Instant::now();
        log("starting");

        let t0 = Instant::now();
        let config = Config::load();
        log(&format!(
            "config: font_size={}, scheme={}, shell={:?}, scrollback={}, cols={}, rows={}, \
             opacity={}, tab_bar_opacity={}, blur={}, cursor={}, cursor_blink={}, \
             cursor_blink_interval_ms={}, copy_on_select={}, bold_is_bright={} \
             ({:.1}ms)",
            config.font.size,
            config.colors.scheme,
            config.terminal.shell,
            config.terminal.scrollback,
            config.window.columns,
            config.window.rows,
            config.window.effective_opacity(),
            config.window.effective_tab_bar_opacity(),
            config.window.blur,
            config.terminal.cursor_style,
            config.terminal.cursor_blink,
            config.terminal.cursor_blink_interval_ms,
            config.behavior.copy_on_select,
            config.behavior.bold_is_bright,
            t0.elapsed().as_secs_f64() * 1000.0,
        ));

        let t0 = Instant::now();
        let glyphs = FontSet::load(config.font.size, config.font.family.as_deref());
        let ui_size = glyphs.size * UI_FONT_SCALE;
        let ui_glyphs = FontSet::load_ui(ui_size).unwrap_or_else(|| glyphs.resize(ui_size));
        log(&format!(
            "font loaded: cell={}x{}, baseline={}, size={} (ui: {}) ({:.1}ms)",
            glyphs.cell_width,
            glyphs.cell_height,
            glyphs.baseline,
            glyphs.size,
            ui_glyphs.size,
            t0.elapsed().as_secs_f64() * 1000.0,
        ));

        let bindings = keybindings::merge_bindings(&config.keybind);

        let active_scheme =
            palette::find_scheme(&config.colors.scheme).map_or("Catppuccin Mocha", |s| s.name);

        let t0 = Instant::now();
        let event_loop = EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("event loop");
        let proxy = event_loop.create_proxy();
        log(&format!(
            "event loop created: {:.1}ms",
            t0.elapsed().as_secs_f64() * 1000.0
        ));

        let config_monitor = ConfigMonitor::new(proxy.clone());

        let shell_integration_dir = if config.behavior.shell_integration {
            match crate::shell_integration::ensure_scripts_on_disk(&config::config_dir()) {
                Ok(dir) => {
                    log(&format!(
                        "shell_integration: scripts written to {}",
                        dir.display()
                    ));
                    Some(dir)
                }
                Err(e) => {
                    log(&format!("shell_integration: failed to write scripts: {e}"));
                    None
                }
            }
        } else {
            None
        };

        log(&format!(
            "pre-event-loop total: {:.1}ms",
            startup.elapsed().as_secs_f64() * 1000.0
        ));

        let mut app = Self {
            config,
            windows: HashMap::new(),
            tabs: HashMap::new(),
            glyphs,
            ui_glyphs,
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
            context_menu: None,
            settings_window: None,
            active_scheme,
            _config_monitor: config_monitor,
            bindings,
            scale_factor: 1.0,
            tab_anim_offsets: HashMap::new(),
            last_anim_time: Instant::now(),
            drag_visual_x: None,
            cursor_blink_reset: Instant::now(),
            tab_bar_dirty: true,
            tab_width_lock: None,
            prev_cursor_visible: true,
            pending_redraw: HashSet::new(),
            shell_integration_dir,
            cached_tab_info: Vec::new(),
            cached_bell_badges: Vec::new(),
            #[cfg(target_os = "windows")]
            torn_off_pending: None,
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }

    /// Scale a logical pixel value by the current DPI scale factor.
    pub(super) fn scale_px(&self, v: usize) -> usize {
        (v as f64 * self.scale_factor).round() as usize
    }

    /// Returns the locked tab width for `window_id`, if active.
    pub(super) fn tab_width_lock_for(&self, window_id: WindowId) -> Option<usize> {
        self.tab_width_lock
            .filter(|(wid, _)| *wid == window_id)
            .map(|(_, w)| w)
    }

    pub(super) fn alloc_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    /// Returns the active tab ID for the given window.
    pub(super) fn active_tab_id(&self, window_id: WindowId) -> Option<TabId> {
        self.windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id)
    }

    /// Copy the current selection text to clipboard. Returns true if text was copied.
    pub(super) fn copy_selection_to_clipboard(&self, tab_id: TabId) -> bool {
        if let Some(tab) = self.tabs.get(&tab_id) {
            if let Some(ref sel) = tab.selection {
                let text = selection::extract_text(tab.grid(), sel);
                if !text.is_empty() {
                    clipboard::set_text(&text);
                    return true;
                }
            }
        }
        false
    }

    /// Dismiss the context menu overlay and mark the tab bar dirty.
    pub(super) fn dismiss_context_menu(&mut self, window_id: WindowId) {
        self.context_menu = None;
        self.tab_bar_dirty = true;
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    /// Toggle the maximized state of a window.
    pub(super) fn toggle_maximize(&mut self, window_id: WindowId) {
        if let Some(tw) = self.windows.get_mut(&window_id) {
            let new_max = !tw.is_maximized;
            tw.is_maximized = new_max;
            tw.window.set_maximized(new_max);
            tw.window.request_redraw();
        }
    }

    /// Rebuild the GPU glyph atlas after font size changes.
    pub(super) fn rebuild_atlas(&mut self) {
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
            renderer.rebuild_atlas(gpu, &mut self.glyphs, &mut self.ui_glyphs);
        }
    }

    pub(super) fn paste_from_clipboard(&mut self, window_id: WindowId) {
        let Some(tab_id) = self.active_tab_id(window_id) else {
            return;
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
}

/// Apply compositor blur/vibrancy when opacity < 1.0.
///
/// With DX12 + `DirectComposition` (`DxgiFromVisual`), the swapchain supports
/// `PreMultiplied` alpha — the compositor reads our alpha channel directly.
/// Acrylic/vibrancy provides the frosted glass blur behind transparent areas.
pub(super) fn apply_window_effects(window: &Window, wc: &config::WindowConfig) {
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
        if wc.blur {
            if let Err(e) = window_vibrancy::apply_vibrancy(
                window,
                window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground,
                None,
                None,
            ) {
                log(&format!("vibrancy: macOS vibrancy failed: {e}"));
            } else {
                log("vibrancy: macOS UnderWindowBackground applied");
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if wc.blur {
            window.set_blur(true);
            log("transparency: blur enabled on Linux");
        }
    }
}

pub(super) fn build_modifiers(m: ModifiersState) -> Modifiers {
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
