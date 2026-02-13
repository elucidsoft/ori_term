//! Configuration structures and loading logic.

mod io;
pub mod monitor;

pub use io::{WindowState, config_dir, config_path, parse_cursor_style, state_path};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::keybindings::KeybindConfig;
use crate::render;

/// Top-level configuration structure.
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

/// Font configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub size: f32,
    pub family: Option<String>,
    /// OpenType features to enable/disable during text shaping.
    ///
    /// Each string is a 4-character feature tag, optionally prefixed with `-`
    /// to disable. Examples: `"calt"`, `"liga"`, `"-dlig"`.
    /// Defaults to `["calt", "liga"]` (contextual alternates + standard ligatures).
    pub features: Vec<String>,
}

/// Terminal behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub shell: Option<String>,
    pub scrollback: usize,
    pub cursor_style: String,
    pub cursor_blink: bool,
    pub cursor_blink_interval_ms: u64,
}

/// Alpha blending mode for text rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaBlending {
    /// Current behavior — sRGB surface format handles gamma-correct blending.
    Linear,
    /// Ghostty-style luminance-based alpha correction for even text weight.
    #[default]
    LinearCorrected,
}

/// Color scheme and palette configuration.
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
    /// Returns `minimum_contrast` clamped to [1.0, 21.0].
    pub fn effective_minimum_contrast(&self) -> f32 {
        self.minimum_contrast.clamp(1.0, 21.0)
    }
}

/// Window size and opacity configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub columns: usize,
    pub rows: usize,
    pub opacity: f32,
    pub tab_bar_opacity: Option<f32>,
    pub blur: bool,
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
    /// Returns opacity clamped to [0.0, 1.0].
    pub fn effective_opacity(&self) -> f32 {
        self.opacity.clamp(0.0, 1.0)
    }

    /// Returns tab bar opacity clamped to [0.0, 1.0].
    /// Falls back to `opacity` when not explicitly set.
    pub fn effective_tab_bar_opacity(&self) -> f32 {
        self.tab_bar_opacity.unwrap_or(self.opacity).clamp(0.0, 1.0)
    }
}

/// User interaction behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub copy_on_select: bool,
    pub bold_is_bright: bool,
    pub shell_integration: bool,
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

/// Visual bell configuration.
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

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            size: render::FONT_SIZE,
            family: None,
            features: vec!["calt".into(), "liga".into()],
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

#[cfg(test)]
mod tests;
