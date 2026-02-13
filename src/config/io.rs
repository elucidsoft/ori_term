//! Configuration file I/O — path resolution, loading, and saving.

use std::path::PathBuf;

use vte::ansi::CursorShape;

use super::Config;
use crate::log;

/// Returns the platform-specific configuration directory for `ori_term`.
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("ori_term");
        }
        PathBuf::from(".").join("ori_term")
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("ori_term");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("ori_term");
        }
        PathBuf::from(".").join("ori_term")
    }
}

/// Returns the path to the config file.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Returns the path to the runtime state file (separate from user config).
pub fn state_path() -> PathBuf {
    config_dir().join("state.toml")
}

/// Persisted window geometry — saved on exit, restored on launch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowState {
    /// Loads window state from `state.toml`. Returns `None` if the file is
    /// missing, unreadable, or contains invalid TOML.
    pub fn load() -> Option<Self> {
        let path = state_path();
        let data = std::fs::read_to_string(&path).ok()?;
        match toml::from_str(&data) {
            Ok(state) => Some(state),
            Err(e) => {
                log(&format!("state: parse error in {}: {e}", path.display()));
                None
            }
        }
    }

    /// Saves window state to `state.toml`. Creates the config directory if needed.
    pub fn save(&self) {
        save_toml(self, &state_path(), "state");
    }
}

impl Config {
    /// Loads config from the default path. Returns defaults if the file
    /// doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let path = config_path();
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log(&format!("config: failed to read {}: {e}", path.display()));
                }
                return Self::default();
            }
        };

        match toml::from_str(&data) {
            Ok(cfg) => {
                log(&format!("config: loaded from {}", path.display()));
                cfg
            }
            Err(e) => {
                log(&format!("config: parse error in {}: {e}", path.display()));
                Self::default()
            }
        }
    }

    /// Tries to load config, returning an error message on failure.
    /// Unlike `load()`, this preserves the distinction between "file missing"
    /// and "parse error" so callers can keep the previous config on error.
    pub fn try_load() -> Result<Self, String> {
        let path = config_path();
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        toml::from_str(&data).map_err(|e| format!("parse error in {}: {e}", path.display()))
    }

    /// Saves config to the default path. Creates the directory if needed.
    pub fn save(&self) {
        save_toml(self, &config_path(), "config");
    }
}

/// Parses a cursor style string to `CursorShape`.
/// Accepts "block", "bar"/"beam", "underline". Defaults to Block.
pub fn parse_cursor_style(s: &str) -> CursorShape {
    match s.to_ascii_lowercase().as_str() {
        "bar" | "beam" => CursorShape::Beam,
        "underline" => CursorShape::Underline,
        _ => CursorShape::Block,
    }
}

/// Serialize a value to TOML and write it to `path`, creating the parent directory if needed.
fn save_toml(value: &impl serde::Serialize, path: &std::path::Path, label: &str) {
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            log(&format!(
                "{label}: failed to create dir {}: {e}",
                dir.display()
            ));
            return;
        }
    }
    match toml::to_string_pretty(value) {
        Ok(data) => {
            if let Err(e) = std::fs::write(path, data) {
                log(&format!("{label}: failed to write {}: {e}", path.display()));
            }
        }
        Err(e) => {
            log(&format!("{label}: serialize error: {e}"));
        }
    }
}
