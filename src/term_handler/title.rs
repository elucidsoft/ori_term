//! Title, charset, clipboard, and keyboard mode operations.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use vte::ansi::{CharsetIndex, KeyboardModes, KeyboardModesApplyBehavior, StandardCharset};

use super::TermHandler;

impl TermHandler<'_> {
    pub(super) fn handle_set_title(&mut self, title: Option<String>) {
        if *self.suppress_title {
            return;
        }
        if let Some(t) = title {
            *self.title = t;
            *self.has_explicit_title = true;
        }
    }

    pub(super) fn handle_push_title(&mut self) {
        self.title_stack.push(self.title.clone());
    }

    pub(super) fn handle_pop_title(&mut self) {
        if let Some(t) = self.title_stack.pop() {
            *self.title = t;
        }
    }

    pub(super) fn handle_configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        let slot = match index {
            CharsetIndex::G0 => 0,
            CharsetIndex::G1 => 1,
            CharsetIndex::G2 => 2,
            CharsetIndex::G3 => 3,
        };
        self.charset.charsets[slot] = charset;
    }

    pub(super) fn handle_set_active_charset(&mut self, index: CharsetIndex) {
        self.charset.active = index;
    }

    #[allow(clippy::needless_pass_by_ref_mut, reason = "Called via Handler trait which requires &mut self")]
    pub(super) fn handle_clipboard_store(&mut self, _clipboard: u8, data: &[u8]) {
        // OSC 52 clipboard store: data is base64-encoded text from the application.
        // Selector byte (_clipboard) maps c/p/s — all go to system clipboard on Windows.
        if let Ok(decoded) = BASE64.decode(data) {
            if let Ok(text) = String::from_utf8(decoded) {
                crate::clipboard::set_text(&text);
            }
        }
    }

    pub(super) fn handle_clipboard_load(&self, _clipboard: u8, terminator: &str) {
        // OSC 52 clipboard load: respond with base64-encoded clipboard contents.
        // Format: ESC ] 52 ; <selector> ; <base64> <terminator>
        if let Some(text) = crate::clipboard::get_text() {
            let encoded = BASE64.encode(text.as_bytes());
            let response = format!("\x1b]52;c;{encoded}{terminator}");
            self.write_pty(response.as_bytes());
        }
    }

    pub(super) fn handle_substitute(&mut self) {
        // SUB — treated as a space character
        self.active_grid().put_char(' ');
    }

    pub(super) fn handle_report_keyboard_mode(&self) {
        let bits = self
            .keyboard_mode_stack
            .last()
            .copied()
            .unwrap_or(KeyboardModes::NO_MODE);
        let response = format!("\x1b[?{}u", bits.bits());
        self.write_pty(response.as_bytes());
    }

    pub(super) fn handle_push_keyboard_mode(&mut self, mode: KeyboardModes) {
        self.keyboard_mode_stack.push(mode);
        self.apply_keyboard_mode();
    }

    pub(super) fn handle_pop_keyboard_modes(&mut self, to_pop: u16) {
        let new_len = self
            .keyboard_mode_stack
            .len()
            .saturating_sub(to_pop as usize);
        self.keyboard_mode_stack.truncate(new_len);
        self.apply_keyboard_mode();
    }

    pub(super) fn handle_set_keyboard_mode(
        &mut self,
        mode: KeyboardModes,
        behavior: KeyboardModesApplyBehavior,
    ) {
        let current = self
            .keyboard_mode_stack
            .last()
            .copied()
            .unwrap_or(KeyboardModes::NO_MODE);
        let new_mode = match behavior {
            KeyboardModesApplyBehavior::Replace => mode,
            KeyboardModesApplyBehavior::Union => current | mode,
            KeyboardModesApplyBehavior::Difference => current & !mode,
        };
        if let Some(top) = self.keyboard_mode_stack.last_mut() {
            *top = new_mode;
        } else {
            self.keyboard_mode_stack.push(new_mode);
        }
        self.apply_keyboard_mode();
    }
}
