//! Search bar — open, close, key handling, match navigation.

use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use crate::search::SearchState;
use crate::tab::TabId;
use crate::window::TermWindow;

use super::App;

impl App {
    pub(super) fn open_search(&mut self, window_id: WindowId) {
        let tab_id = match self
            .windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id)
        {
            Some(id) => id,
            None => return,
        };
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.search = Some(SearchState::new());
            tab.grid_dirty = true;
        }
        self.search_active = Some(window_id);
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    pub(super) fn close_search(&mut self, window_id: WindowId) {
        let tab_id = self
            .windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id);
        if let Some(tid) = tab_id {
            if let Some(tab) = self.tabs.get_mut(&tid) {
                tab.search = None;
                tab.grid_dirty = true;
            }
        }
        self.search_active = None;
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    pub(super) fn handle_search_key(
        &mut self,
        window_id: WindowId,
        event: &winit::event::KeyEvent,
    ) {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.close_search(window_id);
            }
            Key::Named(NamedKey::Enter) => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            if self.modifiers.shift_key() {
                                search.prev_match();
                            } else {
                                search.next_match();
                            }
                        }
                    }
                    self.scroll_to_search_match(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            Key::Named(NamedKey::Backspace) => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            search.query.pop();
                        }
                    }
                    self.update_search(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            Key::Character(c) => {
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    let text = c.as_str().to_owned();
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        if let Some(ref mut search) = tab.search {
                            search.query.push_str(&text);
                        }
                    }
                    self.update_search(tid);
                }
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    pub(super) fn update_search(&mut self, tab_id: TabId) {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            // Take search out temporarily to avoid borrow conflict with grid
            if let Some(mut search) = tab.search.take() {
                search.update_query(tab.grid());
                tab.search = Some(search);
                tab.grid_dirty = true;
            }
        }
        self.scroll_to_search_match(tab_id);
    }

    pub(super) fn scroll_to_search_match(&mut self, tab_id: TabId) {
        // Read the focused match position first
        let match_row = self
            .tabs
            .get(&tab_id)
            .and_then(|tab| tab.search.as_ref()?.focused_match().map(|m| m.start_row));

        if let Some(target_row) = match_row {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                let grid = tab.grid_mut();
                let sb_len = grid.scrollback.len();
                let lines = grid.lines;

                // Check if target_row is visible in the current viewport
                let viewport_start = sb_len.saturating_sub(grid.display_offset);
                let viewport_end = viewport_start + lines;

                if target_row < viewport_start || target_row >= viewport_end {
                    // Scroll so the match is roughly centered in the viewport
                    let center_offset = sb_len.saturating_sub(target_row).saturating_sub(lines / 2);
                    grid.display_offset = center_offset.min(sb_len);
                    tab.grid_dirty = true;
                }
            }
        }
    }
}
