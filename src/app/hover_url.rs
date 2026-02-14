//! Hyperlink and implicit URL hover detection and opening.

use winit::window::{CursorIcon, WindowId};

use crate::log;
use crate::url_detect::UrlSegment;

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
        let no_hit = HoverResult {
            cursor_icon: CursorIcon::Default,
            hover: None,
            url_range: None,
        };

        let Some(tid) = self.active_tab_id(window_id) else {
            return no_hit;
        };

        // Compute abs_row, check bounds, and extract OSC 8 URI in one pass.
        let (abs_row, osc8_uri) = {
            let Some(tab) = self.tabs.get(&tid) else {
                return no_hit;
            };
            let grid = tab.grid();
            let abs_row = grid.viewport_to_absolute(line);
            let Some(row) = grid.absolute_row(abs_row) else {
                return no_hit;
            };
            if col >= row.len() {
                return no_hit;
            }
            let osc8 = row[col].hyperlink().map(|h| h.uri.clone());
            (abs_row, osc8)
        };

        // OSC 8 hyperlink takes priority.
        if let Some(uri) = osc8_uri {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                hover: Some((window_id, uri)),
                url_range: None,
            };
        }

        // Fall through to implicit URL detection.
        let Some(tab) = self.tabs.get(&tid) else {
            return no_hit;
        };
        let url_hit = self.url_cache.url_at(&tab.grid(), abs_row, col);
        if let Some(hit) = url_hit {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                hover: Some((window_id, hit.url)),
                url_range: Some(hit.segments),
            };
        }

        no_hit
    }

    /// Open a URL in the default browser. Only allows safe schemes.
    ///
    /// On Windows, uses `ShellExecuteW` directly (like Windows Terminal and
    /// `WezTerm`) instead of `cmd /C start` which mangles `&` and `%` in URLs.
    #[allow(unsafe_code, reason = "ShellExecuteW FFI requires unsafe")]
    pub(super) fn open_url(uri: &str) {
        let allowed = uri.starts_with("http://")
            || uri.starts_with("https://")
            || uri.starts_with("ftp://")
            || uri.starts_with("file://");
        if !allowed {
            log(&format!(
                "hyperlink: blocked URI with disallowed scheme: {uri}"
            ));
            return;
        }
        log(&format!("hyperlink: opening ({} chars) {uri}", uri.len()));
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStrExt;
            let wide_open: Vec<u16> = std::ffi::OsStr::new("open")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_uri: Vec<u16> = std::ffi::OsStr::new(uri)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: ShellExecuteW is a standard Windows API call with
            // null-terminated wide strings. No memory safety concerns.
            unsafe {
                windows_sys::Win32::UI::Shell::ShellExecuteW(
                    std::ptr::null_mut(),
                    wide_open.as_ptr(),
                    wide_uri.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(uri).spawn();
        }
    }
}
