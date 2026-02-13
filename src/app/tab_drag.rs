//! Chrome-style tab drag — tear-off, animations, OS drag merge.

use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::tab::TabId;
use crate::tab_bar::{
    CONTROLS_ZONE_WIDTH, DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT,
    TAB_LEFT_MARGIN, TabBarLayout,
};
use super::App;

impl App {
    /// Maximum drag X for a tab within a window's tab bar.
    ///
    /// Reserves space for the new-tab button, dropdown button, and window controls
    /// to the right of the dragged tab.
    fn drag_max_x(&self, window_id: WindowId, tab_width: f32) -> f32 {
        let sf = self.scale_factor as f32;
        let window_w = self
            .windows
            .get(&window_id)
            .map_or(0.0, |tw| tw.window.inner_size().width as f32);
        let controls_w = CONTROLS_ZONE_WIDTH as f32 * sf;
        let new_tab_w = NEW_TAB_BUTTON_WIDTH as f32 * sf;
        let dropdown_w = DROPDOWN_BUTTON_WIDTH as f32 * sf;
        window_w - controls_w - new_tab_w - dropdown_w - tab_width
    }

    pub(super) fn tear_off_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        cursor: PhysicalPosition<f64>,
        event_loop: &ActiveEventLoop,
    ) {
        // Remove tab from source window and clear drag overlay so the source
        // window doesn't render a ghost of the dragged tab.
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
            self.tab_bar_dirty = true;
            self.drag_visual_x = None;
            tw.window.request_redraw();
        }

        // Compute screen-space cursor position from source window
        let screen_cursor = self
            .windows
            .get(&source_wid)
            .and_then(|tw| tw.window.outer_position().ok())
            .map(|wp| (wp.x + cursor.x as i32, wp.y + cursor.y as i32));

        // The grab offset: where the cursor will be within the new window.
        // Use mouse_offset_in_tab so the window appears with the cursor at the
        // exact spot the user grabbed the tab.
        let grab_x = self.drag.as_ref().map_or(75.0, |d| d.mouse_offset_in_tab);
        let grab_y = TAB_BAR_HEIGHT as f64 * self.scale_factor / 2.0;

        // Create new frameless window at cursor position (hidden until first frame)
        if let Some(new_wid) = self.create_window(event_loop, None, false) {
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
            }
            // Render new window first (hidden), then show it, then render
            // source window LAST so its frame is the final presented one and
            // `last_rendered_window` points to the source — avoiding cache
            // invalidation delays on Windows where request_redraw may be
            // deferred while the torn-off window has mouse capture.
            self.render_window(new_wid);
            if let Some(tw) = self.windows.get(&new_wid) {
                tw.window.set_visible(true);
            }
            self.tab_bar_dirty = true;
            self.render_window(source_wid);

            // Update drag.source_window to the torn-off window so the
            // release handler uses the correct window id.
            if let Some(ref mut drag) = self.drag {
                drag.source_window = new_wid;
            }

            // The OS drag loop takes over from here. Merge check
            // happens in check_torn_off_merge() after WM_EXITSIZEMOVE.
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

    /// Move a tab from one window to another at the given index.
    #[cfg(target_os = "windows")]
    pub(super) fn relocate_tab(
        &mut self,
        tab_id: TabId,
        from_wid: WindowId,
        to_wid: WindowId,
        idx: usize,
    ) {
        if let Some(tw) = self.windows.get_mut(&from_wid) {
            tw.remove_tab(tab_id);
        }
        if let Some(tw) = self.windows.get_mut(&to_wid) {
            tw.insert_tab_at(tab_id, idx);
            tw.window.request_redraw();
        }
    }

    /// Decay tab animation offsets (time-based exponential decay).
    /// Returns `true` if any animation is still active and needs further ticks.
    pub(super) fn decay_tab_animations(&mut self) -> bool {
        if self.tab_anim_offsets.is_empty() {
            return false;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_anim_time).as_secs_f32();
        self.last_anim_time = now;
        let decay = (-dt * 15.0_f32).exp(); // ~67ms time constant

        let mut any_active = false;
        let mut finished = Vec::new();

        for (wid, offsets) in &mut self.tab_anim_offsets {
            let mut all_zero = true;
            for offset in offsets.iter_mut() {
                *offset *= decay;
                if offset.abs() < 0.5 {
                    *offset = 0.0;
                } else {
                    all_zero = false;
                }
            }
            if all_zero {
                finished.push(*wid);
            } else {
                any_active = true;
            }
        }

        for wid in finished {
            self.tab_anim_offsets.remove(&wid);
        }

        // Offsets changed — mark tab bar dirty so renderer rebuilds with new positions
        if any_active {
            self.tab_bar_dirty = true;
            for tw in self.windows.values() {
                tw.window.request_redraw();
            }
        }

        any_active
    }

    pub(super) fn update_drag_in_bar(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
        tab_id: TabId,
        mouse_offset_in_tab: f64,
    ) {
        let sf = self.scale_factor;
        let left_margin = self.scale_px(TAB_LEFT_MARGIN) as f64;
        let (tab_count, tab_w) = match self.windows.get(&window_id) {
            Some(tw) => {
                let layout = TabBarLayout::compute(
                    tw.tabs.len(),
                    tw.window.inner_size().width as usize,
                    sf,
                    None, // no lock during drag
                );
                (tw.tabs.len(), layout.tab_width)
            }
            None => return,
        };

        let tab_wf = tab_w as f64;

        // 1. Compute dragged tab visual X (pixel-perfect cursor tracking)
        let max_x = self.drag_max_x(window_id, tab_wf as f32);
        let dragged_x = ((position.x - mouse_offset_in_tab) as f32).clamp(0.0, max_x);

        // 2. Compute insertion index from cursor center
        let cursor_center = dragged_x as f64 + tab_wf / 2.0;
        let new_idx =
            ((cursor_center - left_margin) / tab_wf).clamp(0.0, (tab_count - 1) as f64) as usize;

        // 3. If index changed, swap tab in the model.
        // Chrome-style: displaced tabs snap to new positions immediately during
        // drag (no dodge animation). Animation only applies on drag-end.
        let mut swapped = false;
        if let Some(tw) = self.windows.get_mut(&window_id) {
            if let Some(current_idx) = tw.tab_index(tab_id) {
                if current_idx != new_idx {
                    tw.tabs.remove(current_idx);
                    tw.tabs.insert(new_idx, tab_id);
                    tw.active_tab = new_idx;
                    self.tab_anim_offsets.remove(&window_id);
                    swapped = true;
                }
            }
        }

        // 4. Store dragged_x for FrameParams
        self.drag_visual_x = Some((window_id, dragged_x));

        // 5. Request redraw. Only mark tab bar dirty on actual tab swap (full
        // rebuild needed for new slot positions). Position-only moves use the
        // renderer's overlay-only fast path — skips grid + tab bar rebuild.
        if swapped {
            self.tab_bar_dirty = true;
        }
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    /// Populate merge candidate rects and mark the window as torn-off for live
    /// merge detection during the OS drag loop.
    ///
    /// Collects tab bar rects (DWM visible bounds) from all other windows and
    /// stores them in the dragged window's `SnapData`. The `WM_MOVING` handler
    /// checks these during the OS move loop and ends the loop early when the
    /// dragged window's tab bar overlaps a candidate.
    #[cfg(target_os = "windows")]
    pub(super) fn setup_merge_detection(&self, drag_wid: WindowId) {
        let sf = self.scale_factor;
        let tab_bar_h = (TAB_BAR_HEIGHT as f64 * sf).round() as i32;

        let mut rects = Vec::new();
        for (&wid, tw) in &self.windows {
            if wid == drag_wid {
                continue;
            }
            if let Some((l, t, r, _b)) =
                crate::platform_windows::visible_frame_bounds(&tw.window)
            {
                rects.push([l, t, r, t + tab_bar_h]);
            }
        }

        if let Some(tw) = self.windows.get(&drag_wid) {
            crate::platform_windows::set_torn_off(&tw.window, true);
            crate::platform_windows::set_merge_rects(&tw.window, rects);
        }
    }

    pub(super) fn window_containing_tab(&self, tab_id: TabId) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|(_, tw)| tw.tabs.contains(&tab_id))
            .map(|(&wid, _)| wid)
    }

    #[cfg(target_os = "windows")]
    pub(super) fn compute_drop_index(&self, target_wid: WindowId, screen_x: f64) -> usize {
        let tw = match self.windows.get(&target_wid) {
            Some(tw) => tw,
            None => return 0,
        };
        // Use visible frame bounds to get accurate X offset.
        let target_x =
            crate::platform_windows::visible_frame_bounds(&tw.window)
                .map_or_else(
                    || tw.window.outer_position().map(|p| p.x as f64).unwrap_or(0.0),
                    |(l, _, _, _)| l as f64,
                );
        let local_x = (screen_x - target_x) as usize;
        let layout = TabBarLayout::compute(
            tw.tabs.len(),
            tw.window.inner_size().width as usize,
            self.scale_factor,
            None,
        );
        let left_margin = self.scale_px(TAB_LEFT_MARGIN);
        let tab_x = local_x.saturating_sub(left_margin);
        let raw = (tab_x + layout.tab_width / 2) / layout.tab_width.max(1);
        raw.min(tw.tabs.len())
    }

    /// Check if a torn-off tab's OS drag ended and merge into a target window.
    ///
    /// Called from `about_to_wait()` each event loop iteration. When the OS move
    /// loop (`drag_window()`) ends, `WM_EXITSIZEMOVE` captures the cursor position.
    /// If the torn-off window's tab bar overlaps another window's tab bar, merge
    /// the tab into that window. Otherwise the window stays where the OS placed it.
    ///
    /// Uses window-to-window tab bar overlap rather than cursor position because
    /// the cursor is at the grab offset within the torn-off window, not necessarily
    /// inside the target's narrow tab bar zone.
    #[cfg(target_os = "windows")]
    pub(super) fn check_torn_off_merge(&mut self) {
        use crate::log;

        let Some((torn_wid, tab_id)) = self.torn_off_pending else {
            return;
        };
        let Some(tw) = self.windows.get(&torn_wid) else {
            log("merge-check: torn window gone, clearing pending");
            self.torn_off_pending = None;
            return;
        };
        let Some((cx, cy)) = crate::platform_windows::take_drag_ended(&tw.window) else {
            return;
        };
        self.torn_off_pending = None;
        log(&format!("merge-check: drag ended at cursor ({cx}, {cy})"));

        // Check if WM_MOVING detected overlap during the drag. If so, use
        // the proposed window rect captured at that moment — the window has
        // since snapped back after ReleaseCapture, making current positions
        // unreliable.
        let merge_proposed = self
            .windows
            .get(&torn_wid)
            .and_then(|tw| crate::platform_windows::take_merge_detected(&tw.window));

        let sf = self.scale_factor;
        let tab_bar_h = TAB_BAR_HEIGHT as f64 * sf;
        let controls_w = CONTROLS_ZONE_WIDTH as f64 * sf;

        let target = if let Some((pl, pt, pr, _pb)) = merge_proposed {
            // WM_MOVING detected overlap — find target using the proposed rect
            // (the position the OS was about to place the window).
            log(&format!(
                "merge-check: WM_MOVING triggered, proposed=({pl},{pt},{pr})"
            ));
            self.find_merge_target_by_proposed_rect(
                torn_wid, pl, pt, pr, tab_bar_h, controls_w,
            )
        } else {
            // Normal drag end — use window-to-window tab bar overlap.
            self.find_merge_target_by_overlap(torn_wid, tab_bar_h, controls_w)
        };

        if let Some((target_wid, screen_x)) = target {
            let idx = self.compute_drop_index(target_wid, screen_x);
            self.relocate_tab(tab_id, torn_wid, target_wid, idx);
            log(&format!(
                "merge: tab {tab_id:?} into window {target_wid:?} at index {idx}"
            ));

            // Resize tab to fit the target window.
            if let Some(ttw) = self.windows.get(&target_wid) {
                let size = ttw.window.inner_size();
                let (cols, rows) = self.grid_dims_for_size(size.width, size.height);
                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    tab.resize(cols, rows, size.width as u16, size.height as u16);
                    tab.grid_dirty = true;
                }
            }

            // Close the now-empty torn-off window.
            self.windows.remove(&torn_wid);
            self.tab_bar_dirty = true;

            // Activate the target window (Chrome: `attached_context_->GetWidget()->Activate()`)
            // so the merged tab is visible to the user, not hidden behind other windows.
            if let Some(ttw) = self.windows.get(&target_wid) {
                ttw.window.focus_window();
            }
            self.render_window(target_wid);
        } else {
            // No merge target found — ensure the torn-off window is visible.
            // WM_MOVING may have hidden it via raw Win32 ShowWindow(SW_HIDE),
            // so we must use raw ShowWindow(SW_SHOW) to undo it (winit's
            // set_visible may not know the window was hidden).
            if let Some(tw) = self.windows.get(&torn_wid) {
                crate::platform_windows::show_window(&tw.window);
                tw.window.request_redraw();
            }
            log("merge-check: no target window found, keeping torn-off");
        }
    }

    /// Find a merge target using the proposed window rect from `WM_MOVING`.
    ///
    /// The proposed rect is where the OS was about to place the torn-off window
    /// at the moment `WM_MOVING` detected overlap. Uses rect-to-rect tab bar
    /// overlap — the same logic as `find_merge_target_by_overlap` but with the
    /// proposed position instead of the current (snapped-back) position.
    #[cfg(target_os = "windows")]
    fn find_merge_target_by_proposed_rect(
        &self,
        exclude: WindowId,
        prop_left: i32,
        prop_top: i32,
        prop_right: i32,
        tab_bar_h: f64,
        controls_w: f64,
    ) -> Option<(WindowId, f64)> {
        use crate::log;

        let torn_bar_top = prop_top as f64;
        let torn_bar_bot = prop_top as f64 + tab_bar_h;

        for (&wid, tw) in &self.windows {
            if wid == exclude {
                continue;
            }
            let (wx, wy, wr) = if let Some((l, t, r, _)) =
                crate::platform_windows::visible_frame_bounds(&tw.window)
            {
                (l as f64, t as f64, r as f64)
            } else {
                let p = match tw.window.outer_position() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let s = tw.window.inner_size();
                (p.x as f64, p.y as f64, p.x as f64 + s.width as f64)
            };

            let tgt_bar_top = wy;
            let tgt_bar_bot = wy + tab_bar_h;
            let y_overlap = torn_bar_top < tgt_bar_bot && torn_bar_bot > tgt_bar_top;

            let tgt_right = wr - controls_w;
            let x_overlap = (prop_right as f64) > wx && (prop_left as f64) < tgt_right;

            log(&format!(
                "merge-check(proposed): candidate {wid:?} visible=({},{},{}), \
                 y_overlap={y_overlap} x_overlap={x_overlap}",
                wx as i32, wy as i32, wr as i32,
            ));

            if y_overlap && x_overlap {
                let ol = (prop_left as f64).max(wx);
                let or_ = (prop_right as f64).min(tgt_right);
                let center_x = f64::midpoint(ol, or_);
                return Some((wid, center_x));
            }
        }
        None
    }

    /// Find a merge target using window-to-window tab bar overlap.
    ///
    /// Used when the drag ended normally (user released mouse) without
    /// `WM_MOVING` triggering. Checks if the torn-off window's tab bar
    /// overlaps any other window's tab bar.
    #[cfg(target_os = "windows")]
    fn find_merge_target_by_overlap(
        &self,
        torn_wid: WindowId,
        tab_bar_h: f64,
        controls_w: f64,
    ) -> Option<(WindowId, f64)> {
        use crate::log;

        let torn_bounds = self.windows.get(&torn_wid).and_then(|tw| {
            crate::platform_windows::visible_frame_bounds(&tw.window).or_else(|| {
                let p = tw.window.outer_position().ok()?;
                let s = tw.window.inner_size();
                Some((p.x, p.y, p.x + s.width as i32, p.y + s.height as i32))
            })
        });
        let (tl, tt, tr, _tb) = torn_bounds?;
        log(&format!(
            "merge-check(overlap): torn window visible=({tl},{tt},{tr})"
        ));

        for (&wid, tw) in &self.windows {
            if wid == torn_wid {
                continue;
            }
            let (wx, wy, wr) = if let Some((l, t, r, _)) =
                crate::platform_windows::visible_frame_bounds(&tw.window)
            {
                (l as f64, t as f64, r as f64)
            } else {
                let p = match tw.window.outer_position() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let s = tw.window.inner_size();
                (p.x as f64, p.y as f64, p.x as f64 + s.width as f64)
            };

            let torn_bar_top = tt as f64;
            let torn_bar_bot = tt as f64 + tab_bar_h;
            let tgt_bar_top = wy;
            let tgt_bar_bot = wy + tab_bar_h;
            let y_overlap = torn_bar_top < tgt_bar_bot && torn_bar_bot > tgt_bar_top;

            let tgt_right = wr - controls_w;
            let x_overlap = (tr as f64) > wx && (tl as f64) < tgt_right;

            log(&format!(
                "merge-check(overlap): candidate {wid:?} visible=({},{},{}), \
                 y_overlap={y_overlap} x_overlap={x_overlap}",
                wx as i32, wy as i32, wr as i32,
            ));

            if y_overlap && x_overlap {
                let ol = (tl as f64).max(wx);
                let or_ = (tr as f64).min(tgt_right);
                let center_x = f64::midpoint(ol, or_);
                return Some((wid, center_x));
            }
        }
        None
    }
}
