//! Keyboard input handling — action dispatch from keybindings.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::keybindings::Action;

use super::App;

impl App {
    /// Mutate the active tab for `window_id` and request a redraw.
    /// Returns `true` if the tab was found and the closure ran.
    fn with_active_tab_redraw(
        &mut self,
        window_id: WindowId,
        f: impl FnOnce(&mut crate::tab::Tab),
    ) -> bool {
        let Some(tid) = self.active_tab_id(window_id) else {
            return false;
        };
        if let Some(tab) = self.tabs.get_mut(&tid) {
            f(tab);
        }
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
        true
    }

    /// Switch to an adjacent tab (wrapping). `delta` is +1 for next, -1 for prev.
    fn cycle_tab(&mut self, window_id: WindowId, delta: isize) {
        if let Some(tw) = self.windows.get_mut(&window_id) {
            let n = tw.tabs.len();
            if n > 1 {
                tw.active_tab = (tw.active_tab as isize + delta).rem_euclid(n as isize) as usize;
                self.tab_bar_dirty = true;
                tw.window.request_redraw();
            }
        }
    }

    pub(super) fn execute_action(
        &mut self,
        action: &Action,
        window_id: WindowId,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        match action {
            Action::ZoomIn => {
                self.change_font_size(window_id, 1.0);
            }
            Action::ZoomOut => {
                self.change_font_size(window_id, -1.0);
            }
            Action::ZoomReset => {
                self.reset_font_size(window_id);
            }
            Action::NewTab => {
                self.new_tab_in_window(window_id);
            }
            Action::CloseTab => {
                if let Some(tid) = self.active_tab_id(window_id) {
                    self.close_tab(tid, event_loop);
                }
            }
            Action::NextTab => self.cycle_tab(window_id, 1),
            Action::PrevTab => self.cycle_tab(window_id, -1),
            Action::ScrollPageUp => {
                self.with_active_tab_redraw(window_id, |tab| {
                    let page = tab.grid().lines;
                    tab.scroll_page_up(page);
                });
            }
            Action::ScrollPageDown => {
                self.with_active_tab_redraw(window_id, |tab| {
                    let page = tab.grid().lines;
                    tab.scroll_page_down(page);
                });
            }
            Action::ScrollToTop => {
                self.with_active_tab_redraw(window_id, |tab| {
                    tab.scroll_to_top();
                });
            }
            Action::ScrollToBottom => {
                self.with_active_tab_redraw(window_id, |tab| {
                    tab.scroll_to_bottom();
                });
            }
            Action::Copy => {
                if let Some(tid) = self.active_tab_id(window_id) {
                    self.copy_selection_to_clipboard(tid);
                }
            }
            Action::Paste | Action::SmartPaste => {
                self.paste_from_clipboard(window_id);
            }
            Action::SmartCopy => {
                if let Some(tid) = self.active_tab_id(window_id) {
                    let has_selection = self.tabs.get(&tid).is_some_and(|t| t.selection.is_some());
                    if has_selection {
                        self.copy_selection_to_clipboard(tid);
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.clear_selection();
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    } else {
                        // No selection — fall through to PTY.
                        return false;
                    }
                }
            }
            Action::ReloadConfig => {
                self.apply_config_reload();
            }
            Action::OpenSearch => {
                self.open_search(window_id);
            }
            Action::PreviousPrompt => {
                self.with_active_tab_redraw(window_id, |tab| {
                    tab.navigate_to_previous_prompt();
                });
            }
            Action::NextPrompt => {
                self.with_active_tab_redraw(window_id, |tab| {
                    tab.navigate_to_next_prompt();
                });
            }
            Action::SendText(text) => {
                if let Some(tid) = self.active_tab_id(window_id) {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        tab.send_pty(text.as_bytes());
                    }
                }
            }
            Action::DuplicateTab => {
                if let Some(tw) = self.windows.get(&window_id) {
                    self.duplicate_tab_at(tw.active_tab);
                }
            }
            Action::MoveTabToNewWindow => {
                if let Some(tw) = self.windows.get(&window_id) {
                    self.move_tab_to_new_window(tw.active_tab, event_loop);
                }
            }
            Action::None => {
                // Explicitly unbound — should not appear after merge, but
                // consume the key if it does.
            }
        }
        true
    }
}
