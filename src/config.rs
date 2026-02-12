use std::collections::HashMap;
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
    pub bell: BellConfig,
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
    pub cursor_blink: bool,
    pub cursor_blink_interval_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaBlending {
    /// Current behavior — sRGB surface format handles gamma-correct blending.
    Linear,
    /// Ghostty-style luminance-based alpha correction for even text weight.
    #[default]
    LinearCorrected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub scheme: String,
    /// Minimum WCAG 2.0 contrast ratio (1.0 = off, range 1.0–21.0).
    pub minimum_contrast: f32,
    /// Alpha blending mode for text rendering.
    pub alpha_blending: AlphaBlending,
    /// Override foreground color ("#RRGGBB" hex).
    pub foreground: Option<String>,
    /// Override background color ("#RRGGBB" hex).
    pub background: Option<String>,
    /// Override cursor color ("#RRGGBB" hex).
    pub cursor: Option<String>,
    /// Override selection foreground color ("#RRGGBB" hex). Default: swap with bg.
    pub selection_foreground: Option<String>,
    /// Override selection background color ("#RRGGBB" hex). Default: swap with fg.
    pub selection_background: Option<String>,
    /// Override ANSI colors 0-7 by index. Keys "0"-"7", values "#RRGGBB".
    /// Only indices present are overridden.
    #[serde(default)]
    pub ansi: HashMap<String, String>,
    /// Override bright ANSI colors 8-15 by index (0-7 maps to colors 8-15).
    /// Only indices present are overridden.
    #[serde(default)]
    pub bright: HashMap<String, String>,
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
    pub shell_integration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BellConfig {
    /// Visual bell animation: `ease_out`, `linear`, `none`
    pub animation: String,
    /// Duration in milliseconds (0 = disabled)
    pub duration_ms: u16,
    /// Flash color as "#RRGGBB" hex (default: white)
    pub color: Option<String>,
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
            cursor_blink: true,
            cursor_blink_interval_ms: 530,
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            scheme: "Catppuccin Mocha".to_owned(),
            minimum_contrast: 1.0,
            alpha_blending: AlphaBlending::default(),
            foreground: None,
            background: None,
            cursor: None,
            selection_foreground: None,
            selection_background: None,
            ansi: HashMap::new(),
            bright: HashMap::new(),
        }
    }
}

impl ColorConfig {
    /// Return `minimum_contrast` clamped to [1.0, 21.0].
    pub fn effective_minimum_contrast(&self) -> f32 {
        self.minimum_contrast.clamp(1.0, 21.0)
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
            shell_integration: true,
        }
    }
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            animation: "ease_out".into(),
            duration_ms: 150,
            color: None,
        }
    }
}

impl BellConfig {
    /// Returns true when the visual bell is enabled.
    pub fn is_enabled(&self) -> bool {
        self.duration_ms > 0 && self.animation != "none"
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

/// Return the path to the runtime state file (separate from user config).
pub fn state_path() -> PathBuf {
    config_dir().join("state.toml")
}

/// Persisted window geometry — saved on exit, restored on launch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowState {
    /// Load window state from `state.toml`. Returns `None` if the file is
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

    /// Save window state to `state.toml`. Creates the config directory if needed.
    pub fn save(&self) {
        let dir = config_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log(&format!("state: failed to create dir {}: {e}", dir.display()));
            return;
        }
        let path = state_path();
        match toml::to_string_pretty(self) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    log(&format!("state: failed to write {}: {e}", path.display()));
                }
            }
            Err(e) => {
                log(&format!("state: serialize error: {e}"));
            }
        }
    }
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
        assert!(parsed.terminal.cursor_blink);
        assert_eq!(parsed.terminal.cursor_blink_interval_ms, 530);
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
    fn cursor_blink_defaults() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!(parsed.terminal.cursor_blink);
        assert_eq!(parsed.terminal.cursor_blink_interval_ms, 530);
    }

    #[test]
    fn cursor_blink_from_toml() {
        let toml_str = r#"
[terminal]
cursor_blink = false
cursor_blink_interval_ms = 250
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!(!parsed.terminal.cursor_blink);
        assert_eq!(parsed.terminal.cursor_blink_interval_ms, 250);
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
    fn minimum_contrast_defaults_to_off() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!((parsed.colors.minimum_contrast - 1.0).abs() < f32::EPSILON);
        assert!((parsed.colors.effective_minimum_contrast() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn minimum_contrast_clamped() {
        let toml_str = r#"
[colors]
minimum_contrast = 25.0
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert!((parsed.colors.effective_minimum_contrast() - 21.0).abs() < f32::EPSILON);

        let toml_str2 = r#"
[colors]
minimum_contrast = 0.5
"#;
        let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
        assert!((parsed2.colors.effective_minimum_contrast() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn alpha_blending_defaults_to_linear_corrected() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert_eq!(parsed.colors.alpha_blending, AlphaBlending::LinearCorrected);
    }

    #[test]
    fn alpha_blending_from_toml() {
        let toml_str = r#"
[colors]
alpha_blending = "linear"
"#;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(parsed.colors.alpha_blending, AlphaBlending::Linear);

        let toml_str2 = r#"
[colors]
alpha_blending = "linear_corrected"
"#;
        let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
        assert_eq!(parsed2.colors.alpha_blending, AlphaBlending::LinearCorrected);
    }

    #[test]
    fn color_config_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.colors.scheme, "Catppuccin Mocha");
        assert!((parsed.colors.minimum_contrast - 1.0).abs() < f32::EPSILON);
        assert_eq!(parsed.colors.alpha_blending, AlphaBlending::LinearCorrected);
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

    #[test]
    fn color_overrides_from_toml() {
        let toml_str = r##"
[colors]
scheme = "Dracula"
foreground = "#FFFFFF"
background = "#000000"
cursor = "#FF0000"
selection_foreground = "#FFFFFF"
selection_background = "#3A3D5C"
"##;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(parsed.colors.scheme, "Dracula");
        assert_eq!(parsed.colors.foreground.as_deref(), Some("#FFFFFF"));
        assert_eq!(parsed.colors.background.as_deref(), Some("#000000"));
        assert_eq!(parsed.colors.cursor.as_deref(), Some("#FF0000"));
        assert_eq!(parsed.colors.selection_foreground.as_deref(), Some("#FFFFFF"));
        assert_eq!(parsed.colors.selection_background.as_deref(), Some("#3A3D5C"));
    }

    #[test]
    fn color_overrides_default_none() {
        let parsed: Config = toml::from_str("").expect("deserialize");
        assert!(parsed.colors.foreground.is_none());
        assert!(parsed.colors.background.is_none());
        assert!(parsed.colors.cursor.is_none());
        assert!(parsed.colors.selection_foreground.is_none());
        assert!(parsed.colors.selection_background.is_none());
        assert!(parsed.colors.ansi.is_empty());
        assert!(parsed.colors.bright.is_empty());
    }

    #[test]
    fn color_overrides_partial() {
        let toml_str = r##"
[colors]
foreground = "#AABBCC"
"##;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(parsed.colors.foreground.as_deref(), Some("#AABBCC"));
        assert!(parsed.colors.background.is_none());
        assert!(parsed.colors.cursor.is_none());
    }

    #[test]
    fn ansi_overrides_from_toml() {
        let toml_str = r##"
[colors.ansi]
0 = "#111111"
7 = "#EEEEEE"

[colors.bright]
1 = "#FF0000"
"##;
        let parsed: Config = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(parsed.colors.ansi.get("0").map(|s| s.as_str()), Some("#111111"));
        assert!(parsed.colors.ansi.get("1").is_none());
        assert_eq!(parsed.colors.ansi.get("7").map(|s| s.as_str()), Some("#EEEEEE"));
        assert!(parsed.colors.bright.get("0").is_none());
        assert_eq!(parsed.colors.bright.get("1").map(|s| s.as_str()), Some("#FF0000"));
    }

    #[test]
    fn color_overrides_roundtrip() {
        let mut cfg = Config::default();
        cfg.colors.foreground = Some("#FFFFFF".to_owned());
        cfg.colors.selection_background = Some("#3A3D5C".to_owned());
        let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.colors.foreground.as_deref(), Some("#FFFFFF"));
        assert_eq!(parsed.colors.selection_background.as_deref(), Some("#3A3D5C"));
        assert!(parsed.colors.background.is_none());
    }
}
