//! Thin platform wrapper for system clipboard access.
//!
//! On Windows, uses `clipboard-win` for real clipboard operations.
//! On other platforms, provides no-op stubs (to be replaced by arboard or similar
//! when Section 14 — Cross-Platform adds full clipboard support).

/// Read text from the system clipboard.
#[cfg(windows)]
pub fn get_text() -> Option<String> {
    clipboard_win::get_clipboard_string().ok()
}

/// Read text from the system clipboard (stub on non-Windows).
#[cfg(not(windows))]
pub fn get_text() -> Option<String> {
    None
}

/// Write text to the system clipboard. Returns `true` on success.
#[cfg(windows)]
pub fn set_text(text: &str) -> bool {
    clipboard_win::set_clipboard_string(text).is_ok()
}

/// Write text to the system clipboard (stub on non-Windows).
#[cfg(not(windows))]
#[allow(unused_variables)]
pub fn set_text(text: &str) -> bool {
    false
}
