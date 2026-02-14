//! Key event encoding for terminal input (legacy xterm, Kitty protocol).

mod kitty;
mod legacy;

use bitflags::bitflags;
use winit::keyboard::{Key, KeyLocation};

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
    pub(super) fn xterm_param(self) -> u8 {
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
        return kitty::encode_kitty(key, mods, mode, text, location, event_type);
    }

    // Legacy mode only sends on press.
    if event_type == KeyEventType::Release {
        return Vec::new();
    }

    // APP_KEYPAD numpad keys.
    if location == KeyLocation::Numpad && mode.contains(TermMode::APP_KEYPAD) {
        if let Some(bytes) = legacy::encode_numpad_app(key) {
            return bytes;
        }
    }

    legacy::encode_legacy(key, mods, mode, text)
}

#[cfg(test)]
mod tests {
    use winit::keyboard::{Key, KeyLocation, NamedKey};

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
