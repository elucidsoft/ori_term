//! Cursor movement dispatch — resize edges, hover, drag state machine.

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, ResizeDirection, WindowId};

use crate::drag::{DRAG_START_THRESHOLD, DragPhase, TEAR_OFF_THRESHOLD};
use crate::log;
use crate::tab_bar::{CONTROLS_ZONE_WIDTH, TAB_BAR_HEIGHT, TabBarHit, TabBarLayout};
use crate::term_mode::TermMode;
use crate::window::TermWindow;

use super::{App, RESIZE_BORDER};

impl App {
    /// Detects if cursor is in the resize border zone.
    ///
    /// Returns the resize direction if so, or None if the cursor is in the client area.
    pub(super) fn resize_direction_at(
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

        let border = RESIZE_BORDER * self.scale_factor;
        let left = x < border;
        let right = x >= w - border;
        let top = y < border;
        let bottom = y >= h - border;

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

    /// Handles cursor movement, hover effects, URL detection, and drag updates.
    ///
    /// Updates cursor icon for resize edges and hyperlinks, tracks mouse reporting,
    /// updates selection drag, handles tab bar hover, and manages tab drag state.
    pub(super) fn handle_cursor_moved(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
        event_loop: &ActiveEventLoop,
    ) {
        self.cursor_pos.insert(window_id, position);

        // Context menu hover tracking
        if let Some(ref mut menu) = self.context_menu {
            let old = menu.hovered;
            menu.hovered = menu.hit_test(position.x as f32, position.y as f32);
            if menu.hovered != old {
                self.tab_bar_dirty = true;
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
        }

        // Resize border cursor icon
        let cursor_icon = if let Some(dir) = self.resize_direction_at(window_id, position) {
            dir.into()
        } else {
            CursorIcon::Default
        };

        // Hyperlink/URL hover detection
        let (cursor_icon, new_hover, new_url_range) = if cursor_icon == CursorIcon::Default
            && self.modifiers.control_key()
            && !self.is_settings_window(window_id)
        {
            if let Some((col, line)) = self.pixel_to_cell(position) {
                let result = self.detect_hover_url(window_id, col, line);
                (result.cursor_icon, result.hover, result.url_range)
            } else {
                (cursor_icon, None, None)
            }
        } else {
            (cursor_icon, None, None)
        };

        let hover_changed =
            self.hover_hyperlink != new_hover || self.hover_url_range != new_url_range;
        self.hover_hyperlink = new_hover;
        self.hover_url_range = new_url_range;

        if hover_changed {
            if let Some(tid) = self
                .windows
                .get(&window_id)
                .and_then(TermWindow::active_tab_id)
            {
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    tab.grid_dirty = true;
                }
            }
        }

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.set_cursor(cursor_icon);
            if hover_changed {
                tw.window.request_redraw();
            }
        }

        // Mouse motion reporting (before selection drag)
        let mut mouse_motion_reported = false;
        if !self.is_settings_window(window_id) && !self.modifiers.shift_key() {
            let tab_id = self
                .windows
                .get(&window_id)
                .and_then(TermWindow::active_tab_id);
            if let Some(tid) = tab_id {
                let report_all = self
                    .tabs
                    .get(&tid)
                    .is_some_and(|t| t.mode.contains(TermMode::MOUSE_ALL));
                let report_motion = self
                    .tabs
                    .get(&tid)
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

        // Selection drag (skip when mouse reporting handled motion)
        if self.left_mouse_down && !mouse_motion_reported {
            self.update_selection_drag(window_id, position);
        }

        // Tab bar hover
        self.update_tab_bar_hover(window_id, position);

        // Tab drag state machine
        self.update_drag_state(window_id, position, event_loop);
    }

    /// Update tab bar hover state and width lock.
    fn update_tab_bar_hover(&mut self, window_id: WindowId, position: PhysicalPosition<f64>) {
        let y = position.y as usize;
        let x = position.x as usize;
        let tab_bar_h = self.scale_px(TAB_BAR_HEIGHT);

        if y < tab_bar_h {
            if let Some(tw) = self.windows.get(&window_id) {
                let twl = self.tab_width_lock_for(window_id);
                let layout = TabBarLayout::compute(
                    tw.tabs.len(),
                    tw.window.inner_size().width as usize,
                    self.scale_factor,
                    twl,
                );
                let hit = layout.hit_test(x, y, self.scale_factor);
                let prev = self.hover_hit.insert(window_id, hit);
                if prev != Some(hit) {
                    self.tab_bar_dirty = true;
                    tw.window.request_redraw();
                }
            }
        } else {
            // Mouse left tab bar — release width lock so tabs expand naturally
            if self.tab_width_lock.is_some_and(|(wid, _)| wid == window_id) {
                self.tab_width_lock = None;
                self.tab_bar_dirty = true;
            }
            let prev = self.hover_hit.insert(window_id, TabBarHit::None);
            if prev != Some(TabBarHit::None) {
                self.tab_bar_dirty = true;
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
        }
    }

    /// Advance the tab drag state machine (`Pending` → `DraggingInBar` → `TornOff`).
    fn update_drag_state(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
        event_loop: &ActiveEventLoop,
    ) {
        let drag_action = self.drag.as_ref().map(|drag| {
            (
                drag.phase,
                drag.tab_id,
                drag.source_window,
                drag.grab_offset,
                drag.distance_from_origin(position),
                drag.mouse_offset_in_tab,
            )
        });

        let Some((phase, tab_id, source_wid, grab_offset, dist, mouse_off)) = drag_action else {
            return;
        };

        match phase {
            DragPhase::Pending => {
                if dist >= DRAG_START_THRESHOLD {
                    let is_single_tab = self
                        .windows
                        .get(&source_wid)
                        .is_some_and(|tw| tw.tabs.len() <= 1);
                    if is_single_tab {
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::TornOff;
                            drag.grab_offset = position;
                        }
                        #[cfg(target_os = "windows")]
                        if let Some(tw) = self.windows.get(&source_wid) {
                            crate::platform_windows::set_dragging(&tw.window, true);
                        }
                        log("drag: pending -> torn off (single tab)");
                    } else {
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::DraggingInBar;
                        }
                        self.tab_bar_dirty = true;
                        log("drag: pending -> dragging in bar");
                    }
                }
            }
            DragPhase::DraggingInBar => {
                let sf = self.scale_factor;
                let tab_bar_h = (TAB_BAR_HEIGHT as f64 * sf).round();
                let controls_w = CONTROLS_ZONE_WIDTH as f64 * sf;
                let window_w = self
                    .windows
                    .get(&source_wid)
                    .map_or(0.0, |tw| tw.window.inner_size().width as f64);
                let x_max = window_w - controls_w;
                let y = position.y;
                let x = position.x;
                let outside_y = if y < 0.0 {
                    -y
                } else {
                    (y - tab_bar_h).max(0.0)
                };
                let outside_x = if x < 0.0 { -x } else { (x - x_max).max(0.0) };
                let outside = outside_y.max(outside_x);
                if outside >= TEAR_OFF_THRESHOLD {
                    log("drag: tearing off!");
                    self.tear_off_tab(tab_id, source_wid, position, event_loop);
                    if let Some(ref mut drag) = self.drag {
                        drag.phase = DragPhase::TornOff;
                    }
                    #[cfg(target_os = "windows")]
                    if let Some(drag) = &self.drag {
                        if let Some(tw) = self.windows.get(&drag.source_window) {
                            crate::platform_windows::set_dragging(&tw.window, true);
                        }
                    }
                    self.drag_visual_x = None;
                    self.tab_anim_offsets.remove(&source_wid);
                } else {
                    self.update_drag_in_bar(window_id, position, tab_id, mouse_off);
                }
            }
            DragPhase::TornOff => {
                let screen_cursor = self
                    .windows
                    .get(&window_id)
                    .and_then(|tw| tw.window.inner_position().ok())
                    .map(|ip| (ip.x as f64 + position.x, ip.y as f64 + position.y));

                if let Some((sx, sy)) = screen_cursor {
                    self.handle_torn_off_drag(tab_id, source_wid, grab_offset, sx, sy, mouse_off);
                }
            }
        }
    }

    /// Handle positioning and drop preview for a torn-off tab.
    fn handle_torn_off_drag(
        &mut self,
        tab_id: crate::tab::TabId,
        torn_wid: WindowId,
        grab_offset: PhysicalPosition<f64>,
        sx: f64,
        sy: f64,
        mouse_off: f64,
    ) {
        // Position torn-off window (even when hidden)
        if self.drop_preview.is_none() {
            if let Some(tw) = self.windows.get(&torn_wid) {
                let new_x = sx - grab_offset.x;
                let new_y = sy - grab_offset.y;
                tw.window.set_outer_position(PhysicalPosition::new(
                    new_x as i32,
                    new_y as i32,
                ));
            }
        }

        // Check if cursor is over another window's tab bar
        let new_target = self
            .find_window_at_cursor(torn_wid, sx, sy)
            .map(|(twid, scr_x)| {
                let idx = self.compute_drop_index(twid, scr_x);
                (twid, idx)
            });

        let old_preview = self.drop_preview;
        match (old_preview, new_target) {
            (None, Some((target_wid, idx))) => {
                // Entering target: move tab into target, hide torn-off window.
                if let Some(tw) = self.windows.get_mut(&torn_wid) {
                    tw.remove_tab(tab_id);
                }
                if let Some(tw) = self.windows.get_mut(&target_wid) {
                    tw.insert_tab_at(tab_id, idx);
                    tw.window.request_redraw();
                }
                if let Some(tw) = self.windows.get(&torn_wid) {
                    tw.window.set_visible(false);
                }
                self.drop_preview = Some((target_wid, idx));
                self.set_preview_visual(target_wid, sx, mouse_off);
            }
            (Some((old_twid, _)), Some((new_twid, new_idx))) => {
                if old_twid == new_twid {
                    // Same target: reorder tab within it.
                    if let Some(tw) = self.windows.get_mut(&new_twid) {
                        tw.remove_tab(tab_id);
                    }
                    let idx = self.compute_drop_index(new_twid, sx);
                    if let Some(tw) = self.windows.get_mut(&new_twid) {
                        tw.insert_tab_at(tab_id, idx);
                        tw.window.request_redraw();
                    }
                    self.drop_preview = Some((new_twid, idx));
                } else {
                    // Switched to different target window.
                    self.drag_visual_x = None;
                    self.tab_anim_offsets.remove(&old_twid);
                    if let Some(tw) = self.windows.get_mut(&old_twid) {
                        tw.remove_tab(tab_id);
                    }
                    if let Some(tw) = self.windows.get_mut(&new_twid) {
                        tw.insert_tab_at(tab_id, new_idx);
                        tw.window.request_redraw();
                    }
                    self.drop_preview = Some((new_twid, new_idx));
                    self.tab_bar_dirty = true;
                    self.render_window(old_twid);
                }
                self.set_preview_visual(new_twid, sx, mouse_off);
            }
            (Some((old_twid, _)), None) => {
                // Left target: undo preview, show torn-off window at cursor.
                self.drag_visual_x = None;
                self.tab_anim_offsets.remove(&old_twid);
                if let Some(tw) = self.windows.get_mut(&old_twid) {
                    tw.remove_tab(tab_id);
                }
                if let Some(tw) = self.windows.get_mut(&torn_wid) {
                    tw.add_tab(tab_id);
                }
                if let Some(tw) = self.windows.get(&torn_wid) {
                    let new_x = sx - grab_offset.x;
                    let new_y = sy - grab_offset.y;
                    tw.window.set_outer_position(PhysicalPosition::new(
                        new_x as i32,
                        new_y as i32,
                    ));
                    tw.window.set_visible(true);
                }
                self.drop_preview = None;
                self.tab_bar_dirty = true;
                self.render_window(old_twid);
            }
            (None, None) => {
                // Not over any target — nothing to do.
            }
        }
    }
}
