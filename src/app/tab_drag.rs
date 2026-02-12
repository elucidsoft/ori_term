//! Chrome-style tab drag — tear-off, animations, drop preview.

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
                drag.grab_offset = PhysicalPosition::new(grab_x, grab_y);
            }

            // The drag continues via CursorMoved (DragPhase::TornOff)
            // until mouse release.
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
        let left_margin = (TAB_LEFT_MARGIN as f64 * sf).round();
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
        // Dragged tab pushes + and dropdown buttons; max clamp reserves room
        // for tab + both buttons before the window controls zone.
        let window_w = self
            .windows
            .get(&window_id)
            .map_or(0.0, |tw| tw.window.inner_size().width as f32);
        let controls_w = CONTROLS_ZONE_WIDTH as f32 * sf as f32;
        let new_tab_w = NEW_TAB_BUTTON_WIDTH as f32 * sf as f32;
        let dropdown_w = DROPDOWN_BUTTON_WIDTH as f32 * sf as f32;
        let max_x = window_w - controls_w - new_tab_w - dropdown_w - tab_wf as f32;
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
        for (wid, tw) in &self.windows {
            if tw.tabs.contains(&tab_id) {
                return Some(*wid);
            }
        }
        None
    }

    pub(super) fn find_window_at_cursor(
        &self,
        exclude: WindowId,
        screen_x: f64,
        screen_y: f64,
    ) -> Option<(WindowId, f64)> {
        for (&wid, tw) in &self.windows {
            if wid == exclude {
                continue;
            }
            let pos = match tw.window.outer_position() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let size = tw.window.inner_size();
            let wx = pos.x as f64;
            let wy = pos.y as f64;
            let sf = self.scale_factor;
            let tab_bar_h = TAB_BAR_HEIGHT as f64 * sf;
            let controls_w = CONTROLS_ZONE_WIDTH as f64 * sf;
            if screen_x >= wx
                && screen_x < wx + size.width as f64 - controls_w
                && screen_y >= wy
                && screen_y < wy + tab_bar_h
            {
                return Some((wid, screen_x));
            }
        }
        None
    }

    pub(super) fn compute_drop_index(&self, target_wid: WindowId, screen_x: f64) -> usize {
        let tw = match self.windows.get(&target_wid) {
            Some(tw) => tw,
            None => return 0,
        };
        let target_x = tw
            .window
            .outer_position()
            .map(|p| p.x as f64)
            .unwrap_or(0.0);
        let local_x = (screen_x - target_x) as usize;
        let layout = TabBarLayout::compute(
            tw.tabs.len(),
            tw.window.inner_size().width as usize,
            self.scale_factor,
            None,
        );
        let left_margin = (TAB_LEFT_MARGIN as f64 * self.scale_factor).round() as usize;
        let tab_x = local_x.saturating_sub(left_margin);
        let raw = (tab_x + layout.tab_width / 2) / layout.tab_width.max(1);
        raw.min(tw.tabs.len())
    }

    /// Set pixel-tracking visual for a preview tab being dragged into a target window.
    pub(super) fn set_preview_visual(
        &mut self,
        target_wid: WindowId,
        screen_x: f64,
        mouse_off: f64,
    ) {
        let sf = self.scale_factor as f32;
        let target_x = self
            .windows
            .get(&target_wid)
            .and_then(|tw| tw.window.outer_position().ok())
            .map_or(0.0, |p| p.x as f64);
        let local_x = screen_x - target_x;
        let dragged_x = (local_x - mouse_off) as f32;
        let left_margin = TAB_LEFT_MARGIN as f32 * sf;
        // Clamp to same bounds as update_drag_in_bar
        let (window_w, tab_wf) = self.windows.get(&target_wid).map_or((0.0, 0.0), |tw| {
            let layout = TabBarLayout::compute(
                tw.tabs.len(),
                tw.window.inner_size().width as usize,
                sf as f64,
                None,
            );
            (tw.window.inner_size().width as f32, layout.tab_width as f32)
        });
        let controls_w = CONTROLS_ZONE_WIDTH as f32 * sf;
        let new_tab_w = NEW_TAB_BUTTON_WIDTH as f32 * sf;
        let dropdown_w = DROPDOWN_BUTTON_WIDTH as f32 * sf;
        let max_x = window_w - controls_w - new_tab_w - dropdown_w - tab_wf;
        self.drag_visual_x = Some((target_wid, dragged_x.clamp(left_margin, max_x)));
    }
}
