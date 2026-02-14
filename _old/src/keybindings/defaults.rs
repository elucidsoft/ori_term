//! Built-in default keybindings.

use winit::keyboard::NamedKey;

use crate::key_encoding::Modifiers;

use super::{Action, BindingKey, KeyBinding};

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
        KeyBinding {
            key: ch("c"),
            mods: ctrl_shift,
            action: Action::Copy,
        },
        KeyBinding {
            key: ch("v"),
            mods: ctrl_shift,
            action: Action::Paste,
        },
        // Ctrl+Insert / Shift+Insert
        KeyBinding {
            key: named(NamedKey::Insert),
            mods: ctrl,
            action: Action::Copy,
        },
        KeyBinding {
            key: named(NamedKey::Insert),
            mods: shift,
            action: Action::Paste,
        },
        // Config / search
        KeyBinding {
            key: ch("r"),
            mods: ctrl_shift,
            action: Action::ReloadConfig,
        },
        KeyBinding {
            key: ch("f"),
            mods: ctrl_shift,
            action: Action::OpenSearch,
        },
        // Zoom
        KeyBinding {
            key: ch("="),
            mods: ctrl,
            action: Action::ZoomIn,
        },
        KeyBinding {
            key: ch("+"),
            mods: ctrl,
            action: Action::ZoomIn,
        },
        KeyBinding {
            key: ch("-"),
            mods: ctrl,
            action: Action::ZoomOut,
        },
        KeyBinding {
            key: ch("0"),
            mods: ctrl,
            action: Action::ZoomReset,
        },
        // Tabs
        KeyBinding {
            key: ch("t"),
            mods: ctrl,
            action: Action::NewTab,
        },
        KeyBinding {
            key: ch("w"),
            mods: ctrl,
            action: Action::CloseTab,
        },
        KeyBinding {
            key: named(NamedKey::Tab),
            mods: ctrl,
            action: Action::NextTab,
        },
        KeyBinding {
            key: named(NamedKey::Tab),
            mods: ctrl_shift,
            action: Action::PrevTab,
        },
        // Scrollback
        KeyBinding {
            key: named(NamedKey::PageUp),
            mods: shift,
            action: Action::ScrollPageUp,
        },
        KeyBinding {
            key: named(NamedKey::PageDown),
            mods: shift,
            action: Action::ScrollPageDown,
        },
        KeyBinding {
            key: named(NamedKey::Home),
            mods: shift,
            action: Action::ScrollToTop,
        },
        KeyBinding {
            key: named(NamedKey::End),
            mods: shift,
            action: Action::ScrollToBottom,
        },
        // Prompt navigation
        KeyBinding {
            key: named(NamedKey::ArrowUp),
            mods: ctrl_shift,
            action: Action::PreviousPrompt,
        },
        KeyBinding {
            key: named(NamedKey::ArrowDown),
            mods: ctrl_shift,
            action: Action::NextPrompt,
        },
        // Smart copy/paste (Ctrl+C/V without Shift) — must come AFTER
        // Ctrl+Shift variants so those match first.
        KeyBinding {
            key: ch("c"),
            mods: ctrl,
            action: Action::SmartCopy,
        },
        KeyBinding {
            key: ch("v"),
            mods: ctrl,
            action: Action::SmartPaste,
        },
    ]
}
