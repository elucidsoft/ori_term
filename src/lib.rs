pub mod color;
pub mod style;
pub mod text;
pub mod terminal;
pub mod output;
pub mod input;
pub mod layout;
pub mod widgets;

// Terminal emulator modules
pub mod clipboard;
pub mod config;
pub mod config_monitor;
pub mod cell;
pub mod key_encoding;
pub mod keybindings;
pub mod search;
pub mod url_detect;
pub mod selection;
pub mod grid;
pub mod palette;
pub mod term_mode;
pub mod term_handler;
pub mod render;
pub mod gpu;
pub mod tab;
pub mod tab_bar;
pub mod drag;
pub mod window;
pub mod app;

use std::io::Write;

pub fn log_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("oriterm_debug.log")
}

pub fn log(msg: &str) {
    use std::fs::OpenOptions;
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .and_then(|mut f| {
            Write::write_all(&mut f, msg.as_bytes())?;
            Write::write_all(&mut f, b"\n")
        });
}
