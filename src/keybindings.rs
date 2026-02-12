use serde::{Deserialize, Serialize};
use winit::keyboard::{Key, NamedKey};

use crate::key_encoding::Modifiers;

/// Identifies a key independent of modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindingKey {
    Named(NamedKey),
    /// Always stored lowercase.
    Character(String),
}

/// Action to execute when a keybinding matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Copy,
    Paste,
    /// Copy if selection exists, else fall through to PTY.
    SmartCopy,
    /// Paste from clipboard (Ctrl+V without Shift).
    SmartPaste,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    OpenSearch,
    ReloadConfig,
    /// Navigate to previous prompt mark (OSC 133;A).
    PreviousPrompt,
    /// Navigate to next prompt mark (OSC 133;A).
    NextPrompt,
    /// Duplicate the current tab (spawn new tab with same CWD).
    DuplicateTab,
    /// Move the current tab into a new window.
    MoveTabToNewWindow,
    /// Send literal bytes to the PTY.
    SendText(String),
    /// Explicitly unbinds a default binding.
    None,
}

/// A resolved keybinding: key + modifiers → action.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: BindingKey,
    pub mods: Modifiers,
    pub action: Action,
}

/// TOML-serializable keybinding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindConfig {
    pub key: String,
    #[serde(default)]
    pub mods: String,
    pub action: String,
}

/// All built-in default keybindings. More-specific modifier combos come first
/// so that e.g. Ctrl+Shift+C (Copy) is checked before Ctrl+C (`SmartCopy`).
pub fn default_bindings() -> Vec<KeyBinding> {
    let ch = |s: &str| BindingKey::Character(s.to_owned());
    let named = BindingKey::Named;
    let ctrl = Modifiers::CONTROL;
    let shift = Modifiers::SHIFT;
    let ctrl_shift = ctrl | shift;

    vec![
        // Explicit copy / paste (Ctrl+Shift+C/V)
        KeyBinding { key: ch("c"), mods: ctrl_shift, action: Action::Copy },
        KeyBinding { key: ch("v"), mods: ctrl_shift, action: Action::Paste },
        // Ctrl+Insert / Shift+Insert
        KeyBinding { key: named(NamedKey::Insert), mods: ctrl, action: Action::Copy },
        KeyBinding { key: named(NamedKey::Insert), mods: shift, action: Action::Paste },
        // Config / search
        KeyBinding { key: ch("r"), mods: ctrl_shift, action: Action::ReloadConfig },
        KeyBinding { key: ch("f"), mods: ctrl_shift, action: Action::OpenSearch },
        // Zoom
        KeyBinding { key: ch("="), mods: ctrl, action: Action::ZoomIn },
        KeyBinding { key: ch("+"), mods: ctrl, action: Action::ZoomIn },
        KeyBinding { key: ch("-"), mods: ctrl, action: Action::ZoomOut },
        KeyBinding { key: ch("0"), mods: ctrl, action: Action::ZoomReset },
        // Tabs
        KeyBinding { key: ch("t"), mods: ctrl, action: Action::NewTab },
        KeyBinding { key: ch("w"), mods: ctrl, action: Action::CloseTab },
        KeyBinding { key: named(NamedKey::Tab), mods: ctrl, action: Action::NextTab },
        KeyBinding { key: named(NamedKey::Tab), mods: ctrl_shift, action: Action::PrevTab },
        // Scrollback
        KeyBinding { key: named(NamedKey::PageUp), mods: shift, action: Action::ScrollPageUp },
        KeyBinding { key: named(NamedKey::PageDown), mods: shift, action: Action::ScrollPageDown },
        KeyBinding { key: named(NamedKey::Home), mods: shift, action: Action::ScrollToTop },
        KeyBinding { key: named(NamedKey::End), mods: shift, action: Action::ScrollToBottom },
        // Prompt navigation
        KeyBinding { key: named(NamedKey::ArrowUp), mods: ctrl_shift, action: Action::PreviousPrompt },
        KeyBinding { key: named(NamedKey::ArrowDown), mods: ctrl_shift, action: Action::NextPrompt },
        // Smart copy/paste (Ctrl+C/V without Shift) — must come AFTER
        // Ctrl+Shift variants so those match first.
        KeyBinding { key: ch("c"), mods: ctrl, action: Action::SmartCopy },
        KeyBinding { key: ch("v"), mods: ctrl, action: Action::SmartPaste },
    ]
}

/// Convert a winit `Key` to a `BindingKey`, normalizing characters to lowercase.
pub fn key_to_binding_key(key: &Key) -> Option<BindingKey> {
    match key {
        Key::Named(n) => Some(BindingKey::Named(*n)),
        Key::Character(s) => {
            let lower = s.as_str().to_lowercase();
            if lower.is_empty() {
                None
            } else {
                Some(BindingKey::Character(lower))
            }
        }
        _ => None,
    }
}

/// Find the first binding matching the given key and modifiers.
pub fn find_binding<'a>(
    bindings: &'a [KeyBinding],
    key: &BindingKey,
    mods: Modifiers,
) -> Option<&'a Action> {
    bindings.iter().find_map(|b| {
        if b.key == *key && b.mods == mods {
            Some(&b.action)
        } else {
            None
        }
    })
}

/// Merge user keybinding overrides with defaults. User bindings replace
/// defaults that share the same (key, mods). `Action::None` removes a binding.
pub fn merge_bindings(user: &[KeybindConfig]) -> Vec<KeyBinding> {
    let mut bindings = default_bindings();

    for cfg in user {
        let Some(key) = parse_key(&cfg.key) else {
            crate::log(&format!("keybindings: unknown key {:?}", cfg.key));
            continue;
        };
        let mods = parse_mods(&cfg.mods);
        let Some(action) = parse_action(&cfg.action) else {
            crate::log(&format!("keybindings: unknown action {:?}", cfg.action));
            continue;
        };

        // Remove any existing binding with the same key+mods.
        bindings.retain(|b| !(b.key == key && b.mods == mods));

        // Action::None means "unbind" — don't add a replacement.
        if action != Action::None {
            bindings.push(KeyBinding { key, mods, action });
        }
    }

    bindings
}

/// Parse a key string from TOML config. Single characters are lowercased.
/// Named keys: Tab, `PageUp`, `PageDown`, Home, End, Insert,
/// Delete, Escape, Enter, Backspace, Space, F1–F24.
pub fn parse_key(s: &str) -> Option<BindingKey> {
    // Try named keys first.
    let named = match s {
        "Tab" => Some(NamedKey::Tab),
        "PageUp" => Some(NamedKey::PageUp),
        "PageDown" => Some(NamedKey::PageDown),
        "Home" => Some(NamedKey::Home),
        "End" => Some(NamedKey::End),
        "Insert" => Some(NamedKey::Insert),
        "Delete" => Some(NamedKey::Delete),
        "Escape" => Some(NamedKey::Escape),
        "Enter" => Some(NamedKey::Enter),
        "Backspace" => Some(NamedKey::Backspace),
        "Space" => Some(NamedKey::Space),
        "ArrowUp" => Some(NamedKey::ArrowUp),
        "ArrowDown" => Some(NamedKey::ArrowDown),
        "ArrowLeft" => Some(NamedKey::ArrowLeft),
        "ArrowRight" => Some(NamedKey::ArrowRight),
        "F1" => Some(NamedKey::F1),
        "F2" => Some(NamedKey::F2),
        "F3" => Some(NamedKey::F3),
        "F4" => Some(NamedKey::F4),
        "F5" => Some(NamedKey::F5),
        "F6" => Some(NamedKey::F6),
        "F7" => Some(NamedKey::F7),
        "F8" => Some(NamedKey::F8),
        "F9" => Some(NamedKey::F9),
        "F10" => Some(NamedKey::F10),
        "F11" => Some(NamedKey::F11),
        "F12" => Some(NamedKey::F12),
        "F13" => Some(NamedKey::F13),
        "F14" => Some(NamedKey::F14),
        "F15" => Some(NamedKey::F15),
        "F16" => Some(NamedKey::F16),
        "F17" => Some(NamedKey::F17),
        "F18" => Some(NamedKey::F18),
        "F19" => Some(NamedKey::F19),
        "F20" => Some(NamedKey::F20),
        "F21" => Some(NamedKey::F21),
        "F22" => Some(NamedKey::F22),
        "F23" => Some(NamedKey::F23),
        "F24" => Some(NamedKey::F24),
        _ => None,
    };

    if let Some(n) = named {
        return Some(BindingKey::Named(n));
    }

    // Single-character key (always lowercase).
    if !s.is_empty() && s.len() <= 4 {
        // Could be a multi-byte UTF-8 char, but should be a single char.
        let mut chars = s.chars();
        if let Some(c) = chars.next() {
            if chars.next().is_none() {
                return Some(BindingKey::Character(
                    c.to_lowercase().to_string(),
                ));
            }
        }
    }

    None
}

/// Parse a modifier string like "Ctrl", "Ctrl|Shift", "Alt", "", or "None".
pub fn parse_mods(s: &str) -> Modifiers {
    let mut mods = Modifiers::empty();
    for part in s.split('|') {
        match part.trim() {
            "Ctrl" | "Control" => mods |= Modifiers::CONTROL,
            "Shift" => mods |= Modifiers::SHIFT,
            "Alt" => mods |= Modifiers::ALT,
            "Super" => mods |= Modifiers::SUPER,
            _ => {} // "None", "", or unknown — no modifier
        }
    }
    mods
}

/// Parse an action string. Supports `SendText:...` for literal text with
/// escape sequences (`\x1b`, `\n`, `\r`, `\t`, `\\`).
pub fn parse_action(s: &str) -> Option<Action> {
    if let Some(text) = s.strip_prefix("SendText:") {
        return Some(Action::SendText(unescape_send_text(text)));
    }

    Some(match s {
        "Copy" => Action::Copy,
        "Paste" => Action::Paste,
        "SmartCopy" => Action::SmartCopy,
        "SmartPaste" => Action::SmartPaste,
        "NewTab" => Action::NewTab,
        "CloseTab" => Action::CloseTab,
        "NextTab" => Action::NextTab,
        "PrevTab" => Action::PrevTab,
        "ZoomIn" => Action::ZoomIn,
        "ZoomOut" => Action::ZoomOut,
        "ZoomReset" => Action::ZoomReset,
        "ScrollPageUp" => Action::ScrollPageUp,
        "ScrollPageDown" => Action::ScrollPageDown,
        "ScrollToTop" => Action::ScrollToTop,
        "ScrollToBottom" => Action::ScrollToBottom,
        "OpenSearch" => Action::OpenSearch,
        "ReloadConfig" => Action::ReloadConfig,
        "PreviousPrompt" => Action::PreviousPrompt,
        "NextPrompt" => Action::NextPrompt,
        "DuplicateTab" => Action::DuplicateTab,
        "MoveTabToNewWindow" => Action::MoveTabToNewWindow,
        "None" => Action::None,
        _ => return None,
    })
}

/// Process escape sequences in `SendText` values:
/// `\x1b` → ESC, `\n` → newline, `\r` → CR, `\t` → tab, `\\` → backslash.
fn unescape_send_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('x') => {
                    // Parse \xHH hex escape.
                    let hi = chars.next().unwrap_or('0');
                    let lo = chars.next().unwrap_or('0');
                    let hex: String = [hi, lo].iter().collect();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        out.push(byte as char);
                    }
                }
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') | None => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_not_empty() {
        let bindings = default_bindings();
        assert!(!bindings.is_empty());
        assert!(bindings.len() >= 20);
    }

    #[test]
    fn find_binding_ctrl_t() {
        let bindings = default_bindings();
        let key = BindingKey::Character("t".to_owned());
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, Some(&Action::NewTab));
    }

    #[test]
    fn find_binding_no_match() {
        let bindings = default_bindings();
        let key = BindingKey::Character("z".to_owned());
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, None);
    }

    #[test]
    fn merge_user_override() {
        let user = vec![KeybindConfig {
            key: "t".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "CloseTab".to_owned(),
        }];
        let bindings = merge_bindings(&user);
        let key = BindingKey::Character("t".to_owned());
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, Some(&Action::CloseTab));
    }

    #[test]
    fn merge_user_unbind() {
        let user = vec![KeybindConfig {
            key: "t".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "None".to_owned(),
        }];
        let bindings = merge_bindings(&user);
        let key = BindingKey::Character("t".to_owned());
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, None);
    }

    #[test]
    fn merge_preserves_unaffected() {
        let user = vec![KeybindConfig {
            key: "t".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "None".to_owned(),
        }];
        let bindings = merge_bindings(&user);
        // Ctrl+W should still be CloseTab.
        let key = BindingKey::Character("w".to_owned());
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, Some(&Action::CloseTab));
    }

    #[test]
    fn parse_mods_variants() {
        assert_eq!(parse_mods("Ctrl"), Modifiers::CONTROL);
        assert_eq!(
            parse_mods("Ctrl|Shift"),
            Modifiers::CONTROL | Modifiers::SHIFT
        );
        assert_eq!(parse_mods("Alt"), Modifiers::ALT);
        assert_eq!(parse_mods(""), Modifiers::empty());
        assert_eq!(parse_mods("None"), Modifiers::empty());
    }

    #[test]
    fn parse_key_variants() {
        assert_eq!(
            parse_key("c"),
            Some(BindingKey::Character("c".to_owned()))
        );
        assert_eq!(
            parse_key("PageUp"),
            Some(BindingKey::Named(NamedKey::PageUp))
        );
        assert_eq!(
            parse_key("Tab"),
            Some(BindingKey::Named(NamedKey::Tab))
        );
        assert_eq!(
            parse_key("F1"),
            Some(BindingKey::Named(NamedKey::F1))
        );
    }

    #[test]
    fn parse_action_variants() {
        assert_eq!(parse_action("Copy"), Some(Action::Copy));
        assert_eq!(parse_action("Paste"), Some(Action::Paste));
        assert_eq!(parse_action("NewTab"), Some(Action::NewTab));
        assert_eq!(parse_action("None"), Some(Action::None));
        assert_eq!(
            parse_action("SendText:\\x1b[A"),
            Some(Action::SendText("\x1b[A".to_owned()))
        );
        assert_eq!(parse_action("UnknownAction"), None);
    }

    #[test]
    fn key_normalization() {
        // key_to_binding_key lowercases characters.
        let key = Key::Character("C".into());
        let bk = key_to_binding_key(&key);
        assert_eq!(bk, Some(BindingKey::Character("c".to_owned())));
    }

    #[test]
    fn smart_copy_distinct_from_copy() {
        let bindings = default_bindings();
        let key = BindingKey::Character("c".to_owned());

        // Ctrl+C → SmartCopy
        let action = find_binding(&bindings, &key, Modifiers::CONTROL);
        assert_eq!(action, Some(&Action::SmartCopy));

        // Ctrl+Shift+C → Copy
        let action = find_binding(
            &bindings,
            &key,
            Modifiers::CONTROL | Modifiers::SHIFT,
        );
        assert_eq!(action, Some(&Action::Copy));
    }

    #[test]
    fn unescape_sequences() {
        assert_eq!(unescape_send_text("\\x1b[15~"), "\x1b[15~");
        assert_eq!(unescape_send_text("a\\nb"), "a\nb");
        assert_eq!(unescape_send_text("\\r\\t\\\\"), "\r\t\\");
    }
}
