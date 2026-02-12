//! Tab lifecycle — creation, close, duplicate, move to new window.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::config;
use crate::log;
use crate::palette;
use crate::tab::{Tab, TabId};
use crate::window::TermWindow;

use super::App;

impl App {
    pub(super) fn new_tab_in_window(&mut self, window_id: WindowId) -> Option<TabId> {
        // Compute grid size from window
        let default_cols = self.config.window.columns;
        let default_rows = self.config.window.rows;
        let (cols, rows) =
            self.windows
                .get(&window_id)
                .map_or((default_cols, default_rows), |tw| {
                    let size = tw.window.inner_size();
                    self.grid_dims_for_size(size.width, size.height)
                });

        // Inherit CWD from the active tab in this window.
        let inherit_cwd: Option<String> = self
            .windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id)
            .and_then(|tid| self.tabs.get(&tid))
            .and_then(|t| t.cwd.clone());

        let tab_id = self.alloc_tab_id();
        let cursor_shape = config::parse_cursor_style(&self.config.terminal.cursor_style);
        let tab = match Tab::spawn(
            tab_id,
            cols,
            rows,
            self.proxy.clone(),
            self.config.terminal.shell.as_deref(),
            self.config.terminal.scrollback,
            cursor_shape,
            self.shell_integration_dir.as_deref(),
            inherit_cwd.as_deref(),
        ) {
            Ok(t) => t,
            Err(e) => {
                log(&format!("failed to spawn tab: {e}"));
                return None;
            }
        };

        self.tabs.insert(tab_id, tab);

        // Apply the active color scheme, color overrides, and behavior settings to the new tab
        if let Some(t) = self.tabs.get_mut(&tab_id) {
            if let Some(scheme) = palette::find_scheme(self.active_scheme) {
                t.palette.set_scheme(scheme);
            }
            t.palette.apply_overrides(&self.config.colors);
            t.palette.bold_is_bright = self.config.behavior.bold_is_bright;
        }

        self.tab_bar_dirty = true;
        // Clear width lock — adding a tab changes the count
        if self.tab_width_lock.is_some_and(|(wid, _)| wid == window_id) {
            self.tab_width_lock = None;
        }
        if let Some(tw) = self.windows.get_mut(&window_id) {
            tw.add_tab(tab_id);
            tw.window.request_redraw();
        }

        log(&format!("new tab {tab_id:?} in window {window_id:?}"));
        Some(tab_id)
    }

    pub(super) fn close_tab(&mut self, tab_id: TabId, _event_loop: &ActiveEventLoop) {
        // Remove the tab from its window first
        self.tab_bar_dirty = true;
        let mut empty_windows = Vec::new();
        for (wid, tw) in &mut self.windows {
            if tw.remove_tab(tab_id) {
                empty_windows.push(*wid);
            } else {
                tw.window.request_redraw();
            }
        }

        // If this leaves a window with no tabs AND it's the last one, force-exit
        // BEFORE dropping the Tab (ClosePseudoConsole would block).
        for wid in &empty_windows {
            if self.windows.len() <= 1 {
                self.exit_app();
            }
            self.windows.remove(wid);
        }

        // Safe to drop now — only reached for non-last windows
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.shutdown();
        }
        self.tabs.remove(&tab_id);
    }

    /// Duplicate tab at `tab_index` — spawns a new tab with the same CWD.
    pub(super) fn duplicate_tab_at(&mut self, tab_index: usize) {
        let info = self
            .windows
            .iter()
            .find_map(|(&wid, tw)| tw.tabs.get(tab_index).map(|&tid| (wid, tid)));
        let Some((window_id, source_tab_id)) = info else {
            return;
        };
        let cwd = self.tabs.get(&source_tab_id).and_then(|t| t.cwd.clone());

        let default_cols = self.config.window.columns;
        let default_rows = self.config.window.rows;
        let (cols, rows) =
            self.windows
                .get(&window_id)
                .map_or((default_cols, default_rows), |tw| {
                    let size = tw.window.inner_size();
                    self.grid_dims_for_size(size.width, size.height)
                });

        let tab_id = self.alloc_tab_id();
        let cursor_shape = config::parse_cursor_style(&self.config.terminal.cursor_style);
        let tab = match Tab::spawn(
            tab_id,
            cols,
            rows,
            self.proxy.clone(),
            self.config.terminal.shell.as_deref(),
            self.config.terminal.scrollback,
            cursor_shape,
            self.shell_integration_dir.as_deref(),
            cwd.as_deref(),
        ) {
            Ok(t) => t,
            Err(e) => {
                log(&format!("duplicate_tab: failed to spawn: {e}"));
                return;
            }
        };
        self.tabs.insert(tab_id, tab);
        if let Some(t) = self.tabs.get_mut(&tab_id) {
            if let Some(scheme) = palette::find_scheme(self.active_scheme) {
                t.palette.set_scheme(scheme);
            }
        }
        if let Some(tw) = self.windows.get_mut(&window_id) {
            tw.add_tab(tab_id);
            self.tab_bar_dirty = true;
            tw.window.request_redraw();
        }
    }

    /// Move the tab at `tab_index` into a brand-new window.
    pub(super) fn move_tab_to_new_window(
        &mut self,
        tab_index: usize,
        event_loop: &ActiveEventLoop,
    ) {
        let info = self
            .windows
            .iter()
            .find_map(|(&wid, tw)| tw.tabs.get(tab_index).map(|&tid| (wid, tid)));
        let Some((src_wid, tab_id)) = info else {
            return;
        };

        // Don't move if it's the last tab in the window
        if self
            .windows
            .get(&src_wid)
            .is_some_and(|tw| tw.tabs.len() <= 1)
        {
            return;
        }

        // Remove from source window
        if let Some(tw) = self.windows.get_mut(&src_wid) {
            tw.remove_tab(tab_id);
            self.tab_bar_dirty = true;
            tw.window.request_redraw();
        }

        // Create a new window and add the tab
        if let Some(new_wid) = self.create_window(event_loop, None, true) {
            if let Some(tw) = self.windows.get_mut(&new_wid) {
                tw.add_tab(tab_id);
                self.tab_bar_dirty = true;
                tw.window.request_redraw();
            }
        }
    }
}
