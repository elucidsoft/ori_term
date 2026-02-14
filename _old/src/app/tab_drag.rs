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

    /// Tear off a tab into a new single-tab window.
    ///
    /// Creates the window, positions it under the cursor, renders and shows it.
    /// Returns `(new_window_id, grab_offset)` so the caller can start the OS drag.
    /// Does NOT call `begin_os_drag` or `drag_window` — the caller handles that.
    pub(super) fn tear_off_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        event_loop: &ActiveEventLoop,
    ) -> Option<(WindowId, (i32, i32))> {
        // Remove tab from source window and clear drag overlay so the source
        // window doesn't render a ghost of the dragged tab.
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
            self.tab_bar_dirty = true;
            self.drag_visual_x = None;
            tw.window.request_redraw();
        }

        // The grab offset: where the cursor should appear within the new
        // window's client area. Accounts for the left margin (the tab in the
        // new single-tab window starts at TAB_LEFT_MARGIN, not at x=0) and
        // preserves the original Y click position so the cursor stays at the
        // exact spot the user grabbed.
        let left_margin = self.scale_px(TAB_LEFT_MARGIN) as f64;
        let offset_in_tab = self.drag.as_ref().map_or(75.0, |d| d.mouse_offset_in_tab);
        let grab_x = left_margin + offset_in_tab;
        let grab_y = self.drag.as_ref().map_or(
            TAB_BAR_HEIGHT as f64 * self.scale_factor / 2.0,
            |d| d.origin.y,
        );

        // Use the actual screen cursor position (Win32 GetCursorPos) rather
        // than computing from outer_position + client coords. This avoids
        // any coordinate system discrepancy from DWM invisible borders.
        #[cfg(target_os = "windows")]
        let screen_cursor = {
            let (sx, sy) = crate::platform_windows::cursor_screen_pos();
            Some((sx, sy))
        };
        #[cfg(not(target_os = "windows"))]
        let screen_cursor = self
            .windows
            .get(&source_wid)
            .and_then(|tw| tw.window.outer_position().ok())
            .map(|wp| {
                let pos = self
                    .cursor_pos
                    .get(&source_wid)
                    .copied()
                    .unwrap_or(PhysicalPosition::new(0.0, 0.0));
                (wp.x + pos.x as i32, wp.y + pos.y as i32)
            });

        let grab_offset = (grab_x as i32, grab_y as i32);

        // Create new frameless window at cursor position (hidden until first frame)
        let new_wid = self.create_window(event_loop, None, false)?;
        if let Some(tw) = self.windows.get_mut(&new_wid) {
            tw.add_tab(tab_id);
            // Position so cursor is at grab_offset within the client area.
            // No title bar offset needed — frameless window has no OS decoration.
            if let Some((sx, sy)) = screen_cursor {
                let win_x = sx - grab_offset.0;
                let win_y = sy - grab_offset.1;
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

        // If source window is empty, close it.
        let source_empty = self
            .windows
            .get(&source_wid)
            .is_some_and(|tw| tw.tabs.is_empty());
        if source_empty {
            self.windows.remove(&source_wid);
        }

        Some((new_wid, grab_offset))
    }

    /// Unified entry point for starting an OS tab drag with merge detection.
    ///
    /// Collects merge rects from other windows, configures the `WM_MOVING` handler
    /// via `begin_os_drag()`, sets `torn_off_pending`, and calls `drag_window()`.
    /// Used by both single-tab drag and multi-tab tear-off paths.
    #[cfg(target_os = "windows")]
    pub(super) fn begin_os_tab_drag(
        &mut self,
        wid: WindowId,
        tab_id: TabId,
        mouse_offset: f64,
        grab_offset: (i32, i32),
        skip_count: i32,
    ) {
        let merge_rects = self.collect_merge_rects(wid);
        if let Some(tw) = self.windows.get(&wid) {
            crate::platform_windows::begin_os_drag(
                &tw.window,
                crate::platform_windows::OsDragConfig {
                    grab_offset,
                    merge_rects,
                    skip_count,
                },
            );
        }
        self.torn_off_pending = Some((wid, tab_id, mouse_offset));
        if let Some(tw) = self.windows.get(&wid) {
            let _ = tw.window.drag_window();
        }
    }

    /// Collect tab bar zones from all windows except `exclude` for merge detection.
    ///
    /// Each zone is `[left, top, right, tab_bar_bottom]` in screen coordinates.
    /// The controls zone (minimize/maximize/close) is excluded from the right
    /// side so dragging over window controls doesn't trigger a merge.
    #[cfg(target_os = "windows")]
    fn collect_merge_rects(&self, exclude: WindowId) -> Vec<[i32; 4]> {
        let sf = self.scale_factor;
        let tab_bar_h = (TAB_BAR_HEIGHT as f64 * sf).round() as i32;
        let controls_w = (CONTROLS_ZONE_WIDTH as f64 * sf).round() as i32;

        let mut rects = Vec::new();
        for (&wid, tw) in &self.windows {
            if wid == exclude {
                continue;
            }
            if let Some((l, t, r, _b)) =
                crate::platform_windows::visible_frame_bounds(&tw.window)
            {
                rects.push([l, t, r - controls_w, t + tab_bar_h]);
            }
        }
        rects
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
        self.tab_anim_offsets.retain(|_wid, offsets| {
            let mut all_zero = true;
            for offset in offsets.iter_mut() {
                *offset *= decay;
                if offset.abs() < 0.5 {
                    *offset = 0.0;
                } else {
                    all_zero = false;
                }
            }
            if !all_zero {
                any_active = true;
            }
            !all_zero
        });

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

    /// Find a merge target using cursor screen coordinates (Chrome pattern).
    ///
    /// Checks if the cursor falls within any other window's tab bar zone,
    /// expanded by `magnetism` pixels vertically (Chrome's `kVerticalDetachMagnetism`).
    /// Returns `(target_window_id, cursor_screen_x)` for drop index calculation.
    #[cfg(target_os = "windows")]
    fn find_merge_target(
        &self,
        exclude: WindowId,
        cursor: (i32, i32),
        magnetism: i32,
    ) -> Option<(WindowId, f64)> {
        let (cx, cy) = cursor;
        let sf = self.scale_factor;
        let tab_bar_h = (TAB_BAR_HEIGHT as f64 * sf).round() as i32;
        let controls_w = (CONTROLS_ZONE_WIDTH as f64 * sf).round() as i32;

        for (&wid, tw) in &self.windows {
            if wid == exclude {
                continue;
            }
            if let Some((l, t, r, _b)) =
                crate::platform_windows::visible_frame_bounds(&tw.window)
            {
                let in_x = cx >= l && cx < r - controls_w;
                let in_y = cy >= t - magnetism && cy < t + tab_bar_h + magnetism;
                if in_x && in_y {
                    return Some((wid, cx as f64));
                }
            }
        }
        None
    }

    /// Check if a torn-off tab's OS drag ended and merge into a target window.
    ///
    /// Called from `about_to_wait()` each event loop iteration. Uses cursor-based
    /// merge detection: if the cursor is within another window's tab bar zone
    /// (± magnetism), merge the tab into that window. Otherwise the window stays
    /// where the OS placed it.
    #[cfg(target_os = "windows")]
    pub(super) fn check_torn_off_merge(&mut self) {
        use crate::log;

        let Some((torn_wid, tab_id, mouse_offset)) = self.torn_off_pending else {
            return;
        };
        let Some(tw) = self.windows.get(&torn_wid) else {
            log("merge-check: torn window gone, clearing pending");
            self.torn_off_pending = None;
            return;
        };
        let Some(result) = crate::platform_windows::take_os_drag_result(&tw.window) else {
            return;
        };
        self.torn_off_pending = None;

        let sf = self.scale_factor;
        // Chrome's kVerticalDetachMagnetism = 15 DIPs.
        let magnetism = (15.0 * sf).round() as i32;

        let (cursor, is_live_merge) = match result {
            crate::platform_windows::OsDragResult::MergeDetected { cursor } => {
                log(&format!(
                    "merge-check: WM_MOVING merge at cursor ({}, {})",
                    cursor.0, cursor.1,
                ));
                // WM_MOVING already confirmed cursor is inside a tab bar zone,
                // so no extra magnetism needed.
                (cursor, true)
            }
            crate::platform_windows::OsDragResult::DragEnded { cursor } => {
                log(&format!(
                    "merge-check: drag ended at cursor ({}, {})",
                    cursor.0, cursor.1,
                ));
                (cursor, false)
            }
        };

        // For live merge (WM_MOVING already validated), use 0 magnetism since
        // the cursor was confirmed inside the zone. For post-drag, use Chrome's
        // kVerticalDetachMagnetism for a more forgiving check.
        let merge_magnetism = if is_live_merge { 0 } else { magnetism };
        let target = self.find_merge_target(torn_wid, cursor, merge_magnetism);

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
                    tab.set_grid_dirty(true);
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

            // Chrome-style seamless drag: if the merge was triggered by
            // WM_MOVING (user is still holding the mouse button), start a
            // DraggingInBar state so the user can continue dragging the tab
            // within the target window (or tear off again) without releasing.
            if is_live_merge {
                self.begin_seamless_drag_after_merge(
                    target_wid, tab_id, cursor, mouse_offset, sf,
                );
                log("merge: seamless drag active in target window");
            }
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

    /// Synthesize a `DraggingInBar` state after a live merge so the user can
    /// continue dragging the tab without releasing the mouse button.
    ///
    /// This intentionally sets `self.left_mouse_down = true` — the input-tracking
    /// flag owned by `input_mouse.rs` — because the OS modal move loop consumed
    /// the original button-down event. Without this, the mouse-up that eventually
    /// arrives would have no matching down, causing the drag to be ignored.
    #[cfg(target_os = "windows")]
    fn begin_seamless_drag_after_merge(
        &mut self,
        target_wid: WindowId,
        tab_id: TabId,
        cursor: (i32, i32),
        mouse_offset: f64,
        sf: f64,
    ) {
        use crate::drag::{DragPhase, DragState};

        let Some(tw) = self.windows.get(&target_wid) else {
            return;
        };

        let (cx, cy) = cursor;
        // Convert screen cursor to target client coords.
        let (tgt_left, tgt_top) =
            crate::platform_windows::visible_frame_bounds(&tw.window)
                .map_or((0, 0), |(l, t, _, _)| (l, t));
        let local_x = cx as f64 - tgt_left as f64;
        let local_y = cy as f64 - tgt_top as f64;
        let client_pos = PhysicalPosition::new(local_x, local_y);

        // Store cursor position so subsequent events have context.
        self.cursor_pos.insert(target_wid, client_pos);

        // Create drag state in the target window.
        let mut drag = DragState::new(tab_id, target_wid, client_pos);
        drag.phase = DragPhase::DraggingInBar;
        drag.mouse_offset_in_tab = mouse_offset;
        self.drag = Some(drag);

        // Synthesize mouse-down: the OS modal move loop consumed the original
        // button-down, so input tracking must reflect that the button is held.
        self.left_mouse_down = true;

        // Set drag_visual_x so the tab renders at the cursor position
        // immediately — without this, the tab snaps to its slot index
        // until the next cursor_moved event.
        let layout = TabBarLayout::compute(
            tw.tabs.len(),
            tw.window.inner_size().width as usize,
            self.scale_factor,
            None,
        );
        let max_x = self.drag_max_x(target_wid, layout.tab_width as f32);
        let dragged_x =
            ((local_x - mouse_offset) as f32).clamp(0.0, max_x);
        self.drag_visual_x = Some((target_wid, dragged_x));

        // Suppress the stale WM_LBUTTONUP that the OS modal move
        // loop may deliver after ReleaseCapture.
        self.merge_drag_suppress_release = true;

        // Chrome-style vertical detach magnetism: after a merge, add extra Y
        // tolerance so the cursor needs to travel further before tearing off
        // again. Prevents immediate re-tear-off when the cursor was near the
        // bottom of the tab bar during merge.
        // Chrome uses kVerticalDetachMagnetism = 15 DIPs.
        self.tear_off_magnetism = 15.0 * sf;
    }

    /// Compute the grab offset for a single-tab window drag.
    ///
    /// Returns `(grab_x, grab_y)`: the cursor-to-window-origin offset
    /// using DWM visible frame bounds for accuracy.
    #[cfg(target_os = "windows")]
    pub(super) fn compute_single_tab_grab_offset(&self, wid: WindowId) -> (i32, i32) {
        let (sx, sy) = crate::platform_windows::cursor_screen_pos();
        let (wl, wt) = self
            .windows
            .get(&wid)
            .and_then(|tw| crate::platform_windows::visible_frame_bounds(&tw.window))
            .map_or_else(
                || {
                    self.windows
                        .get(&wid)
                        .and_then(|tw| tw.window.outer_position().ok())
                        .map_or((0, 0), |p| (p.x, p.y))
                },
                |(l, t, _, _)| (l, t),
            );
        (sx - wl, sy - wt)
    }
}
