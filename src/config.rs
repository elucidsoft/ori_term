use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use vte::ansi::CursorShape;

use crate::keybindings::KeybindConfig;
use crate::log;
use crate::render;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub terminal: TerminalConfig,
    pub colors: ColorConfig,
    pub window: WindowConfig,
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub keybind: Vec<KeybindConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub size: f32,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub shell: Option<String>,
    pub scrollback: usize,
    pub cursor_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub columns: usize,
    pub rows: usize,
    pub opacity: f32,
    pub tab_bar_opacity: Option<f32>,
    pub blur: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub copy_on_select: bool,
    pub bold_is_bright: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            size: render::FONT_SIZE,
            family: None,
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            scrollback: 10_000,
            cursor_style: "block".to_owned(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            scheme: "Catppuccin Mocha".to_owned(),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 120,
            rows: 30,
            opacity: 1.0,
            tab_bar_opacity: None,
            blur: true,
        }
    }
}

impl WindowConfig {
    /// Return opacity clamped to [0.0, 1.0].
    pub fn effective_opacity(&self) -> f32 {
        self.opacity.clamp(0.0, 1.0)
    }

    /// Return tab bar opacity clamped to [0.0, 1.0].
    /// Falls back to `opacity` when not explicitly set.
    pub fn effective_tab_bar_opacity(&self) -> f32 {
        self.tab_bar_opacity
            .unwrap_or(self.opacity)
            .clamp(0.0, 1.0)
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            copy_on_select: true,
            bold_is_bright: true,
        }
    }
}

/// Return the platform-specific configuration directory for `ori_term`.
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

/// Return the path to the config file.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Parse a cursor style string to `CursorShape`.
/// Accepts "block", "bar"/"beam", "underline". Defaults to Block.
pub fn parse_cursor_style(s: &str) -> CursorShape {
    match s.to_ascii_lowercase().as_str() {
        "bar" | "beam" => CursorShape::Beam,
        "underline" => CursorShape::Underline,
        _ => CursorShape::Block,
    }
}

impl Config {
    /// Load config from the default path. Returns defaults if the file
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

    /// Try to load config, returning an error message on failure.
    /// Unlike `load()`, this preserves the distinction between "file missing"
    /// and "parse error" so callers can keep the previous config on error.
    pub fn try_load() -> Result<Self, String> {
        let path = config_path();
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        toml::from_str(&data)
            .map_err(|e| format!("parse error in {}: {e}", path.display()))
    }

    /// Save config to the default path. Creates the directory if needed.
    pub fn save(&self) {
        let dir = config_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log(&format!("config: failed to create dir {}: {e}", dir.display()));
            return;
        }

        let path = config_path();
        match toml::to_string_pretty(self) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    log(&format!("config: failed to write {}: {e}", path.display()));
                }
            }
            Err(e) => {
                log(&format!("config: serialize error: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
        assert!((parsed.font.size - render::FONT_SIZE).abs() < f32::EPSILON);
        assert_eq!(parsed.terminal.scrollback, 10_000);
        assert_eq!(parsed.terminal.cursor_style, "block");
        assert_eq!(parsed.colors.scheme, "Catppuccin Mocha");
        assert_eq!(parsed.window.columns, 120);
        assert_eq!(parsed.window.rows, 30);
        assert!((parsed.window.opacity - 1.0).abs() < f32::EPSILON);
        assert!(parsed.window.blur);
        assert!(parsed.behavior.copy_on_select);
        assert!(parsed.behavior.bold_is_bright);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let toml_str = r#"
[font]
size = 20.0
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.font.size - 20.0).abs() < f32::EPSILON);
        // Other fields should be defaults
        assert_eq!(parsed.terminal.scrollback, 10_000);
        assert_eq!(parsed.window.columns, 120);
    }

    #[test]
    fn empty_toml_gives_defaults() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!((parsed.font.size - render::FONT_SIZE).abs() < f32::EPSILON);
        assert!(parsed.behavior.copy_on_select);
        assert!(parsed.behavior.bold_is_bright);
        assert_eq!(parsed.terminal.cursor_style, "block");
    }

    #[test]
    fn behavior_config_from_toml() {
        let toml_str = r#"
[behavior]
copy_on_select = false
bold_is_bright = false
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!(!parsed.behavior.copy_on_select);
        assert!(!parsed.behavior.bold_is_bright);
    }

    #[test]
    fn cursor_style_from_toml() {
        let toml_str = r#"
[terminal]
cursor_style = "bar"
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(parsed.terminal.cursor_style, "bar");
        assert_eq!(parse_cursor_style(&parsed.terminal.cursor_style), CursorShape::Beam);
    }

    #[test]
    fn parse_cursor_style_variants() {
        assert_eq!(parse_cursor_style("block"), CursorShape::Block);
        assert_eq!(parse_cursor_style("Block"), CursorShape::Block);
        assert_eq!(parse_cursor_style("bar"), CursorShape::Beam);
        assert_eq!(parse_cursor_style("beam"), CursorShape::Beam);
        assert_eq!(parse_cursor_style("underline"), CursorShape::Underline);
        assert_eq!(parse_cursor_style("Underline"), CursorShape::Underline);
        assert_eq!(parse_cursor_style("unknown"), CursorShape::Block);
    }

    #[test]
    fn opacity_config_from_toml() {
        let toml_str = r#"
[window]
opacity = 0.85
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.window.opacity - 0.85).abs() < f32::EPSILON);
        assert!((parsed.window.effective_opacity() - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn opacity_defaults_to_one() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!((parsed.window.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn opacity_clamped() {
        let toml_str = r#"
[window]
opacity = 1.5
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.window.effective_opacity() - 1.0).abs() < f32::EPSILON);

        let toml_str2 = r#"
[window]
opacity = -0.5
"#;
        let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
        assert!((parsed2.window.effective_opacity()).abs() < f32::EPSILON);
    }

    #[test]
    fn blur_defaults_to_true() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!(parsed.window.blur);
    }

    #[test]
    fn blur_config_from_toml() {
        let toml_str = r#"
[window]
blur = false
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!(!parsed.window.blur);
    }

    #[test]
    fn tab_bar_opacity_defaults_to_none() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!(parsed.window.tab_bar_opacity.is_none());
        // Falls back to opacity (1.0)
        assert!((parsed.window.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tab_bar_opacity_independent() {
        let toml_str = r#"
[window]
opacity = 0.5
tab_bar_opacity = 0.8
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.window.effective_opacity() - 0.5).abs() < f32::EPSILON);
        assert!((parsed.window.effective_tab_bar_opacity() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn tab_bar_opacity_falls_back_to_opacity() {
        let toml_str = r#"
[window]
opacity = 0.7
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!(parsed.window.tab_bar_opacity.is_none());
        assert!((parsed.window.effective_tab_bar_opacity() - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn tab_bar_opacity_clamped() {
        let toml_str = r#"
[window]
tab_bar_opacity = 1.5
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.window.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn config_dir_is_not_empty() {
        let dir = config_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn config_path_ends_with_toml() {
        let path = config_path();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("toml"));
    }
}
