//! Window lifecycle — creation, resize, close, font scaling.

use std::sync::Arc;
use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowId};

use crate::config;
use crate::gpu::{GpuRenderer, GpuState};
use crate::log;
use crate::palette;
use crate::tab::TabId;
use crate::grid::{GRID_PADDING_BOTTOM, GRID_PADDING_LEFT, GRID_PADDING_TOP};
use crate::tab_bar::TAB_BAR_HEIGHT;
use crate::window::TermWindow;
#[cfg(target_os = "windows")]
use super::RESIZE_BORDER;
use super::{App, UI_FONT_SCALE, apply_window_effects};

impl App {
    /// Begin a window drag.
    ///
    /// Start an OS-native window drag. On Windows with snap support, the OS
    /// handles dragging via `HTCAPTION` so this is only a fallback. On
    /// Linux/macOS the compositor handles the move (required on Wayland).
    pub(super) fn start_window_drag(&self, window_id: WindowId) {
        if let Some(tw) = self.windows.get(&window_id) {
            let _ = tw.window.drag_window();
        }
    }

    pub(super) fn change_font_size(&mut self, window_id: WindowId, delta: f32) {
        let new_size = self.font_collection.size + delta * self.scale_factor as f32;
        self.apply_font_size(new_size, "font resize", window_id);
    }

    pub(super) fn reset_font_size(&mut self, window_id: WindowId) {
        let new_size = self.config.font.size * self.scale_factor as f32;
        self.apply_font_size(new_size, "font reset", window_id);
    }

    /// Resize fonts to `new_size`, rebuild the glyph atlas, and reflow all tabs.
    fn apply_font_size(&mut self, new_size: f32, label: &str, window_id: WindowId) {
        self.font_collection = self.font_collection.resize(new_size);
        self.ui_glyphs = self.ui_glyphs.resize(new_size * UI_FONT_SCALE);
        log(&format!(
            "{label}: size={}, cell={}x{}",
            self.font_collection.size,
            self.font_collection.cell_width,
            self.font_collection.cell_height,
        ));
        self.rebuild_atlas();
        self.resize_all_tabs_in_window(window_id);
    }

    pub(super) fn resize_all_tabs_in_window(&mut self, window_id: WindowId) {
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
                tab.clear_selection();
                tab.resize(cols, rows, pixel_w, pixel_h);
                tab.grid_dirty = true;
            }
        }
        self.tab_bar_dirty = true;
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    pub(super) fn load_window_icon() -> Option<Icon> {
        let data = include_bytes!(concat!(env!("OUT_DIR"), "/icon_rgba.bin"));
        let w = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let h = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        Icon::from_rgba(data[8..].to_vec(), w, h).ok()
    }

    pub(super) fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        saved_pos: Option<&config::WindowState>,
        visible: bool,
    ) -> Option<WindowId> {
        let win_w = (self.font_collection.cell_width * self.config.window.columns) as u32
            + self.scale_px(GRID_PADDING_LEFT) as u32;
        let win_h = (self.font_collection.cell_height * self.config.window.rows) as u32
            + self.scale_px(TAB_BAR_HEIGHT) as u32
            + self.scale_px(GRID_PADDING_TOP) as u32
            + self.scale_px(GRID_PADDING_BOTTOM) as u32;

        let use_transparent = self.config.window.effective_opacity() < 1.0;

        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title("oriterm")
            .with_inner_size(winit::dpi::PhysicalSize::new(win_w, win_h))
            .with_decorations(false)
            .with_visible(false)
            .with_transparent(use_transparent)
            .with_window_icon(Self::load_window_icon());

        // On Windows, DX12 + DirectComposition needs WS_EX_NOREDIRECTIONBITMAP
        // so the compositor reads alpha from our swapchain.
        #[cfg(target_os = "windows")]
        if use_transparent {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_no_redirection_bitmap(true);
        }

        let create_start = Instant::now();
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("failed to create window: {e}"));
                return None;
            }
        };
        log(&format!(
            "window created: {:.1}ms",
            create_start.elapsed().as_secs_f64() * 1000.0
        ));

        // Capture initial scale factor from the window
        let initial_scale = window.scale_factor();
        if (initial_scale - self.scale_factor).abs() > 0.01 {
            self.scale_factor = initial_scale;
            let expected_size = self.config.font.size * initial_scale as f32;
            if (self.font_collection.size - expected_size).abs() > 0.1 {
                self.font_collection = self.font_collection.resize(expected_size);
                self.ui_glyphs = self.ui_glyphs.resize(expected_size * UI_FONT_SCALE);
                log(&format!(
                    "HiDPI: scale={initial_scale:.2}, font reloaded at size={expected_size}"
                ));
            }
        }

        // Initialize GPU on first window creation
        if self.gpu.is_none() {
            let t0 = Instant::now();
            let gpu = GpuState::new(&window, use_transparent);
            log(&format!(
                "GPU init: {:.1}ms",
                t0.elapsed().as_secs_f64() * 1000.0
            ));

            let t0 = Instant::now();
            let renderer = GpuRenderer::new(&gpu);
            log(&format!(
                "renderer init: {:.1}ms",
                t0.elapsed().as_secs_f64() * 1000.0
            ));

            // Warn if user requested transparency but GPU doesn't support it.
            if use_transparent && !gpu.supports_transparency() {
                log(
                    "WARNING: transparency requested (opacity < 1.0) but GPU surface alpha mode is Opaque — window will be opaque",
                );
            }

            self.gpu = Some(gpu);
            self.renderer = Some(renderer);
        }

        let gpu = self.gpu.as_ref().expect("GPU initialized");
        let id = window.id();

        let tw = TermWindow::new(window.clone(), gpu)?;

        // Render a clear frame using the configured color scheme's background
        // before showing the window (prevents gray/white flash on startup)
        if let Some(renderer) = &self.renderer {
            let s = crate::gpu::srgb_to_linear;
            let scheme_bg = palette::find_scheme(&self.config.colors.scheme).map_or(
                vte::ansi::Rgb {
                    r: 0x1e,
                    g: 0x1e,
                    b: 0x2e,
                },
                |sc| sc.bg,
            );
            let bg = [
                s(scheme_bg.r as f32 / 255.0),
                s(scheme_bg.g as f32 / 255.0),
                s(scheme_bg.b as f32 / 255.0),
                1.0,
            ];
            renderer.clear_surface(gpu, &tw.surface, bg, self.config.window.effective_opacity());
            // Wait for the GPU to finish so the frame reaches the compositor
            // before the window becomes visible (prevents gray flash)
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }

        // Apply compositor blur/vibrancy + layered window transparency
        apply_window_effects(&window, &self.config.window);

        // Enable Aero Snap for borderless window (WndProc subclass)
        #[cfg(target_os = "windows")]
        crate::platform_windows::enable_snap(
            &window,
            (RESIZE_BORDER * self.scale_factor) as i32,
            self.scale_px(TAB_BAR_HEIGHT) as i32,
        );

        // Restore saved position before showing — avoids gray flash from
        // moving a visible window which triggers a WM repaint.
        if let Some(state) = saved_pos {
            window.set_outer_position(PhysicalPosition::new(state.x, state.y));
        }

        // Now show the window with the dark frame already presented
        if visible {
            window.set_visible(true);
        }

        self.windows.insert(id, tw);
        log(&format!(
            "window created: {id:?}, visible={visible} ({:.1}ms total create_window)",
            create_start.elapsed().as_secs_f64() * 1000.0
        ));
        Some(id)
    }

    pub(super) fn close_window(&mut self, window_id: WindowId, _event_loop: &ActiveEventLoop) {
        // If closing the settings window, just remove it — don't exit
        if self.settings_window == Some(window_id) {
            self.close_settings_window();
            return;
        }

        // Check if this is the last terminal window BEFORE dropping anything.
        let other_terminal_windows = self
            .windows
            .keys()
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
    pub(super) fn exit_app(&mut self) {
        // Save window position and size before exiting.
        if let Some(tw) = self.windows.values().next() {
            if let Ok(pos) = tw.window.outer_position() {
                let size = tw.window.inner_size();
                let state = config::WindowState {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                };
                state.save();
            }
        }

        // Save Vulkan pipeline cache to disk for faster next launch
        if let Some(gpu) = &self.gpu {
            gpu.save_pipeline_cache();
        }

        for tab in self.tabs.values_mut() {
            tab.shutdown();
        }
        // Don't join threads — process::exit will clean them up.
        std::process::exit(0);
    }

    pub(super) fn handle_resize(&mut self, window_id: WindowId, width: u32, height: u32) {
        // Settings window doesn't have tabs to resize
        if self.is_settings_window(window_id) {
            if let (Some(tw), Some(gpu)) = (self.windows.get_mut(&window_id), self.gpu.as_ref()) {
                tw.resize_surface(&gpu.device, width, height);
            }
            return;
        }

        // On Windows, our WndProc subclass eats WM_DPICHANGED (to prevent
        // oscillation), so winit never fires ScaleFactorChanged.  Query the
        // actual DPI stored by the subclass and update self.scale_factor so
        // that font reload and layout calculations use the correct value.
        #[cfg(target_os = "windows")]
        if let Some(tw) = self.windows.get(&window_id) {
            if let Some(sf) = crate::platform_windows::get_current_dpi(&tw.window) {
                if (sf - self.scale_factor).abs() > 0.01 {
                    self.scale_factor = sf;
                }
            }
        }

        // Clear width lock — window size changed, tab widths will be recalculated
        if self.tab_width_lock.is_some_and(|(wid, _)| wid == window_id) {
            self.tab_width_lock = None;
        }

        // Resize the wgpu surface
        if let (Some(tw), Some(gpu)) = (self.windows.get_mut(&window_id), self.gpu.as_ref()) {
            tw.resize_surface(&gpu.device, width, height);
        }

        // If the DPI changed, reload fonts before calculating grid dimensions
        // so that cell metrics match the new scale factor.
        let expected_size = self.config.font.size * self.scale_factor as f32;
        if (self.font_collection.size - expected_size).abs() > 0.1 {
            self.font_collection = self.font_collection.resize(expected_size);
            self.ui_glyphs = self.ui_glyphs.resize(expected_size * UI_FONT_SCALE);
            self.rebuild_atlas();
        }

        let (cols, rows) = self.grid_dims_for_size(width, height);

        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let pixel_w = width as u16;
        let pixel_h = height as u16;

        for &tab_id in &tw.tabs {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.clear_selection();
                tab.resize(cols, rows, pixel_w, pixel_h);
                tab.grid_dirty = true;
            }
        }
    }

    pub(super) fn handle_scale_factor_changed(&mut self, _window_id: WindowId, scale_factor: f64) {
        if (scale_factor - self.scale_factor).abs() < 0.01 {
            return;
        }
        self.scale_factor = scale_factor;
        // Request redraw on all windows so UI chrome rescales
        for tw in self.windows.values() {
            tw.window.request_redraw();
        }
    }
}
