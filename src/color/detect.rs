use std::env;
use std::io::{IsTerminal, stdout};

use super::profile::ColorProfile;

/// Detect the color profile from environment variables and TTY status.
pub fn detect_color_profile() -> ColorProfile {
    // Rule 1: NO_COLOR takes highest priority
    if env::var_os("NO_COLOR").is_some() {
        return ColorProfile::None;
    }

    // Rule 2: CLICOLOR_FORCE overrides TTY check
    let force = match env::var("CLICOLOR_FORCE") {
        Ok(v) if v != "0" => true,
        _ => false,
    };

    // CLICOLOR=0 disables color
    if let Ok(v) = env::var("CLICOLOR") {
        if v == "0" {
            return ColorProfile::None;
        }
    }

    // If not a TTY and not forced, no color
    if !force && !stdout().is_terminal() {
        return ColorProfile::None;
    }

    // Rule 3: Detect profile level
    let term = env::var("TERM").unwrap_or_default();
    let colorterm = env::var("COLORTERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    // Dumb terminal
    if term == "dumb" {
        return ColorProfile::None;
    }

    // TrueColor checks
    if matches!(colorterm.as_str(), "truecolor" | "24bit") {
        return ColorProfile::TrueColor;
    }

    // Known TrueColor terminals
    if matches!(
        term_program.as_str(),
        "iTerm.app" | "WezTerm" | "ghostty"
    ) {
        return ColorProfile::TrueColor;
    }

    // Windows Terminal always supports TrueColor
    if env::var_os("WT_SESSION").is_some() {
        return ColorProfile::TrueColor;
    }

    // 256-color checks
    if colorterm == "256color" || term.contains("256color") {
        return ColorProfile::Ansi256;
    }

    // If TERM is set at all, assume basic ANSI
    if !term.is_empty() {
        return ColorProfile::Ansi;
    }

    // Windows 10+ supports VT sequences natively.
    // If we're on Windows, it's a TTY (or forced), and nothing else matched,
    // assume TrueColor — crossterm enables VT processing automatically.
    #[cfg(windows)]
    {
        return ColorProfile::TrueColor;
    }

    // Fallback: if forced but no TERM, give basic ANSI; otherwise none
    #[cfg(not(windows))]
    if force {
        ColorProfile::Ansi
    } else {
        ColorProfile::None
    }
}
