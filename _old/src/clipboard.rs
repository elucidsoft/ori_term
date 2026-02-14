//! Thin platform wrapper for system clipboard access.
//!
//! On Windows, uses `clipboard-win` for real clipboard operations.
//! On other platforms, uses `arboard` for real clipboard operations.

/// Read text from the system clipboard.
#[cfg(windows)]
pub fn get_text() -> Option<String> {
    clipboard_win::get_clipboard_string().ok()
}

/// Read text from the system clipboard (non-Windows, via arboard).
#[cfg(not(windows))]
pub fn get_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Write text to the system clipboard. Returns `true` on success.
#[cfg(windows)]
pub fn set_text(text: &str) -> bool {
    clipboard_win::set_clipboard_string(text).is_ok()
}

/// Write text to the system clipboard (non-Windows, via arboard).
#[cfg(not(windows))]
pub fn set_text(text: &str) -> bool {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text.to_owned()))
        .is_ok()
}
