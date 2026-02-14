//! Mode management, device queries, and terminal identification.

use std::time::Instant;

use vte::ansi::{Mode, NamedMode, NamedPrivateMode, PrivateMode};

use crate::term_mode::TermMode;

use super::TermHandler;

/// Maps a `NamedPrivateMode` to its DEC parameter number and `TermMode` flag.
///
/// Returns `None` for modes that need special handling (e.g., alt screen swap)
/// or are handled externally (e.g., `SyncUpdate`).
fn private_mode_flag(named: NamedPrivateMode) -> Option<(u32, TermMode)> {
    match named {
        NamedPrivateMode::CursorKeys => Some((1, TermMode::APP_CURSOR)),
        NamedPrivateMode::Origin => Some((6, TermMode::ORIGIN)),
        NamedPrivateMode::LineWrap => Some((7, TermMode::LINE_WRAP)),
        NamedPrivateMode::ShowCursor => Some((25, TermMode::SHOW_CURSOR)),
        NamedPrivateMode::ReportMouseClicks => Some((1000, TermMode::MOUSE_REPORT)),
        NamedPrivateMode::ReportCellMouseMotion => Some((1002, TermMode::MOUSE_MOTION)),
        NamedPrivateMode::ReportAllMouseMotion => Some((1003, TermMode::MOUSE_ALL)),
        NamedPrivateMode::ReportFocusInOut => Some((1004, TermMode::FOCUS_IN_OUT)),
        NamedPrivateMode::Utf8Mouse => Some((1005, TermMode::UTF8_MOUSE)),
        NamedPrivateMode::SgrMouse => Some((1006, TermMode::SGR_MOUSE)),
        NamedPrivateMode::AlternateScroll => Some((1007, TermMode::ALTERNATE_SCROLL)),
        NamedPrivateMode::BracketedPaste => Some((2004, TermMode::BRACKETED_PASTE)),
        _ => None,
    }
}

/// Maps a `NamedMode` to its ANSI parameter number and `TermMode` flag.
fn named_mode_flag(named: NamedMode) -> (u32, TermMode) {
    match named {
        NamedMode::Insert => (4, TermMode::INSERT),
        NamedMode::LineFeedNewLine => (20, TermMode::LINE_FEED_NEW_LINE),
    }
}

impl TermHandler<'_> {
    pub(super) fn handle_set_mode(&mut self, mode: Mode) {
        if let Mode::Named(named) = mode {
            let (_, flag) = named_mode_flag(named);
            self.mode.insert(flag);
        }
    }

    pub(super) fn handle_unset_mode(&mut self, mode: Mode) {
        if let Mode::Named(named) = mode {
            let (_, flag) = named_mode_flag(named);
            self.mode.remove(flag);
        }
    }

    pub(super) fn handle_set_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.swap_alt_screen(true);
            }
            PrivateMode::Named(named) => {
                if let Some((_, flag)) = private_mode_flag(named) {
                    self.mode.insert(flag);
                }
            }
            PrivateMode::Unknown(_) => {}
        }
    }

    pub(super) fn handle_unset_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.restore_primary_screen(true);
            }
            PrivateMode::Named(named) => {
                if let Some((_, flag)) = private_mode_flag(named) {
                    self.mode.remove(flag);
                }
            }
            PrivateMode::Unknown(_) => {}
        }
    }

    pub(super) fn handle_set_keypad_application_mode(&mut self) {
        self.mode.insert(TermMode::APP_KEYPAD);
    }

    pub(super) fn handle_unset_keypad_application_mode(&mut self) {
        self.mode.remove(TermMode::APP_KEYPAD);
    }

    pub(super) fn handle_report_mode(&mut self, mode: Mode) {
        // DECRPM response: CSI Ps; Pm $ y
        // Pm: 1 = set, 2 = reset, 0 = not recognized
        let (param, state) = match mode {
            Mode::Named(named) => {
                let (p, flag) = named_mode_flag(named);
                (p, if self.mode.contains(flag) { 1u8 } else { 2 })
            }
            Mode::Unknown(n) => (n as u32, 0u8),
        };
        let response = format!("\x1b[{param};{state}$y");
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_report_private_mode(&mut self, mode: PrivateMode) {
        // DECRPM response: CSI ? Ps; Pm $ y
        let (param, state) = match mode {
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                let s = if self.mode.contains(TermMode::ALT_SCREEN) { 1u8 } else { 2 };
                (1049u32, s)
            }
            PrivateMode::Named(named) => {
                if let Some((p, flag)) = private_mode_flag(named) {
                    (p, if self.mode.contains(flag) { 1u8 } else { 2 })
                } else {
                    return;
                }
            }
            PrivateMode::Unknown(n) => (n as u32, 0u8),
        };
        let response = format!("\x1b[?{param};{state}$y");
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_device_status(&mut self, status: usize) {
        match status {
            // DSR 5 — Device Status Report: respond "OK"
            5 => {
                self.write_pty(b"\x1b[0n");
            }
            // DSR 6 — Cursor Position Report
            6 => {
                let grid = self.active_grid_ref();
                let response =
                    format!("\x1b[{};{}R", grid.cursor.row + 1, grid.cursor.col + 1,);
                self.write_pty(response.as_bytes());
            }
            _ => {}
        }
    }

    pub(super) fn handle_identify_terminal(&mut self, intermediate: Option<char>) {
        match intermediate {
            // DA2 — Secondary Device Attributes (CSI > c)
            Some('>') => {
                // Report as VT220-compatible: type 1, firmware version 100, ROM 0
                self.write_pty(b"\x1b[>1;100;0c");
            }
            // DA — Primary Device Attributes (CSI c or ESC Z)
            _ => {
                // Report VT220 with ANSI color (62), columns (1), sixel (4), selective erase (6)
                self.write_pty(b"\x1b[?62;22c");
            }
        }
    }

    pub(super) fn handle_text_area_size_chars(&mut self) {
        let grid = self.active_grid_ref();
        let response = format!("\x1b[8;{};{}t", grid.lines, grid.cols);
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_text_area_size_pixels(&mut self) {
        // Report pixel size as CSI 4 ; height ; width t
        // We don't track pixel size in the handler, so report character-based estimate
        let grid = self.active_grid_ref();
        // Approximate: 8px per col, 16px per row (common monospace metrics)
        let response = format!("\x1b[4;{};{}t", grid.lines * 16, grid.cols * 8);
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_bell(&mut self) {
        *self.bell_start = Some(Instant::now());
    }

    pub(super) fn handle_decaln(&mut self) {
        self.active_grid().decaln();
    }

    pub(super) fn handle_reset_state(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = self.active_grid();
        grid.clear_all();
        grid.cursor.reset_attrs();
        *self.mode = TermMode::default();
        *self.active_is_alt = false;
        self.keyboard_mode_stack.clear();
    }
}
