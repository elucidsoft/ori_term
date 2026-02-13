//! Configuration unit tests.

use vte::ansi::CursorShape;

use super::*;
use crate::render;

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
    assert_eq!(
        parse_cursor_style(&parsed.terminal.cursor_style),
        CursorShape::Beam
    );
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
    assert_eq!(
        parsed2.colors.alpha_blending,
        AlphaBlending::LinearCorrected
    );
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
    assert_eq!(
        parsed.colors.selection_foreground.as_deref(),
        Some("#FFFFFF")
    );
    assert_eq!(
        parsed.colors.selection_background.as_deref(),
        Some("#3A3D5C")
    );
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
    assert_eq!(
        parsed.colors.ansi.get("0").map(|s| s.as_str()),
        Some("#111111")
    );
    assert!(parsed.colors.ansi.get("1").is_none());
    assert_eq!(
        parsed.colors.ansi.get("7").map(|s| s.as_str()),
        Some("#EEEEEE")
    );
    assert!(parsed.colors.bright.get("0").is_none());
    assert_eq!(
        parsed.colors.bright.get("1").map(|s| s.as_str()),
        Some("#FF0000")
    );
}

#[test]
fn color_overrides_roundtrip() {
    let mut cfg = Config::default();
    cfg.colors.foreground = Some("#FFFFFF".to_owned());
    cfg.colors.selection_background = Some("#3A3D5C".to_owned());
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.colors.foreground.as_deref(), Some("#FFFFFF"));
    assert_eq!(
        parsed.colors.selection_background.as_deref(),
        Some("#3A3D5C")
    );
    assert!(parsed.colors.background.is_none());
}
