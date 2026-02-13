//! Key event encoding for terminal input (legacy xterm, Kitty protocol).

use bitflags::bitflags;
use winit::keyboard::{Key, KeyLocation, NamedKey};

use crate::term_mode::TermMode;

/// Key event type for Kitty keyboard protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Press,
    Repeat,
    Release,
}

bitflags! {
    /// Keyboard modifiers for key events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SHIFT   = 0b0001;
        const ALT     = 0b0010;
        const CONTROL = 0b0100;
        const SUPER   = 0b1000;
    }
}

impl Modifiers {
    /// Encodes as xterm modifier parameter (1 + bitmask).
    ///
    /// Returns 0 when no modifiers are active (caller should omit the parameter).
    fn xterm_param(self) -> u8 {
        if self.is_empty() { 0 } else { self.bits() + 1 }
    }
}

/// Encode a key event into bytes to send to the PTY.
///
/// Returns an empty `Vec` if the key should not produce output.
pub fn encode_key(
    key: &Key,
    mods: Modifiers,
    mode: TermMode,
    text: Option<&str>,
    location: KeyLocation,
    event_type: KeyEventType,
) -> Vec<u8> {
    // Kitty keyboard protocol takes priority when any flag is set.
    if mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL) {
        return encode_kitty(key, mods, mode, text, location, event_type);
    }

    // Legacy mode only sends on press.
    if event_type == KeyEventType::Release {
        return Vec::new();
    }

    // APP_KEYPAD numpad keys.
    if location == KeyLocation::Numpad && mode.contains(TermMode::APP_KEYPAD) {
        if let Some(bytes) = encode_numpad_app(key) {
            return bytes;
        }
    }

    encode_legacy(key, mods, mode, text)
}

// Legacy (xterm-style) encoding

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

fn encode_legacy(key: &Key, mods: Modifiers, mode: TermMode, text: Option<&str>) -> Vec<u8> {
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let mod_param = mods.xterm_param();

    // Named keys ----------------------------------------------------------
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

    // Character keys ------------------------------------------------------
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

// APP_KEYPAD numpad encoding (SS3 sequences)

fn encode_numpad_app(key: &Key) -> Option<Vec<u8>> {
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

// Kitty keyboard protocol (CSI u)

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
fn encode_kitty(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn no_mode() -> TermMode {
        TermMode::default()
    }

    fn app_cursor_mode() -> TermMode {
        TermMode::default() | TermMode::APP_CURSOR
    }

    fn app_keypad_mode() -> TermMode {
        TermMode::default() | TermMode::APP_KEYPAD
    }

    fn kitty_disambiguate() -> TermMode {
        TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES
    }

    fn enc(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
        encode_key(
            &key,
            mods,
            mode,
            None,
            KeyLocation::Standard,
            KeyEventType::Press,
        )
    }

    fn enc_text(key: Key, mods: Modifiers, mode: TermMode, text: &str) -> Vec<u8> {
        encode_key(
            &key,
            mods,
            mode,
            Some(text),
            KeyLocation::Standard,
            KeyEventType::Press,
        )
    }

    fn enc_numpad(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
        encode_key(
            &key,
            mods,
            mode,
            None,
            KeyLocation::Numpad,
            KeyEventType::Press,
        )
    }

    fn enc_release(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
        encode_key(
            &key,
            mods,
            mode,
            None,
            KeyLocation::Standard,
            KeyEventType::Release,
        )
    }

    // Ctrl+letter C0 codes

    #[test]
    fn ctrl_a() {
        let r = enc(Key::Character("a".into()), Modifiers::CONTROL, no_mode());
        assert_eq!(r, vec![0x01]);
    }

    #[test]
    fn ctrl_c() {
        let r = enc(Key::Character("c".into()), Modifiers::CONTROL, no_mode());
        assert_eq!(r, vec![0x03]);
    }

    #[test]
    fn ctrl_d() {
        let r = enc(Key::Character("d".into()), Modifiers::CONTROL, no_mode());
        assert_eq!(r, vec![0x04]);
    }

    #[test]
    fn ctrl_z() {
        let r = enc(Key::Character("z".into()), Modifiers::CONTROL, no_mode());
        assert_eq!(r, vec![0x1a]);
    }

    #[test]
    fn ctrl_space() {
        let r = enc(Key::Named(NamedKey::Space), Modifiers::CONTROL, no_mode());
        assert_eq!(r, vec![0x00]);
    }

    // Alt prefix

    #[test]
    fn alt_a() {
        let r = enc_text(Key::Character("a".into()), Modifiers::ALT, no_mode(), "a");
        assert_eq!(r, vec![0x1b, b'a']);
    }

    #[test]
    fn alt_ctrl_a() {
        let r = enc(
            Key::Character("a".into()),
            Modifiers::ALT | Modifiers::CONTROL,
            no_mode(),
        );
        assert_eq!(r, vec![0x1b, 0x01]);
    }

    // Modifier-encoded named keys

    #[test]
    fn ctrl_up() {
        let r = enc(Key::Named(NamedKey::ArrowUp), Modifiers::CONTROL, no_mode());
        assert_eq!(r, b"\x1b[1;5A");
    }

    #[test]
    fn shift_right() {
        let r = enc(
            Key::Named(NamedKey::ArrowRight),
            Modifiers::SHIFT,
            no_mode(),
        );
        assert_eq!(r, b"\x1b[1;2C");
    }

    #[test]
    fn ctrl_shift_left() {
        let r = enc(
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::CONTROL | Modifiers::SHIFT,
            no_mode(),
        );
        assert_eq!(r, b"\x1b[1;6D");
    }

    #[test]
    fn ctrl_f5() {
        let r = enc(Key::Named(NamedKey::F5), Modifiers::CONTROL, no_mode());
        assert_eq!(r, b"\x1b[15;5~");
    }

    #[test]
    fn shift_f1() {
        let r = enc(Key::Named(NamedKey::F1), Modifiers::SHIFT, no_mode());
        assert_eq!(r, b"\x1b[1;2P");
    }

    #[test]
    fn ctrl_delete() {
        let r = enc(Key::Named(NamedKey::Delete), Modifiers::CONTROL, no_mode());
        assert_eq!(r, b"\x1b[3;5~");
    }

    #[test]
    fn ctrl_page_up() {
        let r = enc(Key::Named(NamedKey::PageUp), Modifiers::CONTROL, no_mode());
        assert_eq!(r, b"\x1b[5;5~");
    }

    // APP_CURSOR mode

    #[test]
    fn app_cursor_up_no_mods() {
        let r = enc(
            Key::Named(NamedKey::ArrowUp),
            Modifiers::empty(),
            app_cursor_mode(),
        );
        assert_eq!(r, b"\x1bOA");
    }

    #[test]
    fn app_cursor_up_with_ctrl() {
        // Modifiers override SS3 — use CSI format.
        let r = enc(
            Key::Named(NamedKey::ArrowUp),
            Modifiers::CONTROL,
            app_cursor_mode(),
        );
        assert_eq!(r, b"\x1b[1;5A");
    }

    // Unmodified basic keys

    #[test]
    fn enter() {
        assert_eq!(
            enc(Key::Named(NamedKey::Enter), Modifiers::empty(), no_mode()),
            b"\r"
        );
    }

    #[test]
    fn backspace() {
        assert_eq!(
            enc(
                Key::Named(NamedKey::Backspace),
                Modifiers::empty(),
                no_mode()
            ),
            vec![0x7f]
        );
    }

    #[test]
    fn tab() {
        assert_eq!(
            enc(Key::Named(NamedKey::Tab), Modifiers::empty(), no_mode()),
            b"\t"
        );
    }

    #[test]
    fn shift_tab() {
        assert_eq!(
            enc(Key::Named(NamedKey::Tab), Modifiers::SHIFT, no_mode()),
            b"\x1b[Z"
        );
    }

    #[test]
    fn escape() {
        assert_eq!(
            enc(Key::Named(NamedKey::Escape), Modifiers::empty(), no_mode()),
            vec![0x1b]
        );
    }

    #[test]
    fn alt_backspace() {
        assert_eq!(
            enc(Key::Named(NamedKey::Backspace), Modifiers::ALT, no_mode()),
            vec![0x1b, 0x7f]
        );
    }

    // Plain text fallback

    #[test]
    fn plain_text() {
        let r = enc_text(
            Key::Character("x".into()),
            Modifiers::empty(),
            no_mode(),
            "x",
        );
        assert_eq!(r, b"x");
    }

    // APP_KEYPAD numpad

    #[test]
    fn numpad_5_app_keypad() {
        let r = enc_numpad(
            Key::Character("5".into()),
            Modifiers::empty(),
            app_keypad_mode(),
        );
        assert_eq!(r, b"\x1bOu");
    }

    #[test]
    fn numpad_5_no_app_keypad() {
        let r = enc_numpad(Key::Character("5".into()), Modifiers::empty(), no_mode());
        // Without APP_KEYPAD, just sends the digit as text (fallback).
        // The encode_key path will fall through to text, but text is None so empty.
        assert!(r.is_empty());
    }

    #[test]
    fn numpad_enter_app_keypad() {
        let r = enc_numpad(
            Key::Named(NamedKey::Enter),
            Modifiers::empty(),
            app_keypad_mode(),
        );
        assert_eq!(r, b"\x1bOM");
    }

    #[test]
    fn non_numpad_5_app_keypad() {
        // Standard location — APP_KEYPAD should not affect it.
        let r = enc_text(
            Key::Character("5".into()),
            Modifiers::empty(),
            app_keypad_mode(),
            "5",
        );
        assert_eq!(r, b"5");
    }

    // Kitty keyboard protocol

    #[test]
    fn kitty_escape() {
        let r = enc(
            Key::Named(NamedKey::Escape),
            Modifiers::empty(),
            kitty_disambiguate(),
        );
        assert_eq!(r, b"\x1b[27u");
    }

    #[test]
    fn kitty_ctrl_a() {
        let r = enc(
            Key::Character("a".into()),
            Modifiers::CONTROL,
            kitty_disambiguate(),
        );
        assert_eq!(r, b"\x1b[97;5u");
    }

    #[test]
    fn kitty_plain_text() {
        // Printable char with no mods — should send as plain text, not CSI u.
        let r = enc_text(
            Key::Character("a".into()),
            Modifiers::empty(),
            kitty_disambiguate(),
            "a",
        );
        assert_eq!(r, b"a");
    }

    #[test]
    fn kitty_enter() {
        let r = enc(
            Key::Named(NamedKey::Enter),
            Modifiers::empty(),
            kitty_disambiguate(),
        );
        assert_eq!(r, b"\x1b[13u");
    }

    #[test]
    fn kitty_shift_tab() {
        let r = enc(
            Key::Named(NamedKey::Tab),
            Modifiers::SHIFT,
            kitty_disambiguate(),
        );
        assert_eq!(r, b"\x1b[9;2u");
    }

    // Kitty event types

    #[test]
    fn kitty_release_without_report_events() {
        // DISAMBIGUATE only — release should produce nothing.
        let r = enc_release(
            Key::Named(NamedKey::Escape),
            Modifiers::empty(),
            kitty_disambiguate(),
        );
        assert!(r.is_empty());
    }

    #[test]
    fn kitty_release_with_report_events() {
        let mode =
            TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let r = enc_release(Key::Named(NamedKey::Escape), Modifiers::empty(), mode);
        assert_eq!(r, b"\x1b[27;1:3u");
    }

    #[test]
    fn kitty_repeat() {
        let mode =
            TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let r = encode_key(
            &Key::Character("a".into()),
            Modifiers::empty(),
            mode,
            Some("a"),
            KeyLocation::Standard,
            KeyEventType::Repeat,
        );
        assert_eq!(r, b"\x1b[97;1:2u");
    }

    // Legacy release produces nothing

    #[test]
    fn legacy_release_empty() {
        let r = enc_release(Key::Named(NamedKey::ArrowUp), Modifiers::empty(), no_mode());
        assert!(r.is_empty());
    }
}
