//! Pixel-to-cell coordinate conversion and URL opening.

use winit::dpi::PhysicalPosition;

use crate::grid::{GRID_PADDING_LEFT, GRID_PADDING_TOP};
use crate::log;
use crate::selection::Side;
use crate::tab_bar::TAB_BAR_HEIGHT;

use super::App;

impl App {
    /// Convert pixel coordinates to grid cell (col, `viewport_line`).
    /// Returns None if outside the grid area.
    pub(super) fn pixel_to_cell(&self, pos: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        let x = pos.x as usize;
        let y = pos.y as usize;
        let grid_top = self.scale_px(TAB_BAR_HEIGHT) + self.scale_px(GRID_PADDING_TOP);
        let padding_left = self.scale_px(GRID_PADDING_LEFT);
        if y < grid_top || x < padding_left {
            return None;
        }
        let cw = self.glyphs.cell_width;
        let ch = self.glyphs.cell_height;
        if cw == 0 || ch == 0 {
            return None;
        }
        let col = (x - padding_left) / cw;
        let line = (y - grid_top) / ch;
        Some((col, line))
    }

    /// Determine which side of the cell the cursor is on.
    pub(super) fn pixel_to_side(&self, pos: PhysicalPosition<f64>) -> Side {
        let x = pos.x as usize;
        let cw = self.glyphs.cell_width;
        if cw == 0 {
            return Side::Left;
        }
        let padding_left = self.scale_px(GRID_PADDING_LEFT);
        let cell_x = (x.saturating_sub(padding_left)) % cw;
        if cell_x < cw / 2 {
            Side::Left
        } else {
            Side::Right
        }
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
