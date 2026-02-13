//! Mode management, device queries, and terminal identification.

use std::time::Instant;

use vte::ansi::{Mode, NamedMode, NamedPrivateMode, PrivateMode};

use crate::term_mode::TermMode;

use super::TermHandler;

impl TermHandler<'_> {
    pub(super) fn handle_set_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.insert(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => {
                self.mode.insert(TermMode::LINE_FEED_NEW_LINE);
            }
            _ => {}
        }
    }

    pub(super) fn handle_unset_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.remove(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => {
                self.mode.remove(TermMode::LINE_FEED_NEW_LINE);
            }
            _ => {}
        }
    }

    pub(super) fn handle_set_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::CursorKeys) => {
                self.mode.insert(TermMode::APP_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::Origin) => {
                self.mode.insert(TermMode::ORIGIN);
            }
            PrivateMode::Named(NamedPrivateMode::LineWrap) => {
                self.mode.insert(TermMode::LINE_WRAP);
            }
            PrivateMode::Named(NamedPrivateMode::ShowCursor) => {
                self.mode.insert(TermMode::SHOW_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::ReportMouseClicks) => {
                self.mode.insert(TermMode::MOUSE_REPORT);
            }
            PrivateMode::Named(NamedPrivateMode::ReportCellMouseMotion) => {
                self.mode.insert(TermMode::MOUSE_MOTION);
            }
            PrivateMode::Named(NamedPrivateMode::ReportAllMouseMotion) => {
                self.mode.insert(TermMode::MOUSE_ALL);
            }
            PrivateMode::Named(NamedPrivateMode::ReportFocusInOut) => {
                self.mode.insert(TermMode::FOCUS_IN_OUT);
            }
            PrivateMode::Named(NamedPrivateMode::SgrMouse) => {
                self.mode.insert(TermMode::SGR_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::Utf8Mouse) => {
                self.mode.insert(TermMode::UTF8_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::AlternateScroll) => {
                self.mode.insert(TermMode::ALTERNATE_SCROLL);
            }
            PrivateMode::Named(NamedPrivateMode::BracketedPaste) => {
                self.mode.insert(TermMode::BRACKETED_PASTE);
            }
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.swap_alt_screen(true);
            }
            // SyncUpdate (mode 2026): handled by vte Processor internally
            _ => {}
        }
    }

    pub(super) fn handle_unset_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::CursorKeys) => {
                self.mode.remove(TermMode::APP_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::Origin) => {
                self.mode.remove(TermMode::ORIGIN);
            }
            PrivateMode::Named(NamedPrivateMode::LineWrap) => {
                self.mode.remove(TermMode::LINE_WRAP);
            }
            PrivateMode::Named(NamedPrivateMode::ShowCursor) => {
                self.mode.remove(TermMode::SHOW_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::ReportMouseClicks) => {
                self.mode.remove(TermMode::MOUSE_REPORT);
            }
            PrivateMode::Named(NamedPrivateMode::ReportCellMouseMotion) => {
                self.mode.remove(TermMode::MOUSE_MOTION);
            }
            PrivateMode::Named(NamedPrivateMode::ReportAllMouseMotion) => {
                self.mode.remove(TermMode::MOUSE_ALL);
            }
            PrivateMode::Named(NamedPrivateMode::ReportFocusInOut) => {
                self.mode.remove(TermMode::FOCUS_IN_OUT);
            }
            PrivateMode::Named(NamedPrivateMode::SgrMouse) => {
                self.mode.remove(TermMode::SGR_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::Utf8Mouse) => {
                self.mode.remove(TermMode::UTF8_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::AlternateScroll) => {
                self.mode.remove(TermMode::ALTERNATE_SCROLL);
            }
            PrivateMode::Named(NamedPrivateMode::BracketedPaste) => {
                self.mode.remove(TermMode::BRACKETED_PASTE);
            }
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.restore_primary_screen(true);
            }
            // SyncUpdate (mode 2026): handled by vte Processor internally
            _ => {}
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
            Mode::Named(NamedMode::Insert) => (
                4,
                if self.mode.contains(TermMode::INSERT) {
                    1
                } else {
                    2
                },
            ),
            Mode::Named(NamedMode::LineFeedNewLine) => (
                20,
                if self.mode.contains(TermMode::LINE_FEED_NEW_LINE) {
                    1
                } else {
                    2
                },
            ),
            Mode::Unknown(n) => (n as u32, 0u8),
        };
        let response = format!("\x1b[{param};{state}$y");
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_report_private_mode(&mut self, mode: PrivateMode) {
        // DECRPM response: CSI ? Ps; Pm $ y
        let (param, state) = match mode {
            PrivateMode::Named(named) => {
                let flag = match named {
                    NamedPrivateMode::CursorKeys => (1, TermMode::APP_CURSOR),
                    NamedPrivateMode::Origin => (6, TermMode::ORIGIN),
                    NamedPrivateMode::LineWrap => (7, TermMode::LINE_WRAP),
                    NamedPrivateMode::ShowCursor => (25, TermMode::SHOW_CURSOR),
                    NamedPrivateMode::ReportMouseClicks => (1000, TermMode::MOUSE_REPORT),
                    NamedPrivateMode::ReportCellMouseMotion => (1002, TermMode::MOUSE_MOTION),
                    NamedPrivateMode::ReportAllMouseMotion => (1003, TermMode::MOUSE_ALL),
                    NamedPrivateMode::ReportFocusInOut => (1004, TermMode::FOCUS_IN_OUT),
                    NamedPrivateMode::Utf8Mouse => (1005, TermMode::UTF8_MOUSE),
                    NamedPrivateMode::SgrMouse => (1006, TermMode::SGR_MOUSE),
                    NamedPrivateMode::AlternateScroll => (1007, TermMode::ALTERNATE_SCROLL),
                    NamedPrivateMode::BracketedPaste => (2004, TermMode::BRACKETED_PASTE),
                    NamedPrivateMode::SwapScreenAndSetRestoreCursor => {
                        (1049, TermMode::ALT_SCREEN)
                    }
                    _ => return,
                };
                (flag.0, if self.mode.contains(flag.1) { 1u8 } else { 2 })
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
