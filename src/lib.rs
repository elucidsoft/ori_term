pub mod color;
pub mod style;
pub mod text;
pub mod terminal;
pub mod output;
pub mod input;
pub mod layout;
pub mod widgets;

// Terminal emulator modules
pub mod grid;
pub mod render;
pub mod vte_performer;
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
        .unwrap_or(std::path::Path::new("."))
        .join("ori_console_debug.log")
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
