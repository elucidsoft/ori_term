//! Keyboard input handling — action dispatch from keybindings.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::clipboard;
use crate::keybindings::Action;
use crate::selection;
use crate::window::TermWindow;

use super::App;

impl App {
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
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    self.close_tab(tid, event_loop);
                }
            }
            Action::NextTab => {
                if let Some(tw) = self.windows.get_mut(&window_id) {
                    let n = tw.tabs.len();
                    if n > 1 {
                        tw.active_tab = (tw.active_tab + 1) % n;
                        self.tab_bar_dirty = true;
                        tw.window.request_redraw();
                    }
                }
            }
            Action::PrevTab => {
                if let Some(tw) = self.windows.get_mut(&window_id) {
                    let n = tw.tabs.len();
                    if n > 1 {
                        tw.active_tab = (tw.active_tab + n - 1) % n;
                        self.tab_bar_dirty = true;
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollPageUp => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let page = grid.lines;
                        let max = grid.scrollback.len();
                        grid.display_offset = (grid.display_offset + page).min(max);
                        tab.grid_dirty = true;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollPageDown => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let page = grid.lines;
                        grid.display_offset = grid.display_offset.saturating_sub(page);
                        tab.grid_dirty = true;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollToTop => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        grid.display_offset = grid.scrollback.len();
                        tab.grid_dirty = true;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::ScrollToBottom => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        tab.grid_mut().display_offset = 0;
                        tab.grid_dirty = true;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::Copy => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get(&tid) {
                        if let Some(ref sel) = tab.selection {
                            let text = selection::extract_text(tab.grid(), sel);
                            if !text.is_empty() {
                                clipboard::set_text(&text);
                            }
                        }
                    }
                }
            }
            Action::Paste | Action::SmartPaste => {
                self.paste_from_clipboard(window_id);
            }
            Action::SmartCopy => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    let has_selection = self.tabs.get(&tid).is_some_and(|t| t.selection.is_some());
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
                            tab.selection = None;
                            tab.grid_dirty = true;
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
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let sb_len = grid.scrollback.len();
                        // Current top of viewport as scrollback index.
                        let viewport_top_sb = sb_len.saturating_sub(grid.display_offset);
                        // Scan scrollback rows backwards from just above viewport top.
                        let mut target_sb = None;
                        for i in (0..viewport_top_sb).rev() {
                            if grid.scrollback[i].prompt_start {
                                target_sb = Some(i);
                                break;
                            }
                        }
                        if let Some(sb_idx) = target_sb {
                            grid.display_offset = sb_len.saturating_sub(sb_idx);
                            tab.grid_dirty = true;
                        }
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::NextPrompt => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let grid = tab.grid_mut();
                        let sb_len = grid.scrollback.len();
                        // Current bottom of viewport as scrollback index.
                        let viewport_bottom_sb =
                            sb_len.saturating_sub(grid.display_offset) + grid.lines;
                        // Scan scrollback + visible rows forward from below viewport.
                        let total_rows = sb_len + grid.lines;
                        let mut target_sb = None;
                        for i in viewport_bottom_sb..total_rows {
                            let has_prompt = if i < sb_len {
                                grid.scrollback[i].prompt_start
                            } else {
                                grid.row(i - sb_len).prompt_start
                            };
                            if has_prompt {
                                target_sb = Some(i);
                                break;
                            }
                        }
                        if let Some(idx) = target_sb {
                            if idx < sb_len {
                                grid.display_offset = sb_len.saturating_sub(idx);
                            } else {
                                grid.display_offset = 0;
                            }
                        } else {
                            // No prompt below — scroll to live.
                            grid.display_offset = 0;
                        }
                        tab.grid_dirty = true;
                    }
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                }
            }
            Action::SendText(text) => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
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
