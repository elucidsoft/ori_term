//! Mouse input handling — click dispatch, drag finalization, scrolling.

use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::clipboard;
use crate::context_menu;
use crate::drag::{DragPhase, DragState};
use crate::selection::{self, Selection};
use crate::tab_bar::{
    NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TabBarHit, TabBarLayout,
};
use crate::term_mode::TermMode;
use crate::window::TermWindow;

use super::{App, DOUBLE_CLICK_MS, SCROLL_LINES};

impl App {
    #[allow(clippy::too_many_lines, reason = "event dispatch table with inline handlers")]
    pub(super) fn handle_mouse_input(
        &mut self,
        window_id: WindowId,
        state: ElementState,
        button: MouseButton,
        event_loop: &ActiveEventLoop,
    ) {
        let pos = self
            .cursor_pos
            .get(&window_id)
            .copied()
            .unwrap_or(PhysicalPosition::new(0.0, 0.0));

        let x = pos.x as usize;
        let y = pos.y as usize;

        // Context menu interaction: if a menu is open, handle clicks.
        if self.context_menu.is_some() && state == ElementState::Pressed {
            let menu = self.context_menu.as_ref().unwrap();
            let clicked_item = menu.hit_test(pos.x as f32, pos.y as f32).and_then(|idx| {
                let entry = menu.entries.get(idx)?;
                match entry {
                    context_menu::MenuEntry::Item { action, .. }
                    | context_menu::MenuEntry::Check { action, .. } => Some(action.clone()),
                    context_menu::MenuEntry::Separator => None,
                }
            });
            let clicked_inside = self
                .context_menu
                .as_ref()
                .is_some_and(|m| m.contains(pos.x as f32, pos.y as f32));

            // Always close the menu
            self.context_menu = None;
            self.tab_bar_dirty = true;
            if let Some(tw) = self.windows.get(&window_id) {
                tw.window.request_redraw();
            }

            if let Some(action) = clicked_item {
                self.dispatch_context_action(action, event_loop);
                return;
            }
            if clicked_inside {
                // Clicked on separator or non-actionable area — consume
                return;
            }
            // Clicked outside the menu — fall through to process the click normally
        }

        // Mouse reporting: if any mouse mode is active and Shift is NOT held,
        // report to PTY and skip normal handling (Shift overrides mouse reporting
        // so the user can still select text).
        if !self.modifiers.shift_key() && !self.is_settings_window(window_id) {
            let tab_id = self
                .windows
                .get(&window_id)
                .and_then(TermWindow::active_tab_id);
            if let Some(tid) = tab_id {
                let mouse_active = self
                    .tabs
                    .get(&tid)
                    .is_some_and(|t| t.mode.intersects(TermMode::ANY_MOUSE));
                if mouse_active {
                    if let Some((col, line)) = self.pixel_to_cell(pos) {
                        let btn_code = match button {
                            MouseButton::Left => 0u8,
                            MouseButton::Middle => 1,
                            MouseButton::Right => 2,
                            _ => return,
                        };
                        let pressed = state == ElementState::Pressed;
                        let report_code = if pressed { btn_code } else { 3 };
                        // Reset motion dedup on press/release
                        self.last_mouse_cell = if pressed { Some((col, line)) } else { None };
                        // Track left_mouse_down for motion reporting
                        if button == MouseButton::Left {
                            self.left_mouse_down = pressed;
                        }
                        self.send_mouse_report(tid, report_code, col, line, pressed);
                    }
                    return;
                }
            }
        }

        // Right-click handling
        if button == MouseButton::Right {
            if state == ElementState::Released {
                let tab_bar_h = self.scale_px(TAB_BAR_HEIGHT);
                if y < tab_bar_h {
                    // Right-click in tab bar → context menu overlay
                    if let Some(tw) = self.windows.get(&window_id) {
                        let twl = self.tab_width_lock_for(window_id);
                        let layout = TabBarLayout::compute(
                            tw.tabs.len(),
                            tw.window.inner_size().width as usize,
                            self.scale_factor,
                            twl,
                        );
                        let hit = layout.hit_test(x, y, self.scale_factor);
                        let s = self.scale_factor as f32;
                        let menu_pos = (pos.x as f32, pos.y as f32);
                        let mut menu = match hit {
                            TabBarHit::Tab(idx) | TabBarHit::CloseTab(idx) => {
                                context_menu::build_tab_menu(menu_pos, idx, s)
                            }
                            _ => context_menu::build_tab_bar_menu(menu_pos, s),
                        };
                        menu.layout(&mut self.ui_glyphs);
                        self.context_menu = Some(menu);
                        self.tab_bar_dirty = true;
                        tw.window.request_redraw();
                    }
                } else {
                    // Right-click in grid area → copy if selection, paste if not
                    let tab_id = self
                        .windows
                        .get(&window_id)
                        .and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        let has_selection =
                            self.tabs.get(&tid).is_some_and(|t| t.selection.is_some());
                        if has_selection {
                            if let Some(tab) = self.tabs.get(&tid) {
                                if let Some(ref sel) = tab.selection {
                                    let text = selection::extract_text(tab.grid(), sel);
                                    if !text.is_empty() {
                                        clipboard::set_text(&text);
                                    }
                                }
                            }
                            if let Some(tab) = self.tabs.get_mut(&tid) {
                                tab.clear_selection();
                            }
                            if let Some(tw) = self.windows.get(&window_id) {
                                tw.window.request_redraw();
                            }
                        } else {
                            self.paste_from_clipboard(window_id);
                        }
                    }
                }
            }
            return;
        }

        if button != MouseButton::Left {
            return;
        }

        match state {
            ElementState::Pressed => {
                // If clicking in settings window, handle separately
                if self.is_settings_window(window_id) {
                    self.handle_settings_mouse(window_id, x, y);
                    return;
                }

                // Check resize borders first
                if let Some(direction) = self.resize_direction_at(window_id, pos) {
                    if let Some(tw) = self.windows.get(&window_id) {
                        let _ = tw.window.drag_resize_window(direction);
                    }
                    return;
                }

                let tab_bar_h = self.scale_px(TAB_BAR_HEIGHT);
                if y < tab_bar_h {
                    let tw = match self.windows.get(&window_id) {
                        Some(tw) => tw,
                        None => return,
                    };
                    let twl = self.tab_width_lock_for(window_id);
                    let layout = TabBarLayout::compute(
                        tw.tabs.len(),
                        tw.window.inner_size().width as usize,
                        self.scale_factor,
                        twl,
                    );
                    let hit = layout.hit_test(x, y, self.scale_factor);

                    match hit {
                        TabBarHit::NewTab => {
                            self.new_tab_in_window(window_id);
                        }
                        TabBarHit::DropdownButton => {
                            // Show dropdown menu overlay below the button
                            if let Some(tw) = self.windows.get(&window_id) {
                                let s = self.scale_factor as f32;
                                let bar_w = tw.window.inner_size().width as usize;
                                let tab_count = tw.tabs.len();
                                let twl = self.tab_width_lock_for(window_id);
                                let btn_layout = TabBarLayout::compute(
                                    tab_count,
                                    bar_w,
                                    self.scale_factor,
                                    twl,
                                );
                                let tabs_end = self.scale_px(TAB_LEFT_MARGIN)
                                    + tab_count * btn_layout.tab_width;
                                let menu_x =
                                    (tabs_end + self.scale_px(NEW_TAB_BUTTON_WIDTH)) as f32;
                                let menu_y = self.scale_px(TAB_BAR_HEIGHT) as f32;
                                let scheme = self.active_scheme;
                                let mut menu =
                                    context_menu::build_dropdown_menu((menu_x, menu_y), scheme, s);
                                menu.layout(&mut self.ui_glyphs);
                                self.context_menu = Some(menu);
                                self.tab_bar_dirty = true;
                                tw.window.request_redraw();
                            }
                        }
                        TabBarHit::CloseTab(idx) => {
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            // Chrome-style width lock: freeze tab width so close
                            // buttons stay under cursor during rapid closes.
                            self.tab_width_lock = Some((window_id, layout.tab_width));
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                self.close_tab(tab_id, event_loop);
                            }
                        }
                        TabBarHit::Minimize => {
                            if let Some(tw) = self.windows.get(&window_id) {
                                tw.window.set_minimized(true);
                            }
                        }
                        TabBarHit::Maximize => {
                            if let Some(tw) = self.windows.get_mut(&window_id) {
                                let new_max = !tw.is_maximized;
                                tw.is_maximized = new_max;
                                tw.window.set_maximized(new_max);
                                tw.window.request_redraw();
                            }
                        }
                        TabBarHit::CloseWindow => {
                            self.close_window(window_id, event_loop);
                        }
                        TabBarHit::DragArea => {
                            // Check for double-click to toggle maximize
                            let now = Instant::now();
                            let is_double = self.last_click_time.is_some_and(|t| {
                                now.duration_since(t).as_millis() < DOUBLE_CLICK_MS
                            }) && self.last_click_window == Some(window_id);

                            if is_double {
                                // Double-click: toggle maximize
                                self.last_click_time = None;
                                self.last_click_window = None;
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    let new_max = !tw.is_maximized;
                                    tw.is_maximized = new_max;
                                    tw.window.set_maximized(new_max);
                                    tw.window.request_redraw();
                                }
                            } else {
                                // Single click: start window drag
                                self.last_click_time = Some(now);
                                self.last_click_window = Some(window_id);
                                self.start_window_drag(window_id, pos);
                            }
                        }
                        TabBarHit::Tab(idx) => {
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                let twl = self.tab_width_lock_for(window_id);
                                let layout = TabBarLayout::compute(
                                    tw.tabs.len(),
                                    tw.window.inner_size().width as usize,
                                    self.scale_factor,
                                    twl,
                                );
                                let left_margin = self.scale_px(TAB_LEFT_MARGIN) as f64;
                                let tab_left = left_margin + idx as f64 * layout.tab_width as f64;
                                let offset_in_tab = pos.x - tab_left;
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    tw.active_tab = idx;
                                    self.tab_bar_dirty = true;
                                    tw.window.request_redraw();
                                }
                                let mut drag = DragState::new(tab_id, window_id, pos, idx);
                                drag.mouse_offset_in_tab = offset_in_tab;
                                self.drag = Some(drag);
                            }
                        }
                        TabBarHit::None => {}
                    }
                } else {
                    // Click in grid area — handle selection
                    self.handle_grid_press(window_id, pos);
                }
            }
            ElementState::Released => {
                // Finalize selection and auto-copy
                if self.left_mouse_down {
                    self.left_mouse_down = false;
                    let tab_id = self
                        .windows
                        .get(&window_id)
                        .and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            if tab.selection.as_ref().is_some_and(Selection::is_empty) {
                                tab.clear_selection();
                            } else if self.config.behavior.copy_on_select {
                                if let Some(ref sel) = tab.selection {
                                    let text = selection::extract_text(tab.grid(), sel);
                                    if !text.is_empty() {
                                        clipboard::set_text(&text);
                                    }
                                }
                            } else {
                                // copy_on_select disabled — keep selection visible
                            }
                        }
                    }
                }

                if let Some(drag) = self.drag.take() {
                    // drag.source_window was updated to the torn-off window
                    // during tear_off_tab — use it, not window_id
                    let torn_wid = drag.source_window;
                    // Re-enable DPI handling now that the drag is over
                    #[cfg(target_os = "windows")]
                    if drag.phase == DragPhase::TornOff {
                        if let Some(tw) = self.windows.get(&torn_wid) {
                            crate::platform_windows::set_dragging(&tw.window, false);
                        }
                    }
                    match drag.phase {
                        DragPhase::TornOff => {
                            // Clear drag visuals for any target window
                            self.drag_visual_x = None;
                            if let Some((target_wid, _)) = self.drop_preview.take() {
                                self.tab_anim_offsets.remove(&target_wid);
                                // Preview active — tab is already in the
                                // target window. Finalize: hide and close the
                                // (empty) torn-off window, render target inline.
                                if let Some(tw) = self.windows.get(&torn_wid) {
                                    tw.window.set_visible(false);
                                }
                                let source_empty = self
                                    .windows
                                    .get(&torn_wid)
                                    .is_some_and(|tw| tw.tabs.is_empty());
                                if source_empty {
                                    self.windows.remove(&torn_wid);
                                }
                                self.tab_bar_dirty = true;
                                self.render_window(target_wid);
                            }
                            // No preview — torn-off window stays as-is.
                        }
                        DragPhase::DraggingInBar | DragPhase::Pending => {
                            // Clear drag visuals and rebuild tab bar (show tab at slot again)
                            self.drag_visual_x = None;
                            self.tab_anim_offsets.remove(&drag.source_window);
                            self.tab_bar_dirty = true;
                            if let Some(tw) = self.windows.get(&drag.source_window) {
                                tw.window.request_redraw();
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn handle_mouse_wheel(&mut self, window_id: WindowId, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                if y > 0.0 {
                    SCROLL_LINES as i32
                } else {
                    -(SCROLL_LINES as i32)
                }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                let cell_h = self.glyphs.cell_height as f64;
                if cell_h > 0.0 {
                    (pos.y / cell_h).round() as i32
                } else {
                    0
                }
            }
        };

        if lines == 0 {
            return;
        }

        let tab_id = self
            .windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id);
        let Some(tid) = tab_id else { return };

        // Mouse reporting: scroll events sent to PTY when mouse mode active
        if !self.modifiers.shift_key() {
            let mouse_active = self
                .tabs
                .get(&tid)
                .is_some_and(|t| t.mode.intersects(TermMode::ANY_MOUSE));
            if mouse_active {
                let pos = self
                    .cursor_pos
                    .get(&window_id)
                    .copied()
                    .unwrap_or(PhysicalPosition::new(0.0, 0.0));
                if let Some((col, line)) = self.pixel_to_cell(pos) {
                    let btn = if lines > 0 { 64u8 } else { 65 };
                    let count = lines.unsigned_abs() as usize;
                    for _ in 0..count {
                        self.send_mouse_report(tid, btn, col, line, true);
                    }
                }
                return;
            }
        }

        // Alternate scroll: convert scroll to arrow keys in alt screen
        if !self.modifiers.shift_key() {
            let alt_scroll = self.tabs.get(&tid).is_some_and(|t| {
                t.mode.contains(TermMode::ALT_SCREEN) && t.mode.contains(TermMode::ALTERNATE_SCROLL)
            });
            if alt_scroll {
                let app_cursor = self
                    .tabs
                    .get(&tid)
                    .is_some_and(|t| t.mode.contains(TermMode::APP_CURSOR));
                let (up, down) = if app_cursor {
                    (b"\x1bOA" as &[u8], b"\x1bOB" as &[u8])
                } else {
                    (b"\x1b[A" as &[u8], b"\x1b[B" as &[u8])
                };
                let seq = if lines > 0 { up } else { down };
                let count = lines.unsigned_abs() as usize;
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    for _ in 0..count {
                        tab.send_pty(seq);
                    }
                }
                return;
            }
        }

        // Normal scrollback
        if let Some(tab) = self.tabs.get_mut(&tid) {
            tab.scroll_lines(lines);
        }
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }
}
