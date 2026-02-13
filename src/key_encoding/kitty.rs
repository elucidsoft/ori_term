//! Kitty keyboard protocol encoding (CSI u format).

use winit::keyboard::{Key, KeyLocation, NamedKey};

use super::{KeyEventType, Modifiers};
use crate::term_mode::TermMode;

/// Kitty-defined codepoints for functional keys.
fn kitty_codepoint(key: NamedKey) -> Option<u32> {
    Some(match key {
        NamedKey::Escape => 27,
        NamedKey::Enter => 13,
        NamedKey::Tab => 9,
        NamedKey::Backspace => 127,
        NamedKey::Insert => 57348,
        NamedKey::Delete => 57349,
        NamedKey::ArrowLeft => 57350,
        NamedKey::ArrowRight => 57351,
        NamedKey::ArrowUp => 57352,
        NamedKey::ArrowDown => 57353,
        NamedKey::PageUp => 57354,
        NamedKey::PageDown => 57355,
        NamedKey::Home => 57356,
        NamedKey::End => 57357,
        NamedKey::CapsLock => 57358,
        NamedKey::ScrollLock => 57359,
        NamedKey::NumLock => 57360,
        NamedKey::PrintScreen => 57361,
        NamedKey::Pause => 57362,
        NamedKey::ContextMenu => 57363,
        NamedKey::F1 => 57364,
        NamedKey::F2 => 57365,
        NamedKey::F3 => 57366,
        NamedKey::F4 => 57367,
        NamedKey::F5 => 57368,
        NamedKey::F6 => 57369,
        NamedKey::F7 => 57370,
        NamedKey::F8 => 57371,
        NamedKey::F9 => 57372,
        NamedKey::F10 => 57373,
        NamedKey::F11 => 57374,
        NamedKey::F12 => 57375,
        NamedKey::F13 => 57376,
        NamedKey::F14 => 57377,
        NamedKey::F15 => 57378,
        NamedKey::F16 => 57379,
        NamedKey::F17 => 57380,
        NamedKey::F18 => 57381,
        NamedKey::F19 => 57382,
        NamedKey::F20 => 57383,
        NamedKey::F21 => 57384,
        NamedKey::F22 => 57385,
        NamedKey::F23 => 57386,
        NamedKey::F24 => 57387,
        NamedKey::F25 => 57388,
        NamedKey::F26 => 57389,
        NamedKey::F27 => 57390,
        NamedKey::F28 => 57391,
        NamedKey::F29 => 57392,
        NamedKey::F30 => 57393,
        NamedKey::F31 => 57394,
        NamedKey::F32 => 57395,
        NamedKey::F33 => 57396,
        NamedKey::F34 => 57397,
        NamedKey::F35 => 57398,
        NamedKey::Space => 32,
        _ => return None,
    })
}

/// Encode a key event using the Kitty keyboard protocol (CSI u format).
///
/// Format: `ESC [ codepoint ; modifiers [: event_type] u`
pub(super) fn encode_kitty(
    key: &Key,
    mods: Modifiers,
    mode: TermMode,
    text: Option<&str>,
    _location: KeyLocation,
    event_type: KeyEventType,
) -> Vec<u8> {
    let report_all = mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC);
    let report_events = mode.contains(TermMode::REPORT_EVENT_TYPES);

    // Determine the codepoint.
    let codepoint = match key {
        Key::Named(named) => {
            if let Some(cp) = kitty_codepoint(*named) {
                cp
            } else {
                return Vec::new();
            }
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            let mut chars = s.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() {
                    // Single character — use its Unicode codepoint.
                    // For letters with Ctrl, use the base lowercase codepoint.
                    let cp = c as u32;
                    let needs_event_type = report_events && event_type != KeyEventType::Press;
                    if !report_all && !needs_event_type && mods.is_empty() && cp >= 32 && cp != 127
                    {
                        // No mods, printable, normal press — send as plain text.
                        return text.map_or_else(Vec::new, |t| t.as_bytes().to_vec());
                    }
                    cp
                } else {
                    // Multi-char — send as text.
                    return text.map_or_else(Vec::new, |t| t.as_bytes().to_vec());
                }
            } else {
                return Vec::new();
            }
        }
        _ => return Vec::new(),
    };

    let mod_param = mods.xterm_param();

    // Kitty event types: 1=press (default, omitted), 2=repeat, 3=release.
    let event_suffix: &str = if report_events {
        match event_type {
            KeyEventType::Press => "",
            KeyEventType::Repeat => ":2",
            KeyEventType::Release => ":3",
        }
    } else {
        // Without REPORT_EVENT_TYPES, release events should not be sent.
        if event_type == KeyEventType::Release {
            return Vec::new();
        }
        ""
    };

    // Build CSI u sequence.
    if mod_param > 0 || !event_suffix.is_empty() {
        let m = if mod_param > 0 { mod_param } else { 1 };
        format!("\x1b[{codepoint};{m}{event_suffix}u").into_bytes()
    } else {
        format!("\x1b[{codepoint}u").into_bytes()
    }
}
