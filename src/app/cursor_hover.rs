//! Cursor movement, hover effects, URL detection, selection drag, and resize edge detection.

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, ResizeDirection, WindowId};

use crate::drag::{DRAG_START_THRESHOLD, DragPhase, TEAR_OFF_THRESHOLD};
use crate::log;
use crate::selection::{self, SelectionMode, SelectionPoint, Side};
use crate::tab_bar::{
    CONTROLS_ZONE_WIDTH, GRID_PADDING_TOP, TAB_BAR_HEIGHT, TabBarHit, TabBarLayout,
};
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
    #[expect(
        clippy::too_many_lines,
        reason = "cursor dispatch handles multiple concerns: hyperlink hover, mouse reporting, selection drag, tab bar hover, and tab drag state machine"
    )]
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
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                // Check OSC 8 hyperlink first
                let osc8_uri: Option<String> = tab_id.and_then(|tid| {
                    let tab = self.tabs.get(&tid)?;
                    let grid = tab.grid();
                    let abs_row = Self::viewport_to_absolute(grid, line);
                    let row = grid.absolute_row(abs_row)?;
                    if col >= row.len() {
                        return None;
                    }
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

        // Mouse motion reporting (before selection drag).
        // When mouse reporting is active, motion in the grid is sent to PTY
        // and selection drag is suppressed. Tab bar hover and drag still work.
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

        // Selection drag: update selection end point (skip when mouse reporting handled motion)
        if self.left_mouse_down && !mouse_motion_reported {
            if let Some((col, line)) = self.pixel_to_cell(position) {
                let side = self.pixel_to_side(position);
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
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
                                let (w_start, w_end) =
                                    selection::word_boundaries(tab.grid(), abs_row, col);
                                let anchor = tab.selection.as_ref().map(|s| s.anchor);
                                let start_pt = SelectionPoint {
                                    row: abs_row,
                                    col: w_start,
                                    side: Side::Left,
                                };
                                let end_pt = SelectionPoint {
                                    row: abs_row,
                                    col: w_end,
                                    side: Side::Right,
                                };
                                if anchor.is_some_and(|a| start_pt < a) {
                                    Some(start_pt)
                                } else {
                                    Some(end_pt)
                                }
                            }
                            Some(SelectionMode::Line) => {
                                let drag_line_start =
                                    selection::logical_line_start(tab.grid(), abs_row);
                                let drag_line_end =
                                    selection::logical_line_end(tab.grid(), abs_row);
                                if sel_anchor_row.is_some_and(|ar| abs_row < ar) {
                                    Some(SelectionPoint {
                                        row: drag_line_start,
                                        col: 0,
                                        side: Side::Left,
                                    })
                                } else {
                                    Some(SelectionPoint {
                                        row: drag_line_end,
                                        col: grid_cols.saturating_sub(1),
                                        side: Side::Right,
                                    })
                                }
                            }
                            Some(_) => Some(SelectionPoint {
                                row: abs_row,
                                col,
                                side,
                            }),
                            None => None,
                        };

                        if let Some(new_end) = new_end {
                            tab.update_selection_end(new_end);
                        }
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            } else {
                // Mouse outside grid — auto-scroll
                let y = position.y as usize;
                let grid_top = (TAB_BAR_HEIGHT as f64 * self.scale_factor).round() as usize
                    + (GRID_PADDING_TOP as f64 * self.scale_factor).round() as usize;
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if y < grid_top {
                        // Above grid: scroll up into history
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.scroll_lines(1);
                        }
                    }
                    // Below grid: scroll down toward live
                    // (Only if display_offset > 0)
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let ch = self.glyphs.cell_height;
                        let grid_bottom = grid_top + tab.grid().lines * ch;
                        if y >= grid_bottom && tab.grid().display_offset > 0 {
                            tab.scroll_lines(-1);
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

        let tab_bar_h_hover = (TAB_BAR_HEIGHT as f64 * self.scale_factor).round() as usize;
        if y < tab_bar_h_hover {
            if let Some(tw) = self.windows.get(&window_id) {
                let twl = self
                    .tab_width_lock
                    .filter(|(wid, _)| *wid == window_id)
                    .map(|(_, w)| w);
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

        // Handle drag — extract values to avoid borrow conflicts with self
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

        if let Some((phase, tab_id, source_wid, grab_offset, dist, mouse_off)) = drag_action {
            match phase {
                DragPhase::Pending => {
                    if dist >= DRAG_START_THRESHOLD {
                        let is_single_tab = self
                            .windows
                            .get(&source_wid)
                            .is_some_and(|tw| tw.tabs.len() <= 1);
                        if is_single_tab {
                            // Single-tab window: skip DraggingInBar, go straight
                            // to TornOff so the whole window can be dropped onto
                            // another window's tab bar.
                            if let Some(ref mut drag) = self.drag {
                                drag.phase = DragPhase::TornOff;
                                drag.grab_offset = position;
                            }
                            // Suppress WM_DPICHANGED during manual positioning
                            #[cfg(target_os = "windows")]
                            if let Some(tw) = self.windows.get(&source_wid) {
                                crate::platform_windows::set_dragging(&tw.window, true);
                            }
                            log("drag: pending -> torn off (single tab)");
                        } else {
                            if let Some(ref mut drag) = self.drag {
                                drag.phase = DragPhase::DraggingInBar;
                            }
                            // Full rebuild so tab bar hides the dragged tab's slot.
                            // After this, position-only moves use the overlay fast path.
                            self.tab_bar_dirty = true;
                            log("drag: pending -> dragging in bar");
                        }
                    }
                }
                DragPhase::DraggingInBar => {
                    // Chrome-style tear-off: cursor leaves the draggable area.
                    // Y: above or below the tab bar by threshold.
                    // X: past the window edges (left=0, right=controls start).
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
                        // Suppress WM_DPICHANGED on the torn-off window
                        #[cfg(target_os = "windows")]
                        if let Some(drag) = &self.drag {
                            if let Some(tw) = self.windows.get(&drag.source_window) {
                                crate::platform_windows::set_dragging(&tw.window, true);
                            }
                        }
                        // Clear drag visuals — source window was already rendered
                        // inline by tear_off_tab with the tab removed.
                        self.drag_visual_x = None;
                        self.tab_anim_offsets.remove(&source_wid);
                    } else {
                        self.update_drag_in_bar(window_id, position, tab_id, mouse_off);
                    }
                }
                DragPhase::TornOff => {
                    // Convert cursor to screen coordinates using the window
                    // that actually sent this CursorMoved event (which has
                    // mouse capture — may be the original window, not the
                    // torn-off one).
                    let screen_cursor = self
                        .windows
                        .get(&window_id)
                        .and_then(|tw| tw.window.inner_position().ok())
                        .map(|ip| (ip.x as f64 + position.x, ip.y as f64 + position.y));

                    if let Some((sx, sy)) = screen_cursor {
                        let torn_wid = source_wid;

                        // Position torn-off window (even when hidden, so it
                        // reappears at the right spot).
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
                        let new_target =
                            self.find_window_at_cursor(torn_wid, sx, sy)
                                .map(|(twid, scr_x)| {
                                    let idx = self.compute_drop_index(twid, scr_x);
                                    (twid, idx)
                                });

                        let old_preview = self.drop_preview;
                        match (old_preview, new_target) {
                            (None, Some((target_wid, idx))) => {
                                // Entering target: move tab into target, hide
                                // torn-off window (Chrome-style preview).
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
                                // Set pixel-tracking for preview tab in target
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
                                    // Render old target inline to remove ghost tab
                                    self.tab_bar_dirty = true;
                                    self.render_window(old_twid);
                                }
                                self.set_preview_visual(new_twid, sx, mouse_off);
                            }
                            (Some((old_twid, _)), None) => {
                                // Left target: undo preview, show torn-off
                                // window at cursor.
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
                                // Render target window inline to remove the
                                // preview tab immediately (request_redraw is
                                // deferred on Windows during mouse capture).
                                self.tab_bar_dirty = true;
                                self.render_window(old_twid);
                            }
                            (None, None) => {
                                // Not over any target — nothing to do.
                            }
                        }
                    }
                }
            }
        }
    }
}
