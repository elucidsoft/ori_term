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
use crate::tab::TermEvent;
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

                self.cursor_blink_reset = Instant::now();

                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    tab.process_output(&data);
                }

                // Process bell state: if this tab rang the bell and is NOT active,
                // set the bell badge so the tab bar shows an indicator.
                let is_active = self
                    .window_containing_tab(tab_id)
                    .and_then(|wid| self.windows.get(&wid))
                    .is_some_and(|tw| tw.active_tab_id() == Some(tab_id));
                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    if tab.bell_start.is_some() && !is_active {
                        tab.has_bell_badge = true;
                        self.tab_bar_dirty = true;
                    }
                    // Active tab never needs a bell badge (user can see it).
                    if is_active && tab.has_bell_badge {
                        tab.has_bell_badge = false;
                    }
                    for notif in tab.drain_notifications() {
                        if notif.title.is_empty() {
                            log(&format!("notification: {}", notif.body));
                        } else {
                            log(&format!("notification: {} — {}", notif.title, notif.body));
                        }
                    }
                }

                self.url_cache.invalidate();
                // Defer redraw — coalesced in about_to_wait()
                if let Some(wid) = self.window_containing_tab(tab_id) {
                    if let Some(tw) = self.windows.get(&wid) {
                        let bell_active =
                            self.tabs.get(&tab_id).and_then(|t| t.bell_start).is_some();
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

        // Drain coalesced redraws: N PTY events → 1 request_redraw per window.
        for wid in self.pending_redraw.drain() {
            if let Some(tw) = self.windows.get(&wid) {
                tw.window.request_redraw();
            }
        }

        // Tick tab drag/reorder animations (time-based decay, independent of mouse events)
        let anim_active = self.decay_tab_animations();

        // Smart cursor blink / bell scheduling: instead of spinning at 60fps,
        // sleep until the next blink transition or bell animation tick.
        // NOTE: dragging does NOT use Poll — CursorMoved events drive redraws
        // directly. Polling would present stale frames between mouse events,
        // each blocking on vsync and adding latency (Chrome doesn't poll either).
        let has_bell_badge = self.tabs.values().any(|t| t.has_bell_badge);
        if has_bell_badge || anim_active {
            // Bell badge or tab animation is active — schedule ~60fps redraws.
            // Use WaitUntil instead of Poll to avoid spinning the event loop,
            // which would block CursorMoved events behind vsync presents.
            for tw in self.windows.values() {
                tw.window.request_redraw();
            }
            let deadline = Instant::now() + Duration::from_millis(16);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else if self.config.terminal.cursor_blink {
            // Schedule wake-up at the next blink transition.
            let interval_ms = self.config.terminal.cursor_blink_interval_ms.max(1);
            let elapsed_ms = self.cursor_blink_reset.elapsed().as_millis() as u64;
            let next_toggle_ms = ((elapsed_ms / interval_ms) + 1) * interval_ms;
            let sleep_ms = next_toggle_ms.saturating_sub(elapsed_ms);
            let deadline = Instant::now() + Duration::from_millis(sleep_ms);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            // Request redraw for all windows so the blink state updates.
            for tw in self.windows.values() {
                tw.window.request_redraw();
            }
        } else {
            // Fully idle — sleep until next event.
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
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
                self.handle_cursor_moved(window_id, position, event_loop);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Allow key release through only when Kitty REPORT_EVENT_TYPES is active.
                if event.state != ElementState::Pressed {
                    let has_kitty_events = self
                        .active_tab_id(window_id)
                        .and_then(|tid| self.tabs.get(&tid))
                        .is_some_and(|tab| tab.mode.contains(TermMode::REPORT_EVENT_TYPES));
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
                            tab.mode,
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
                        if tab.mode.contains(TermMode::FOCUS_IN_OUT) {
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
