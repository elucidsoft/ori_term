//! Settings window — theme picker lifecycle and rendering.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::App;
use crate::log;
use crate::palette::{BUILTIN_SCHEMES, ColorScheme};
use crate::window::TermWindow;

/// Settings window close button size (pixels, top-right corner).
const SETTINGS_CLOSE_SIZE: usize = 30;

/// Settings window title area height (pixels).
const SETTINGS_TITLE_HEIGHT: usize = 50;

/// Settings window row height for scheme list items (pixels).
const SETTINGS_ROW_HEIGHT: usize = 40;

impl App {
    pub(super) fn is_settings_window(&self, window_id: WindowId) -> bool {
        self.settings_window == Some(window_id)
    }

    pub(super) fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
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

        let attrs = winit::window::Window::default_attributes()
            .with_title("Settings")
            .with_inner_size(winit::dpi::PhysicalSize::new(win_w, win_h))
            .with_decorations(false)
            .with_resizable(false);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => std::sync::Arc::new(w),
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
            let s = crate::gpu::srgb_to_linear;
            let bg = [
                s(0x1e as f32 / 255.0),
                s(0x1e as f32 / 255.0),
                s(0x2e as f32 / 255.0),
                1.0,
            ];
            renderer.clear_surface(gpu, &tw.surface, bg, 1.0);
        }

        window.set_visible(true);
        self.windows.insert(id, tw);
        self.settings_window = Some(id);
        log(&format!("created settings window {id:?}"));
    }

    pub(super) fn close_settings_window(&mut self) {
        if let Some(wid) = self.settings_window.take() {
            self.windows.remove(&wid);
        }
    }

    pub(super) fn render_settings_window(&mut self, window_id: WindowId) {
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
            &mut self.ui_collection,
        );
    }

    pub(super) fn handle_settings_mouse(&mut self, window_id: WindowId, x: usize, y: usize) {
        let w = self
            .windows
            .get(&window_id)
            .map_or(0, |tw| tw.window.inner_size().width as usize);
        if x >= w.saturating_sub(SETTINGS_CLOSE_SIZE) && y < SETTINGS_CLOSE_SIZE {
            self.close_settings_window();
            return;
        }

        if y < SETTINGS_TITLE_HEIGHT {
            return;
        }

        let row_idx = (y - SETTINGS_TITLE_HEIGHT) / SETTINGS_ROW_HEIGHT;
        if row_idx < BUILTIN_SCHEMES.len() {
            let scheme = BUILTIN_SCHEMES[row_idx];
            self.apply_scheme_to_all_tabs(scheme);
        }
    }

    pub(super) fn apply_scheme_to_all_tabs(&mut self, scheme: &'static ColorScheme) {
        self.active_scheme = scheme.name;
        for tab in self.tabs.values_mut() {
            tab.apply_color_config(
                Some(scheme),
                &self.config.colors,
                self.config.behavior.bold_is_bright,
            );
        }
        // Persist the scheme change
        scheme.name.clone_into(&mut self.config.colors.scheme);
        self.config.save();
        // Redraw all windows
        for tw in self.windows.values() {
            tw.window.request_redraw();
        }
    }
}
