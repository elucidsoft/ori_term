//! Render coordination — frame building and context menu action dispatch.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::App;
use crate::context_menu::ContextAction;
use crate::gpu::FrameParams;
use crate::log;
use crate::palette;
use crate::tab::TabId;
use crate::grid::{GRID_PADDING_BOTTOM, GRID_PADDING_LEFT, GRID_PADDING_TOP};
use crate::tab_bar::TAB_BAR_HEIGHT;
#[cfg(target_os = "windows")]
use crate::tab_bar::{
    CONTROLS_ZONE_WIDTH, DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_LEFT_MARGIN,
    TabBarLayout,
};
use crate::tab_bar::TabBarHit;

impl App {
    /// Computes grid dimensions (cols, rows) for a given window size in physical pixels.
    pub(super) fn grid_dims_for_size(&self, width: u32, height: u32) -> (usize, usize) {
        let cw = self.glyphs.cell_width;
        let ch = self.glyphs.cell_height;
        let grid_w = (width as usize).saturating_sub(self.scale_px(GRID_PADDING_LEFT));
        let grid_h = (height as usize).saturating_sub(
            self.scale_px(TAB_BAR_HEIGHT)
                + self.scale_px(GRID_PADDING_TOP)
                + self.scale_px(GRID_PADDING_BOTTOM),
        );
        let cols = if cw > 0 {
            grid_w / cw
        } else {
            self.config.window.columns
        };
        let rows = if ch > 0 {
            grid_h / ch
        } else {
            self.config.window.rows
        };
        (cols.max(2), rows.max(1))
    }

    /// Renders a single window, building frame params and dispatching to GPU renderer.
    pub(super) fn render_window(&mut self, window_id: WindowId) {
        // Settings window has its own renderer path
        if self.is_settings_window(window_id) {
            self.render_settings_window(window_id);
            return;
        }

        // Extract all info we need before borrowing mutably.
        let (phys, active_idx, active_tab_id, is_maximized) = {
            let tw = match self.windows.get(&window_id) {
                Some(tw) => tw,
                None => return,
            };
            let phys = tw.window.inner_size();
            let active_idx = tw.active_tab;
            let active_tab_id = tw.active_tab_id();
            let is_maximized = tw.is_maximized;
            (phys, active_idx, active_tab_id, is_maximized)
        };

        // Only rebuild tab bar data (titles + bell badges) when dirty.
        if self.tab_bar_dirty {
            let (new_info, new_badges) = if let Some(tw) = self.windows.get(&window_id) {
                let info: Vec<(TabId, String)> = tw
                    .tabs
                    .iter()
                    .map(|id| {
                        let title = self
                            .tabs
                            .get(id)
                            .map_or_else(|| "?".to_string(), |t| t.effective_title().into_owned());
                        (*id, title)
                    })
                    .collect();
                let badges: Vec<bool> = tw
                    .tabs
                    .iter()
                    .map(|id| {
                        self.tabs.get(id).is_some_and(|t| t.has_bell_badge)
                            && Some(*id) != active_tab_id
                    })
                    .collect();
                (info, badges)
            } else {
                (Vec::new(), Vec::new())
            };
            self.cached_tab_info = new_info;
            self.cached_bell_badges = new_badges;
        }
        let tab_info = &self.cached_tab_info;
        let bell_badges = &self.cached_bell_badges;

        if phys.width == 0 || phys.height == 0 {
            return;
        }

        // Update snap hit-test rects so WM_NCHITTEST knows where interactive
        // elements are (HTCLIENT). Everything else in the caption zone becomes
        // HTCAPTION so the OS handles drag natively (avoids DPI oscillation).
        #[cfg(target_os = "windows")]
        {
            if let Some(tw) = self.windows.get(&window_id) {
                let sf = self.scale_factor;
                let bar_w = phys.width as usize;
                let twl = self.tab_width_lock_for(window_id);
                let layout = TabBarLayout::compute(tab_info.len(), bar_w, sf, twl);
                let h = self.scale_px(TAB_BAR_HEIGHT) as i32;
                let left_margin = self.scale_px(TAB_LEFT_MARGIN);
                let mut rects = Vec::new();
                // Individual tab rects
                for i in 0..layout.tab_count {
                    let left = (left_margin + i * layout.tab_width) as i32;
                    let right = left + layout.tab_width as i32;
                    rects.push([left, 0, right, h]);
                }
                // New tab button
                let new_tab_w = self.scale_px(NEW_TAB_BUTTON_WIDTH);
                let tabs_end = left_margin + layout.tab_count * layout.tab_width;
                rects.push([tabs_end as i32, 0, (tabs_end + new_tab_w) as i32, h]);
                // Dropdown button
                let dropdown_w = self.scale_px(DROPDOWN_BUTTON_WIDTH);
                let dd_start = tabs_end + new_tab_w;
                rects.push([dd_start as i32, 0, (dd_start + dropdown_w) as i32, h]);
                // Window controls
                let controls_w = self.scale_px(CONTROLS_ZONE_WIDTH);
                let controls_start = bar_w.saturating_sub(controls_w) as i32;
                rects.push([controls_start, 0, bar_w as i32, h]);
                crate::platform_windows::set_client_rects(&tw.window, rects);
            }
        }

        let hover = self
            .hover_hit
            .get(&window_id)
            .copied()
            .unwrap_or(TabBarHit::None);

        // Drag visual: if this window has a dragged tab, pass its pixel X and
        // the current animation offsets.
        let dragged_tab = self
            .drag_visual_x
            .filter(|(wid, _)| *wid == window_id)
            .and_then(|(_, px)| {
                self.drag.as_ref().and_then(|d| {
                    self.windows
                        .get(&window_id)
                        .and_then(|tw| tw.tab_index(d.tab_id))
                        .map(|idx| (idx, px))
                })
            });
        let tab_offsets = self
            .tab_anim_offsets
            .get(&window_id)
            .map_or(&[][..], |v| v.as_slice());

        let any_bell_badge = bell_badges.iter().any(|&b| b);

        // Smooth sine pulse for bell badge animation (~0.5 Hz).
        let bell_phase = if any_bell_badge {
            let secs = self.last_anim_time.elapsed().as_secs_f32();
            (secs * std::f32::consts::TAU * 0.5).sin() * 0.5 + 0.5
        } else {
            0.0
        };

        // Cursor blink: compute visibility based on elapsed time since last reset.
        let cursor_visible = if self.config.terminal.cursor_blink {
            let elapsed_ms = self.cursor_blink_reset.elapsed().as_millis() as u64;
            let interval = self.config.terminal.cursor_blink_interval_ms.max(1);
            (elapsed_ms / interval).is_multiple_of(2)
        } else {
            true
        };

        // Damage tracking: compute dirty flags.
        let cursor_visible_changed = cursor_visible != self.prev_cursor_visible;
        self.prev_cursor_visible = cursor_visible;

        let grid_dirty = active_tab_id
            .and_then(|id| self.tabs.get(&id))
            .is_some_and(|tab| tab.grid_dirty)
            || cursor_visible_changed;
        let tab_bar_dirty = self.tab_bar_dirty;

        // Pre-compute before closure to avoid borrowing all of `self`.
        let twl = self.tab_width_lock_for(window_id);

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
                tab_info,
                active_tab: active_idx,
                hover_hit: hover,
                is_maximized,
                context_menu: self.context_menu.as_ref(),
                opacity: self.config.window.effective_opacity(),
                hover_hyperlink: self
                    .hover_hyperlink
                    .as_ref()
                    .filter(|(wid, _)| *wid == window_id)
                    .map(|(_, uri)| uri.as_str()),
                hover_url_range: self.hover_url_range.as_deref(),
                minimum_contrast: self.config.colors.effective_minimum_contrast(),
                alpha_blending: self.config.colors.alpha_blending,
                dragged_tab,
                tab_offsets,
                bell_badges,
                bell_phase,
                scale: self.scale_factor as f32,
                cursor_visible,
                grid_dirty,
                tab_bar_dirty,
                window_id,
                tab_width_lock: twl,
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
            &mut self.ui_glyphs,
        );

        // Clear dirty flags after rendering.
        if let Some(tab_id) = active_tab_id {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.grid_dirty = false;
            }
        }
        self.tab_bar_dirty = false;
    }

    /// Dispatches context menu action by executing the appropriate app method.
    pub(super) fn dispatch_context_action(
        &mut self,
        action: ContextAction,
        event_loop: &ActiveEventLoop,
    ) {
        log(&format!("menu action: {action:?}"));
        match action {
            ContextAction::CloseTab(idx) => {
                let tab_id = self
                    .windows
                    .values()
                    .find_map(|tw| tw.tabs.get(idx).copied());
                if let Some(tid) = tab_id {
                    self.close_tab(tid, event_loop);
                }
            }
            ContextAction::DuplicateTab(idx) => {
                self.duplicate_tab_at(idx);
            }
            ContextAction::MoveTabToNewWindow(idx) => {
                self.move_tab_to_new_window(idx, event_loop);
            }
            ContextAction::NewTab => {
                if let Some(&wid) = self.windows.keys().next() {
                    self.new_tab_in_window(wid);
                }
            }
            ContextAction::OpenSettings => {
                self.open_settings_window(event_loop);
            }
            ContextAction::SelectScheme(name) => {
                if let Some(scheme) = palette::find_scheme(&name) {
                    self.apply_scheme_to_all_tabs(scheme);
                }
            }
        }
    }
}
