//! Search bar — open, close, key handling, match navigation.

use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use super::App;
use crate::tab::TabId;

impl App {
    pub(super) fn open_search(&mut self, window_id: WindowId) {
        let Some(tab_id) = self.active_tab_id(window_id) else {
            return;
        };
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.open_search();
        }
        self.search_active = Some(window_id);
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    pub(super) fn close_search(&mut self, window_id: WindowId) {
        if let Some(tid) = self.active_tab_id(window_id) {
            if let Some(tab) = self.tabs.get_mut(&tid) {
                tab.close_search();
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
        // Escape closes the search bar (which handles its own redraw).
        if event.logical_key == Key::Named(NamedKey::Escape) {
            self.close_search(window_id);
            return;
        }

        let Some(tid) = self.active_tab_id(window_id) else {
            return;
        };

        let mut needs_redraw = false;
        match &event.logical_key {
            Key::Named(NamedKey::Enter) => {
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    if let Some(search) = &mut tab.search {
                        if self.modifiers.shift_key() {
                            search.prev_match();
                        } else {
                            search.next_match();
                        }
                    }
                }
                self.scroll_to_search_match(tid);
                needs_redraw = true;
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    if let Some(search) = &mut tab.search {
                        search.query.pop();
                    }
                }
                self.update_search(tid);
                needs_redraw = true;
            }
            Key::Character(c) => {
                if let Some(tab) = self.tabs.get_mut(&tid) {
                    if let Some(search) = &mut tab.search {
                        search.query.push_str(c.as_str());
                    }
                }
                self.update_search(tid);
                needs_redraw = true;
            }
            _ => {}
        }

        if needs_redraw {
            if let Some(tw) = self.windows.get(&window_id) {
                tw.window.request_redraw();
            }
        }
    }

    pub(super) fn update_search(&mut self, tab_id: TabId) {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.update_search_query();
        }
        self.scroll_to_search_match(tab_id);
    }

    pub(super) fn scroll_to_search_match(&mut self, tab_id: TabId) {
        // Read the focused match stable row, then convert to absolute.
        let match_abs_row = self.tabs.get(&tab_id).and_then(|tab| {
            let stable = tab.search.as_ref()?.focused_match()?.start_row;
            stable.to_absolute(tab.grid())
        });

        if let Some(target_row) = match_abs_row {
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
