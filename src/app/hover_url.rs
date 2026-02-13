//! Hyperlink and implicit URL hover detection.

use winit::window::{CursorIcon, WindowId};

use crate::url_detect::UrlSegment;
use crate::window::TermWindow;

use super::App;

/// Result of hover URL detection at the current cursor position.
pub(super) struct HoverResult {
    pub cursor_icon: CursorIcon,
    pub hover: Option<(WindowId, String)>,
    pub url_range: Option<Vec<UrlSegment>>,
}

impl App {
    /// Detect hyperlink or implicit URL under the cursor when Ctrl is held.
    ///
    /// Returns cursor icon (Pointer for links, Default otherwise) and the
    /// hovered URL info for display/tracking.
    pub(super) fn detect_hover_url(
        &mut self,
        window_id: WindowId,
        col: usize,
        line: usize,
    ) -> HoverResult {
        let tab_id = self
            .windows
            .get(&window_id)
            .and_then(TermWindow::active_tab_id);

        // Check OSC 8 hyperlink first
        let osc8_uri: Option<String> = tab_id.and_then(|tid| {
            let tab = self.tabs.get(&tid)?;
            let grid = tab.grid();
            let abs_row = grid.viewport_to_absolute(line);
            let row = grid.absolute_row(abs_row)?;
            if col >= row.len() {
                return None;
            }
            row[col].hyperlink().map(|h| h.uri.clone())
        });

        if let Some(ref uri) = osc8_uri {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                hover: Some((window_id, uri.clone())),
                url_range: None,
            };
        }

        // Fall through to implicit URL detection
        if let Some(tid) = tab_id {
            if let Some(tab) = self.tabs.get(&tid) {
                let grid = tab.grid();
                let abs_row = grid.viewport_to_absolute(line);
                let in_bounds = grid
                    .absolute_row(abs_row)
                    .is_some_and(|row| col < row.len());
                if in_bounds {
                    if let Some(hit) = self.url_cache.url_at(grid, abs_row, col) {
                        return HoverResult {
                            cursor_icon: CursorIcon::Pointer,
                            hover: Some((window_id, hit.url)),
                            url_range: Some(hit.segments),
                        };
                    }
                }
            }
        }

        HoverResult {
            cursor_icon: CursorIcon::Default,
            hover: None,
            url_range: None,
        }
    }
}
