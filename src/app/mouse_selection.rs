//! Grid selection — click count detection, single/double/triple click, shift-extend.

use std::time::Instant;

use winit::dpi::PhysicalPosition;
use winit::window::WindowId;

use crate::grid::StableRowIndex;
use crate::selection::{self, Selection, SelectionMode, SelectionPoint, Side};

use super::{App, DOUBLE_CLICK_MS};

impl App {
    /// Detect click count (1=char, 2=word, 3=line), cycling on rapid clicks.
    pub(super) fn detect_click_count(
        &mut self,
        window_id: WindowId,
        col: usize,
        line: usize,
    ) -> u8 {
        let now = Instant::now();
        let same_pos = self.last_grid_click_pos == Some((col, line));
        let same_window = self.last_click_window == Some(window_id);
        let within_time = self
            .last_click_time
            .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS);

        let count = if same_pos && same_window && within_time {
            match self.click_count {
                1 => 2,
                2 => 3,
                _ => 1,
            }
        } else {
            1
        };

        self.last_click_time = Some(now);
        self.last_click_window = Some(window_id);
        self.last_grid_click_pos = Some((col, line));
        self.click_count = count;
        count
    }

    /// Handle Ctrl+click to open a hyperlink URL (OSC 8 or implicit).
    ///
    /// Returns true if a URL was opened and the click should be consumed.
    fn handle_ctrl_click_url(&mut self, tab_id: crate::tab::TabId, abs_row: usize, col: usize) -> bool {
        // Check OSC 8 hyperlink first
        let uri: Option<String> = self.tabs.get(&tab_id).and_then(|tab| {
            let grid = tab.grid();
            let row = grid.absolute_row(abs_row)?;
            if col >= row.len() {
                return None;
            }
            row[col].hyperlink().map(|h| h.uri.clone())
        });
        if let Some(ref uri) = uri {
            Self::open_url(uri);
            return true;
        }
        // Fall through to implicit URL detection
        let implicit_url: Option<String> = self.tabs.get(&tab_id).and_then(|tab| {
            let grid = tab.grid();
            let hit = self.url_cache.url_at(&grid, abs_row, col)?;
            Some(hit.url)
        });
        if let Some(ref url) = implicit_url {
            Self::open_url(url);
            return true;
        }
        false
    }

    /// Handle a left-click press in the grid area — selection start.
    pub(super) fn handle_grid_press(&mut self, window_id: WindowId, pos: PhysicalPosition<f64>) {
        let (col, line) = match self.pixel_to_cell(pos) {
            Some(c) => c,
            None => return,
        };
        let side = self.pixel_to_side(pos);

        let Some(tab_id) = self.active_tab_id(window_id) else {
            return;
        };

        // Single lock: clamp bounds, compute absolute + stable row.
        let (grid_cols, col, line, abs_row, stable_row) = match self.tabs.get(&tab_id) {
            Some(tab) => {
                let g = tab.grid();
                let c = col.min(g.cols.saturating_sub(1));
                let l = line.min(g.lines.saturating_sub(1));
                let abs = g.viewport_to_absolute(l);
                let stable = StableRowIndex::from_absolute(&g, abs);
                (g.cols, c, l, abs, stable)
            }
            None => return,
        };

        // Ctrl+click: open hyperlink URL (OSC 8 first, then implicit URL)
        if self.modifiers.control_key() && self.handle_ctrl_click_url(tab_id, abs_row, col) {
            return;
        }

        let click_count = self.detect_click_count(window_id, col, line);
        let shift = self.modifiers.shift_key();
        let alt = self.modifiers.alt_key();

        // Shift+click: extend existing selection
        if shift {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                if tab.selection.is_some() {
                    tab.update_selection_end(SelectionPoint {
                        row: stable_row,
                        col,
                        side,
                    });
                    self.left_mouse_down = true;
                    if let Some(tw) = self.windows.get(&window_id) {
                        tw.window.request_redraw();
                    }
                    return;
                }
            }
        }

        // Create new selection based on click count. Double/triple-click
        // need one more grid lock for word/line boundary computation.
        let new_selection = match click_count {
            2 => {
                // Double-click: word selection
                self.tabs.get(&tab_id).map(|tab| {
                    let grid = tab.grid();
                    let (ws, we) = selection::word_boundaries(&grid, abs_row, col);
                    Selection::new_word(
                        SelectionPoint { row: stable_row, col: ws, side: Side::Left },
                        SelectionPoint { row: stable_row, col: we, side: Side::Right },
                    )
                })
            }
            3 => {
                // Triple-click: line selection
                self.tabs.get(&tab_id).map(|tab| {
                    let grid = tab.grid();
                    let ls = selection::logical_line_start(&grid, abs_row);
                    let le = selection::logical_line_end(&grid, abs_row);
                    Selection::new_line(
                        SelectionPoint {
                            row: StableRowIndex::from_absolute(&grid, ls),
                            col: 0,
                            side: Side::Left,
                        },
                        SelectionPoint {
                            row: StableRowIndex::from_absolute(&grid, le),
                            col: grid_cols.saturating_sub(1),
                            side: Side::Right,
                        },
                    )
                })
            }
            _ => {
                // Single click: char selection (or block if Alt held)
                let mut sel = Selection::new_char(stable_row, col, side);
                if alt {
                    sel.mode = SelectionMode::Block;
                }
                Some(sel)
            }
        };

        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            if let Some(sel) = new_selection {
                tab.set_selection(sel);
            }
        }
        self.left_mouse_down = true;

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }

    /// Update selection endpoint during mouse drag.
    ///
    /// Handles char, word, line, and block selection modes. When the cursor
    /// is outside the grid, auto-scrolls into history (above) or toward live
    /// (below).
    pub(super) fn update_selection_drag(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
    ) {
        let Some(tid) = self.active_tab_id(window_id) else {
            return;
        };

        if let Some((col, line)) = self.pixel_to_cell(position) {
            let side = self.pixel_to_side(position);
            let Some(tab) = self.tabs.get_mut(&tid) else {
                return;
            };

            // Read selection state before locking the grid (selection lives
            // on Tab, not behind the Mutex).
            let sel_mode = tab.selection.as_ref().map(|s| s.mode);
            let sel_anchor = tab.selection.as_ref().map(|s| s.anchor);

            // Single lock: clamp, compute absolute/stable row, and any
            // word/line boundary data needed for the current selection mode.
            let new_end = {
                let grid = tab.grid();
                let col = col.min(grid.cols.saturating_sub(1));
                let line = line.min(grid.lines.saturating_sub(1));
                let abs_row = grid.viewport_to_absolute(line);
                let stable_row = StableRowIndex::from_absolute(&grid, abs_row);

                match sel_mode {
                    Some(SelectionMode::Word) => {
                        let (ws, we) = selection::word_boundaries(&grid, abs_row, col);
                        let start_pt = SelectionPoint { row: stable_row, col: ws, side: Side::Left };
                        let end_pt = SelectionPoint { row: stable_row, col: we, side: Side::Right };
                        if sel_anchor.is_some_and(|a| start_pt < a) {
                            Some(start_pt)
                        } else {
                            Some(end_pt)
                        }
                    }
                    Some(SelectionMode::Line) => {
                        let ls = selection::logical_line_start(&grid, abs_row);
                        let le = selection::logical_line_end(&grid, abs_row);
                        let grid_cols = grid.cols;
                        if sel_anchor.is_some_and(|a| stable_row < a.row) {
                            Some(SelectionPoint {
                                row: StableRowIndex::from_absolute(&grid, ls),
                                col: 0,
                                side: Side::Left,
                            })
                        } else {
                            Some(SelectionPoint {
                                row: StableRowIndex::from_absolute(&grid, le),
                                col: grid_cols.saturating_sub(1),
                                side: Side::Right,
                            })
                        }
                    }
                    Some(_) => Some(SelectionPoint { row: stable_row, col, side }),
                    None => None,
                }
            };

            if let Some(end) = new_end {
                tab.update_selection_end(end);
            }
        } else {
            // Mouse outside grid — auto-scroll
            let y = position.y as usize;
            let grid_top = self.grid_top();
            let ch = self.font_collection.cell_height;
            let Some(tab) = self.tabs.get_mut(&tid) else {
                return;
            };
            if y < grid_top {
                tab.scroll_lines(1);
            } else {
                // Single lock for both reads.
                let grid = tab.grid();
                let grid_bottom = grid_top + grid.lines * ch;
                let offset = grid.display_offset;
                drop(grid);
                if y >= grid_bottom && offset > 0 {
                    tab.scroll_lines(-1);
                }
            }
        }

        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.request_redraw();
        }
    }
}
