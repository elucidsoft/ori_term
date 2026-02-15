//! Character set translation state (G0–G3, single shifts).
//!
//! Wraps `vte::ansi::StandardCharset` and `vte::ansi::CharsetIndex` with a
//! state machine that tracks the active charset slot and single-shift state.
//! DEC special graphics mapping is provided by `vte::ansi::StandardCharset::map`.

pub use vte::ansi::{CharsetIndex, StandardCharset};

/// Character set translation state.
///
/// Tracks four charset slots (G0–G3), which slot is active, and an optional
/// single-shift override for one character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharsetState {
    /// G0–G3 charset slots (default: all ASCII).
    charsets: [StandardCharset; 4],
    /// Currently active charset slot (default: G0).
    active: CharsetIndex,
    /// SS2/SS3 single-shift override — used for one character, then cleared.
    single_shift: Option<CharsetIndex>,
}

impl Default for CharsetState {
    fn default() -> Self {
        Self {
            charsets: [StandardCharset::Ascii; 4],
            active: CharsetIndex::G0,
            single_shift: None,
        }
    }
}

impl CharsetState {
    /// Translate a character through the active charset.
    ///
    /// If a single shift is pending, uses that charset for this one character
    /// and then clears the single shift. Otherwise uses the active charset.
    pub fn translate(&mut self, ch: char) -> char {
        let idx = if let Some(ss) = self.single_shift.take() {
            ss
        } else {
            self.active
        };
        self.charsets[idx as usize].map(ch)
    }

    /// Currently active charset slot.
    pub fn active(&self) -> &CharsetIndex {
        &self.active
    }

    /// Assign a charset to a slot (ESC (, ESC ), ESC *, ESC +).
    pub fn set_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        self.charsets[index as usize] = charset;
    }

    /// Switch the active charset slot (SO/SI control codes).
    pub fn set_active(&mut self, index: CharsetIndex) {
        self.active = index;
    }

    /// Set a single-shift override (SS2/SS3).
    pub fn set_single_shift(&mut self, index: CharsetIndex) {
        self.single_shift = Some(index);
    }
}

#[cfg(test)]
mod tests;
