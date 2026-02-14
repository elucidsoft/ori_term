//! Cursor movement dispatch — resize edges, hover, drag state machine.

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, ResizeDirection, WindowId};

use crate::drag::{DRAG_START_THRESHOLD, DragPhase, TEAR_OFF_THRESHOLD};
use crate::log;
use crate::tab::Tab;
use crate::tab_bar::{CONTROLS_ZONE_WIDTH, TAB_BAR_HEIGHT, TabBarHit, TabBarLayout};
use crate::term_mode::TermMode;

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
            if let Some(tid) = self.active_tab_id(window_id) {
                if let Some(tab) = self.tabs.get(&tid) {
                    tab.set_grid_dirty(true);
                }
            }
        }

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.set_cursor(cursor_icon);
            if hover_changed {
                tw.window.request_redraw();
            }
        }

        // Mouse motion reporting (before selection drag).
        // When the PTY handles motion, skip selection drag to avoid conflicts.
        let mouse_motion_reported = self.report_mouse_motion(window_id, position);
        if self.left_mouse_down && !mouse_motion_reported {
            self.update_selection_drag(window_id, position);
        }

        // Tab bar hover
        self.update_tab_bar_hover(window_id, position);

        // Tab drag state machine
        self.update_drag_state(window_id, position, event_loop);
    }

    /// Send mouse motion reports to the PTY when mouse tracking is active.
    ///
    /// Returns true if motion was reported (caller should skip selection drag).
    fn report_mouse_motion(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
    ) -> bool {
        if self.is_settings_window(window_id) || self.modifiers.shift_key() {
            return false;
        }
        let Some(tid) = self.active_tab_id(window_id) else {
            return false;
        };
        let mode = self
            .tabs
            .get(&tid)
            .map_or(TermMode::empty(), Tab::mode);
        let report_all = mode.contains(TermMode::MOUSE_ALL);
        let report_motion = mode.contains(TermMode::MOUSE_MOTION);
        if !(report_all || report_motion && self.left_mouse_down) {
            return false;
        }
        let Some((col, line)) = self.pixel_to_cell(position) else {
            return false;
        };
        let cell = (col, line);
        if self.last_mouse_cell != Some(cell) {
            self.last_mouse_cell = Some(cell);
            // Motion code: 32 + button (32 for left drag, 35 for no button).
            let code = if self.left_mouse_down { 32 } else { 35 };
            self.send_mouse_report(tid, code, col, line, true);
        }
        true
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

    /// Advance the tab drag state machine (`Pending` → `DraggingInBar` → OS drag).
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
                drag.distance_from_origin(position),
                drag.mouse_offset_in_tab,
            )
        });

        let Some((phase, tab_id, source_wid, dist, mouse_off)) = drag_action else {
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
                        // Single-tab window: hand off to OS-native drag so
                        // Windows snap (Aero Snap, right-click snap menu) works.
                        #[cfg(target_os = "windows")]
                        {
                            let grab_offset = self.compute_single_tab_grab_offset(source_wid);
                            self.drag = None;
                            self.begin_os_tab_drag(source_wid, tab_id, mouse_off, grab_offset, 0);
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            self.drag = None;
                            self.start_window_drag(source_wid);
                        }
                        log(&format!(
                            "drag: single tab -> OS window drag, torn_off_pending={source_wid:?}"
                        ));
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
                // Chrome-style magnetism: after a merge, extra Y tolerance
                // prevents immediate re-tear-off when the cursor was near
                // the bottom of the tab bar.
                let effective_bar_h = tab_bar_h + self.tear_off_magnetism;

                // Downward: distance below the effective tab bar boundary.
                let outside_down = (y - effective_bar_h).max(0.0);

                // Upward: on a borderless window the cursor is clamped to
                // y >= 0 by the screen edge, so negative-Y detection alone
                // is unreliable. Fall back to Y displacement from the drag
                // origin when the cursor is in the top portion of the bar.
                let outside_up = if y < 0.0 {
                    -y
                } else {
                    let origin_y =
                        self.drag.as_ref().map_or(y, |d| d.origin.y);
                    let disp = (origin_y - y).max(0.0);
                    if y < tab_bar_h * 0.5 { disp } else { 0.0 }
                };

                let outside_y = outside_up.max(outside_down);
                let outside_x = if x < 0.0 { -x } else { (x - x_max).max(0.0) };
                let outside = outside_y.max(outside_x);
                // Upward uses a lower threshold because the cursor range is
                // physically limited (can't go above screen top). Chrome uses
                // kVerticalDetachMagnetism = 15 DIPs; we use half the normal
                // threshold for the same reason.
                let threshold = if outside_up > outside_down && outside_up > outside_x {
                    TEAR_OFF_THRESHOLD * 0.5
                } else {
                    TEAR_OFF_THRESHOLD
                };
                if outside >= threshold {
                    log("drag: tearing off!");
                    #[allow(unused_variables, reason = "grab_offset used only on Windows")]
                    if let Some((new_wid, grab_offset)) =
                        self.tear_off_tab(tab_id, source_wid, event_loop)
                    {
                        self.drag = None;
                        self.drag_visual_x = None;
                        self.tear_off_magnetism = 0.0;
                        self.tab_anim_offsets.remove(&source_wid);
                        #[cfg(target_os = "windows")]
                        {
                            self.begin_os_tab_drag(new_wid, tab_id, mouse_off, grab_offset, 5);
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            self.start_window_drag(new_wid);
                        }
                        log(&format!(
                            "drag: tear-off -> OS window drag, torn_off_pending={new_wid:?}"
                        ));
                    }
                } else {
                    self.update_drag_in_bar(window_id, position, tab_id, mouse_off);
                }
            }
        }
    }
}
