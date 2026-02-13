//! GPU-accelerated terminal emulator library.

pub mod app;
pub mod cell;
pub mod clipboard;
pub mod config;
pub mod context_menu;
pub mod drag;
pub mod gpu;
pub mod grid;
pub mod icons;
pub mod key_encoding;
pub mod keybindings;
pub mod palette;
pub mod render;
pub mod search;
pub mod selection;
pub mod shell_integration;
pub mod tab;
pub mod tab_bar;
pub mod term_handler;
pub mod term_mode;
pub mod url_detect;
pub mod window;

#[cfg(target_os = "windows")]
pub mod platform_windows;

use std::io::Write;

/// Returns the path to the debug log file.
pub fn log_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("oriterm_debug.log")
}

/// Persistent buffered log file handle — opened once, reused for all writes.
static LOG_FILE: std::sync::OnceLock<std::sync::Mutex<std::io::BufWriter<std::fs::File>>> =
    std::sync::OnceLock::new();

fn log_writer() -> &'static std::sync::Mutex<std::io::BufWriter<std::fs::File>> {
    LOG_FILE.get_or_init(|| {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .expect("failed to open log file");
        std::sync::Mutex::new(std::io::BufWriter::new(file))
    })
}

/// Writes a log message to the debug log file.
pub fn log(msg: &str) {
    if let Ok(mut w) = log_writer().lock() {
        let _ = w.write_all(msg.as_bytes());
        let _ = w.write_all(b"\n");
        let _ = w.flush();
    }
}
