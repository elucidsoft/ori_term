use bitflags::bitflags;
use vte::ansi::KeyboardModes;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TermMode: u32 {
        const SHOW_CURSOR         = 1 << 0;
        const APP_CURSOR          = 1 << 1;
        const APP_KEYPAD          = 1 << 2;
        const LINE_WRAP           = 1 << 3;
        const ORIGIN              = 1 << 4;
        const INSERT              = 1 << 5;
        const ALT_SCREEN          = 1 << 6;
        const MOUSE_REPORT        = 1 << 7;
        const MOUSE_MOTION        = 1 << 8;
        const MOUSE_ALL           = 1 << 9;
        const SGR_MOUSE           = 1 << 10;
        const FOCUS_IN_OUT        = 1 << 11;
        const BRACKETED_PASTE     = 1 << 12;
        const ALTERNATE_SCROLL    = 1 << 13;
        const LINE_FEED_NEW_LINE  = 1 << 14;
        const UTF8_MOUSE          = 1 << 15;

        // Kitty keyboard protocol flags (bits 16-20).
        const DISAMBIGUATE_ESC_CODES  = 1 << 16;
        const REPORT_EVENT_TYPES      = 1 << 17;
        const REPORT_ALTERNATE_KEYS   = 1 << 18;
        const REPORT_ALL_KEYS_AS_ESC  = 1 << 19;
        const REPORT_ASSOCIATED_TEXT   = 1 << 20;

        const KITTY_KEYBOARD_PROTOCOL = Self::DISAMBIGUATE_ESC_CODES.bits()
            | Self::REPORT_EVENT_TYPES.bits()
            | Self::REPORT_ALTERNATE_KEYS.bits()
            | Self::REPORT_ALL_KEYS_AS_ESC.bits()
            | Self::REPORT_ASSOCIATED_TEXT.bits();
    }
}

impl TermMode {
    /// Any mouse reporting mode is active.
    pub const ANY_MOUSE: Self = Self::MOUSE_REPORT.union(Self::MOUSE_MOTION).union(Self::MOUSE_ALL);
}

impl From<KeyboardModes> for TermMode {
    fn from(km: KeyboardModes) -> Self {
        let mut mode = Self::empty();
        if km.contains(KeyboardModes::DISAMBIGUATE_ESC_CODES) {
            mode |= Self::DISAMBIGUATE_ESC_CODES;
        }
        if km.contains(KeyboardModes::REPORT_EVENT_TYPES) {
            mode |= Self::REPORT_EVENT_TYPES;
        }
        if km.contains(KeyboardModes::REPORT_ALTERNATE_KEYS) {
            mode |= Self::REPORT_ALTERNATE_KEYS;
        }
        if km.contains(KeyboardModes::REPORT_ALL_KEYS_AS_ESC) {
            mode |= Self::REPORT_ALL_KEYS_AS_ESC;
        }
        if km.contains(KeyboardModes::REPORT_ASSOCIATED_TEXT) {
            mode |= Self::REPORT_ASSOCIATED_TEXT;
        }
        mode
    }
}

impl Default for TermMode {
    fn default() -> Self {
        Self::LINE_WRAP | Self::SHOW_CURSOR
    }
}
