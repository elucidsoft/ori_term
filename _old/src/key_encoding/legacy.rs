//! Legacy xterm-style key encoding and `APP_KEYPAD` numpad sequences.

use winit::keyboard::{Key, NamedKey};

use super::Modifiers;
use crate::term_mode::TermMode;

/// Named key with a letter terminator (SS3 / CSI variant).
struct LetterKey {
    /// The terminator character (e.g. `b'A'` for Up).
    term: u8,
    /// Whether this key uses SS3 (`ESC O`) in no-mod mode. If false, uses `CSI`.
    ss3: bool,
}

/// Named key with a tilde terminator (`CSI {num} ~`).
struct TildeKey {
    num: u8,
}

fn letter_key(key: NamedKey) -> Option<LetterKey> {
    Some(match key {
        NamedKey::ArrowUp => LetterKey {
            term: b'A',
            ss3: true,
        },
        NamedKey::ArrowDown => LetterKey {
            term: b'B',
            ss3: true,
        },
        NamedKey::ArrowRight => LetterKey {
            term: b'C',
            ss3: true,
        },
        NamedKey::ArrowLeft => LetterKey {
            term: b'D',
            ss3: true,
        },
        NamedKey::Home => LetterKey {
            term: b'H',
            ss3: true,
        },
        NamedKey::End => LetterKey {
            term: b'F',
            ss3: true,
        },
        NamedKey::F1 => LetterKey {
            term: b'P',
            ss3: true,
        },
        NamedKey::F2 => LetterKey {
            term: b'Q',
            ss3: true,
        },
        NamedKey::F3 => LetterKey {
            term: b'R',
            ss3: true,
        },
        NamedKey::F4 => LetterKey {
            term: b'S',
            ss3: true,
        },
        _ => return None,
    })
}

fn tilde_key(key: NamedKey) -> Option<TildeKey> {
    Some(match key {
        NamedKey::Insert => TildeKey { num: 2 },
        NamedKey::Delete => TildeKey { num: 3 },
        NamedKey::PageUp => TildeKey { num: 5 },
        NamedKey::PageDown => TildeKey { num: 6 },
        NamedKey::F5 => TildeKey { num: 15 },
        NamedKey::F6 => TildeKey { num: 17 },
        NamedKey::F7 => TildeKey { num: 18 },
        NamedKey::F8 => TildeKey { num: 19 },
        NamedKey::F9 => TildeKey { num: 20 },
        NamedKey::F10 => TildeKey { num: 21 },
        NamedKey::F11 => TildeKey { num: 23 },
        NamedKey::F12 => TildeKey { num: 24 },
        _ => return None,
    })
}

pub(super) fn encode_legacy(
    key: &Key,
    mods: Modifiers,
    mode: TermMode,
    text: Option<&str>,
) -> Vec<u8> {
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let mod_param = mods.xterm_param();

    // Named keys
    if let Key::Named(named) = key {
        // Letter-terminated keys (arrows, Home, End, F1-F4).
        if let Some(lk) = letter_key(*named) {
            return if mod_param > 0 {
                // Modifiers always use CSI format: ESC [ 1 ; {mod} {term}
                format!("\x1b[1;{}{}", mod_param, lk.term as char).into_bytes()
            } else if lk.ss3 && app_cursor {
                vec![0x1b, b'O', lk.term]
            } else {
                vec![0x1b, b'[', lk.term]
            };
        }

        // Tilde-terminated keys (Insert, Delete, PgUp, PgDn, F5-F12).
        if let Some(tk) = tilde_key(*named) {
            return if mod_param > 0 {
                format!("\x1b[{};{}~", tk.num, mod_param).into_bytes()
            } else {
                format!("\x1b[{}~", tk.num).into_bytes()
            };
        }

        // Simple named keys that produce fixed bytes.
        return match named {
            NamedKey::Enter => vec![b'\r'],
            NamedKey::Backspace => {
                if mods.contains(Modifiers::ALT) {
                    vec![0x1b, 0x7f]
                } else {
                    vec![0x7f]
                }
            }
            NamedKey::Tab => {
                if mods.contains(Modifiers::SHIFT) {
                    b"\x1b[Z".to_vec()
                } else {
                    vec![b'\t']
                }
            }
            NamedKey::Escape => vec![0x1b],
            NamedKey::Space => {
                if mods.contains(Modifiers::CONTROL) {
                    // Ctrl+Space = NUL
                    let mut v = Vec::new();
                    if mods.contains(Modifiers::ALT) {
                        v.push(0x1b);
                    }
                    v.push(0x00);
                    v
                } else if mods.contains(Modifiers::ALT) {
                    vec![0x1b, b' ']
                } else {
                    vec![b' ']
                }
            }
            _ => Vec::new(),
        };
    }

    // Character keys
    if let Key::Character(ch) = key {
        let s = ch.as_str();

        // Ctrl+letter → C0 control code.
        if mods.contains(Modifiers::CONTROL) {
            if let Some(c0) = ctrl_key_byte(s) {
                let mut v = Vec::new();
                if mods.contains(Modifiers::ALT) {
                    v.push(0x1b);
                }
                v.push(c0);
                return v;
            }
        }

        // Alt prefix for character keys (without Ctrl).
        if mods.contains(Modifiers::ALT) && !mods.contains(Modifiers::CONTROL) {
            if let Some(t) = text {
                let mut v = vec![0x1b];
                v.extend_from_slice(t.as_bytes());
                return v;
            }
        }
    }

    // Fallback: send the text as-is.
    text.map_or_else(Vec::new, |t| t.as_bytes().to_vec())
}

/// Map a Ctrl+key combination to its C0 control byte.
///
/// Handles a-z, `[`, `\\`, `]`, `^`, `_`, and 2-8 (xterm-compatible).
fn ctrl_key_byte(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 1 {
        return None;
    }
    let b = bytes[0];
    match b {
        // a-z → 0x01-0x1A
        b'a'..=b'z' => Some(b - b'a' + 1),
        b'A'..=b'Z' => Some(b - b'A' + 1),
        b'[' | b'3' => Some(0x1b),  // Ctrl+[ = ESC
        b'\\' | b'4' => Some(0x1c), // Ctrl+\ = FS
        b']' | b'5' => Some(0x1d),  // Ctrl+] = GS
        b'^' | b'6' => Some(0x1e),  // Ctrl+^ = RS
        b'_' | b'7' => Some(0x1f),  // Ctrl+_ = US
        b'`' | b'2' => Some(0x00),  // Ctrl+` = NUL (same as Ctrl+Space)
        b'8' => Some(0x7f),         // Ctrl+8 = DEL
        _ => None,
    }
}

/// Encode numpad keys in `APP_KEYPAD` mode (SS3 sequences).
pub(super) fn encode_numpad_app(key: &Key) -> Option<Vec<u8>> {
    let code = match key {
        Key::Character(c) => match c.as_str() {
            "0" => b'p',
            "1" => b'q',
            "2" => b'r',
            "3" => b's',
            "4" => b't',
            "5" => b'u',
            "6" => b'v',
            "7" => b'w',
            "8" => b'x',
            "9" => b'y',
            "+" => b'k',
            "-" => b'm',
            "*" => b'j',
            "." => b'n',
            _ => return None,
        },
        Key::Named(NamedKey::Enter) => b'M',
        _ => return None,
    };
    Some(vec![0x1b, b'O', code])
}
