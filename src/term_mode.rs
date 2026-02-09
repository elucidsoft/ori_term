use bitflags::bitflags;

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
    }
}

impl Default for TermMode {
    fn default() -> Self {
        Self::LINE_WRAP | Self::SHOW_CURSOR
    }
}
