//! Configuration hot-reload — applies config changes to all windows and tabs.

use winit::window::WindowId;

use crate::config::{self, Config};
use crate::font::FontCollection;
use crate::keybindings;
use crate::log;
use crate::palette;
use super::{App, UI_FONT_SCALE};

impl App {
    /// Applies a reloaded configuration to all windows and tabs.
    ///
    /// Reloads the config file, applies color scheme, font, cursor style, and keybinding changes,
    /// and marks all windows dirty for redraw.
    pub(super) fn apply_config_reload(&mut self) {
        let new_config = match Config::try_load() {
            Ok(c) => c,
            Err(e) => {
                log(&format!("config reload: {e}"));
                return;
            }
        };

        // Color scheme + overrides
        let scheme_changed = new_config.colors.scheme != self.config.colors.scheme;
        let scheme = if scheme_changed {
            let s = palette::find_scheme(&new_config.colors.scheme);
            if let Some(s) = s {
                self.active_scheme = s.name;
            }
            s
        } else {
            None
        };
        for tab in self.tabs.values_mut() {
            tab.apply_color_config(
                scheme,
                &new_config.colors,
                new_config.behavior.bold_is_bright,
            );
        }

        // Font size, family, or features change
        let font_changed = (new_config.font.size - self.config.font.size).abs() > f32::EPSILON
            || new_config.font.family != self.config.font.family
            || new_config.font.features != self.config.font.features;
        if font_changed {
            let scaled_size = new_config.font.size * self.scale_factor as f32;
            self.font_collection = FontCollection::load(
                scaled_size,
                new_config.font.family.as_deref(),
                &FontCollection::parse_features(&new_config.font.features),
            );
            self.ui_glyphs = self.ui_glyphs.resize(scaled_size * UI_FONT_SCALE);
            log(&format!(
                "config reload: font size={}, cell={}x{}",
                self.font_collection.size,
                self.font_collection.cell_width,
                self.font_collection.cell_height,
            ));
            self.rebuild_atlas();
            let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
            for wid in window_ids {
                if !self.is_settings_window(wid) {
                    self.resize_all_tabs_in_window(wid);
                }
            }
        }

        // Cursor style
        if new_config.terminal.cursor_style != self.config.terminal.cursor_style {
            let new_cursor = config::parse_cursor_style(&new_config.terminal.cursor_style);
            for tab in self.tabs.values_mut() {
                tab.set_cursor_shape(new_cursor);
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
