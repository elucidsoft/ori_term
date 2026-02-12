//! Configuration hot-reload — applies config changes to all windows and tabs.

use winit::window::WindowId;

use crate::config::{self, Config};
use crate::keybindings;
use crate::log;
use crate::palette;
use crate::render::FontSet;

use super::{App, UI_FONT_SCALE};

impl App {
    pub(super) fn apply_config_reload(&mut self) {
        let new_config = match Config::try_load() {
            Ok(c) => c,
            Err(e) => {
                log(&format!("config reload: {e}"));
                return;
            }
        };

        // Color scheme
        let scheme_changed = new_config.colors.scheme != self.config.colors.scheme;
        if scheme_changed {
            if let Some(scheme) = palette::find_scheme(&new_config.colors.scheme) {
                self.active_scheme = scheme.name;
                for tab in self.tabs.values_mut() {
                    tab.palette.set_scheme(scheme);
                }
            }
        }
        // Re-apply color overrides (always — they may have changed independently)
        for tab in self.tabs.values_mut() {
            tab.palette.apply_overrides(&new_config.colors);
        }

        // Font size or family change
        let font_changed = (new_config.font.size - self.config.font.size).abs() > f32::EPSILON
            || new_config.font.family != self.config.font.family;
        if font_changed {
            let scaled_size = new_config.font.size * self.scale_factor as f32;
            self.glyphs = FontSet::load(scaled_size, new_config.font.family.as_deref());
            self.ui_glyphs = self.ui_glyphs.resize(scaled_size * UI_FONT_SCALE);
            log(&format!(
                "config reload: font size={}, cell={}x{}",
                self.glyphs.size, self.glyphs.cell_width, self.glyphs.cell_height
            ));
            if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
                renderer.rebuild_atlas(gpu, &mut self.glyphs, &mut self.ui_glyphs);
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

        // Mark everything dirty — config may affect both grid and tab bar.
        self.tab_bar_dirty = true;
        for tab in self.tabs.values_mut() {
            tab.grid_dirty = true;
        }

        // Redraw all windows
        for tw in self.windows.values() {
            tw.window.request_redraw();
        }
        log("config reload: applied successfully");
    }
}
