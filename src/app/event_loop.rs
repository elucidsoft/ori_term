//! winit event loop — `ApplicationHandler` impl + window effects.

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use crate::config;
use crate::key_encoding::{self, KeyEventType};
use crate::keybindings;
use crate::log;
use crate::tab::{Tab, TermEvent};
use crate::term_mode::TermMode;

use super::{App, build_modifiers};

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.first_window_created {
            return;
        }
        self.first_window_created = true;

        // Load saved window position before creating the window so we can
        // apply it before making the window visible (avoids gray flash from
        // moving a visible window).
        let saved_pos = config::WindowState::load();

        if let Some(wid) = self.create_window(event_loop, saved_pos.as_ref(), true) {
            // Query actual DPI scale factor and reload fonts if needed
            if let Some(tw) = self.windows.get(&wid) {
                let sf = tw.window.scale_factor();
                if (sf - self.scale_factor).abs() > 0.01 {
                    self.handle_scale_factor_changed(wid, sf);
                }
            }
            self.new_tab_in_window(wid);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::PtyOutput(tab_id, data) => {
                log(&format!("pty_output: tab={tab_id:?} len={}", data.len()));
                self.pty_event_count += 1;
                self.pty_bytes_received += data.len() as u64;

                self.cursor_blink_reset = Instant::now();

                // Single lock: process output, capture title change, bell, and
                // notifications all at once.
                let Some(tab) = self.tabs.get_mut(&tab_id) else {
                    return;
                };
                let (title_changed, bell_active, notifications) =
                    tab.process_output_batch(&data);
                if title_changed {
                    self.tab_bar_dirty = true;
                }

                // Process bell state: if this tab rang the bell and is NOT
                // active, set the bell badge so the tab bar shows an indicator.
                let is_active = self
                    .window_containing_tab(tab_id)
                    .and_then(|wid| self.windows.get(&wid))
                    .is_some_and(|tw| tw.active_tab_id() == Some(tab_id));
                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    if bell_active && !is_active {
                        tab.has_bell_badge = true;
                        self.tab_bar_dirty = true;
                    }
                    if is_active && tab.has_bell_badge {
                        tab.has_bell_badge = false;
                    }
                }
                for notif in notifications {
                    if notif.title.is_empty() {
                        log(&format!("notification: {}", notif.body));
                    } else {
                        log(&format!("notification: {} — {}", notif.title, notif.body));
                    }
                }

                self.url_cache.invalidate();
                if let Some(wid) = self.window_containing_tab(tab_id) {
                    if let Some(tw) = self.windows.get(&wid) {
                        if tw.active_tab_id() == Some(tab_id) || bell_active {
                            self.pending_redraw.insert(wid);
                        }
                    }
                }
            }
            TermEvent::PtyExited(tab_id) => {
                log(&format!("pty_exited: tab={tab_id:?}"));
                self.close_tab(tab_id, event_loop);
            }
            TermEvent::ConfigReload => {
                self.apply_config_reload();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Check if a torn-off tab's OS drag ended and merge if over a target.
        #[cfg(target_os = "windows")]
        self.check_torn_off_merge();

        // Tick tab drag/reorder animations (time-based decay, independent of mouse events)
        let anim_active = self.decay_tab_animations();
        let has_bell_badge = self.tabs.values().any(|t| t.has_bell_badge);

        // Cursor blink: detect transition since last render.
        let cursor_blink_dirty = if self.config.terminal.cursor_blink {
            self.cursor_blink_visible() != self.prev_cursor_visible
        } else {
            false
        };

        // Aggregate dirty state: pending PTY output, tab bar changes,
        // bell animation, drag animation, cursor blink transition,
        // or any active tab with dirty grid content.
        let grid_dirty = self.windows.values().any(|tw| {
            tw.active_tab_id()
                .and_then(|tid| self.tabs.get(&tid))
                .is_some_and(Tab::grid_dirty)
        });
        let needs_render = !self.pending_redraw.is_empty()
            || self.tab_bar_dirty
            || grid_dirty
            || has_bell_badge
            || anim_active
            || cursor_blink_dirty;

        // Direct rendering bypasses WM_PAINT starvation on Windows.
        // WM_PAINT has the lowest message priority — continuous WM_MOUSEMOVE
        // (e.g. htop with MOUSE_ALL) starves it indefinitely. Rendering from
        // about_to_wait is immune to this starvation.
        let frame_budget = Duration::from_millis(8);
        if needs_render && self.last_render_time.elapsed() >= frame_budget {
            self.pending_redraw.clear();
            let wids: Vec<WindowId> = self.windows.keys().copied().collect();
            for wid in wids {
                self.render_window(wid);
            }
            self.last_render_time = Instant::now();
        }

        // Schedule next wake-up based on what needs attention.
        if needs_render {
            // Active dirty state — wake at frame budget to render.
            let remaining = frame_budget.saturating_sub(self.last_render_time.elapsed());
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + remaining));
        } else if self.config.terminal.cursor_blink {
            // Idle with cursor blink — schedule at next blink transition.
            let interval_ms = self.config.terminal.cursor_blink_interval_ms.max(1);
            let elapsed_ms = self.cursor_blink_reset.elapsed().as_millis() as u64;
            let next_toggle_ms = ((elapsed_ms / interval_ms) + 1) * interval_ms;
            let sleep_ms = next_toggle_ms.saturating_sub(elapsed_ms);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(sleep_ms),
            ));
        } else {
            // Fully idle — sleep until next event.
            event_loop.set_control_flow(ControlFlow::Wait);
        }

        // Periodic stats logging.
        self.about_to_wait_count += 1;
        if self.stats_log_time.elapsed().as_secs() >= 5 {
            let secs = self.stats_log_time.elapsed().as_secs_f64();
            let avg_ms = if self.render_count > 0 {
                self.render_total_ms / f64::from(self.render_count)
            } else {
                0.0
            };
            log(&format!(
                "stats: renders={}/s (avg {avg_ms:.1}ms, total {:.0}ms), \
                 pty={} ev/s {:.1} KB/s, \
                 win_events={}/s, cursor_moved={}/s, about_to_wait={}/s",
                (f64::from(self.render_count) / secs) as u32,
                self.render_total_ms,
                (f64::from(self.pty_event_count) / secs) as u32,
                self.pty_bytes_received as f64 / secs / 1024.0,
                (f64::from(self.window_event_count) / secs) as u32,
                (f64::from(self.cursor_moved_count) / secs) as u32,
                (f64::from(self.about_to_wait_count) / secs) as u32,
            ));
            self.render_count = 0;
            self.render_total_ms = 0.0;
            self.pty_event_count = 0;
            self.pty_bytes_received = 0;
            self.window_event_count = 0;
            self.cursor_moved_count = 0;
            self.about_to_wait_count = 0;
            self.stats_log_time = Instant::now();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.window_event_count += 1;
        match event {
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }

            WindowEvent::RedrawRequested => {
                self.render_window(window_id);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(window_id, size.width, size.height);
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.handle_scale_factor_changed(window_id, scale_factor);
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(window_id, state, button, event_loop);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(window_id, delta);
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_moved_count += 1;
                self.handle_cursor_moved(window_id, position, event_loop);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Allow key release through only when Kitty REPORT_EVENT_TYPES is active.
                if event.state != ElementState::Pressed {
                    let has_kitty_events = self
                        .active_tab_id(window_id)
                        .and_then(|tid| self.tabs.get(&tid))
                        .is_some_and(|tab| tab.mode().contains(TermMode::REPORT_EVENT_TYPES));
                    if !has_kitty_events {
                        return;
                    }
                }

                let is_pressed = event.state == ElementState::Pressed;

                // Settings window: Escape closes it, all other keys ignored
                if is_pressed && self.is_settings_window(window_id) {
                    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                        self.close_settings_window();
                    }
                    return;
                }

                // Context menu: Escape dismisses it, all other keys ignored
                if self.context_menu.is_some() {
                    if is_pressed && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                        self.dismiss_context_menu(window_id);
                    }
                    return;
                }

                // Search mode: intercept all keys when search is active
                if self.search_active == Some(window_id) {
                    if is_pressed {
                        self.handle_search_key(window_id, &event);
                    }
                    return;
                }

                // Handle Escape during drag
                if is_pressed && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    if let Some(drag) = self.drag.take() {
                        // Clear drag visuals and animation state
                        self.tab_anim_offsets.remove(&drag.source_window);
                        self.drag_visual_x = None;
                        self.tab_bar_dirty = true;
                        // Redraw all windows
                        for tw in self.windows.values() {
                            tw.window.request_redraw();
                        }
                        return;
                    }
                }

                // Keybinding lookup
                let mods = build_modifiers(self.modifiers);
                if let Some(binding_key) = keybindings::key_to_binding_key(&event.logical_key) {
                    if let Some(action) =
                        keybindings::find_binding(&self.bindings, &binding_key, mods)
                    {
                        let action = action.clone();
                        if is_pressed {
                            if self.execute_action(&action, window_id, event_loop) {
                                return;
                            }
                            // SmartCopy with no selection — fall through to PTY
                        } else {
                            return; // matched binding on key release — consume
                        }
                    }
                }

                // Any keyboard input to PTY — scroll to live and clear selection
                if is_pressed {
                    self.cursor_blink_reset = Instant::now();
                }
                if let Some(tid) = self.active_tab_id(window_id) {
                    // Scroll to live on press (not release).
                    if is_pressed {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            if tab.grid().display_offset != 0 {
                                tab.scroll_to_bottom();
                            }
                            tab.clear_selection();
                        }
                    }

                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let mods = build_modifiers(self.modifiers);
                        let evt = if event.repeat {
                            KeyEventType::Repeat
                        } else if event.state == ElementState::Pressed {
                            KeyEventType::Press
                        } else {
                            KeyEventType::Release
                        };
                        let bytes = key_encoding::encode_key(
                            &event.logical_key,
                            mods,
                            tab.mode(),
                            event.text.as_ref().map(winit::keyboard::SmolStr::as_str),
                            event.location,
                            evt,
                        );
                        if !bytes.is_empty() {
                            tab.send_pty(&bytes);
                        }
                    }
                }
            }

            WindowEvent::Focused(focused) => {
                // Dismiss context menu on focus loss
                if !focused && self.context_menu.is_some() {
                    self.dismiss_context_menu(window_id);
                }
                // Skip settings window — no PTY to send to
                if self.is_settings_window(window_id) {
                    return;
                }
                if focused {
                    self.cursor_blink_reset = Instant::now();
                }
                if let Some(tid) = self.active_tab_id(window_id) {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if tab.mode().contains(TermMode::FOCUS_IN_OUT) {
                            let seq = if focused {
                                b"\x1b[I" as &[u8]
                            } else {
                                b"\x1b[O"
                            };
                            tab.send_pty(seq);
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
