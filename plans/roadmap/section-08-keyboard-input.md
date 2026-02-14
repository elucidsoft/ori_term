---
section: 8
title: Keyboard Input
status: not-started
tier: 3
goal: Legacy + Kitty keyboard encoding, keyboard dispatch, IME support
sections:
  - id: "8.1"
    title: Legacy Key Encoding
    status: not-started
  - id: "8.2"
    title: Kitty Keyboard Protocol
    status: not-started
  - id: "8.3"
    title: Keyboard Input Dispatch
    status: not-started
  - id: "8.4"
    title: Section Completion
    status: not-started
---

# Section 08: Keyboard Input

**Status:** Not Started
**Goal:** Encode all key events correctly for terminal applications using both legacy xterm/VT sequences and the Kitty keyboard protocol. Wire keyboard dispatch through keybindings to PTY output with IME support for CJK input.

**Crate:** `oriterm` (binary)
**Dependencies:** `winit` (key events), `oriterm_core` (TermMode)
**Reference:** `_old/src/key_encoding/legacy.rs`, `_old/src/key_encoding/kitty.rs`, `_old/src/app/input_keyboard.rs`

**Prerequisite:** Section 03 complete (PTY running, event loop accepting input). Section 02 complete (TermMode flags available for mode-dependent encoding).

---

## 8.1 Legacy Key Encoding

Correctly encode all key events for the terminal using legacy xterm/VT sequences. This is the baseline encoding used when Kitty protocol is not active.

**File:** `oriterm/src/key_encoding/legacy.rs`

**Reference:** `_old/src/key_encoding/legacy.rs` — carries forward the proven LetterKey/TildeKey dispatch tables.

- [ ] `encode_legacy(key: &KeyEvent, mode: TermMode) -> Option<Vec<u8>>`
  - [ ] Main entry point: inspects key, modifiers, and terminal mode to produce the byte sequence
  - [ ] Returns `None` for keys that should not be sent (e.g., bare modifier keys)
- [ ] **Regular text input**:
  - [ ] Printable characters: send UTF-8 bytes directly
  - [ ] Enter: send `\r` (or `\r\n` if LINEFEED_MODE active)
  - [ ] Backspace: send `\x7f` (DEL) by default, `\x08` (BS) if DECBKM active
  - [ ] Tab: send `\t`; Shift+Tab (backtab): send `ESC[Z`
  - [ ] Escape: send `\x1b`
  - [ ] Space: send `\x20`; Ctrl+Space: send `\x00` (NUL)
- [ ] **Arrow keys** (mode-dependent DECCKM):
  - [ ] Normal mode: `ESC[A` (Up), `ESC[B` (Down), `ESC[C` (Right), `ESC[D` (Left)
  - [ ] Application cursor mode (DECCKM): `ESCOA`, `ESCOB`, `ESCOC`, `ESCOD`
  - [ ] Modifiers override SS3 to CSI format: Ctrl+Up = `ESC[1;5A` (even in app mode)
- [ ] **Home/End**:
  - [ ] Normal: `ESC[H` / `ESC[F`
  - [ ] Application cursor mode: `ESCOH` / `ESCOF`
  - [ ] With modifiers: `ESC[1;{mod}H` / `ESC[1;{mod}F`
- [ ] **PageUp/PageDown, Insert/Delete**:
  - [ ] PageUp: `ESC[5~`, PageDown: `ESC[6~`
  - [ ] Insert: `ESC[2~`, Delete: `ESC[3~`
  - [ ] With modifiers: `ESC[5;{mod}~` etc.
- [ ] **Function keys F1-F12**:
  - [ ] F1-F4 use SS3: `ESCOP`, `ESCOQ`, `ESCOR`, `ESCOS`
  - [ ] F5-F12 use tilde: `ESC[15~`, `ESC[17~`, `ESC[18~`, `ESC[19~`, `ESC[20~`, `ESC[21~`, `ESC[23~`, `ESC[24~`
  - [ ] With modifiers: F1 = `ESC[1;{mod}P`, F5 = `ESC[15;{mod}~`
- [ ] **Ctrl+letter** — send C0 control codes:
  - [ ] Ctrl+A = `\x01`, Ctrl+B = `\x02`, ..., Ctrl+Z = `\x1A`
  - [ ] Ctrl+[ = `\x1b` (ESC), Ctrl+] = `\x1d`, Ctrl+\ = `\x1c`
  - [ ] Ctrl+/ = `\x1f`, Ctrl+@ = `\x00`
- [ ] **Alt+key** — ESC prefix:
  - [ ] Alt+a = `\x1b a`, Alt+A = `\x1b A`
  - [ ] Alt+Backspace = `\x1b \x7f`
  - [ ] Alt+Ctrl combinations: ESC prefix + C0 byte (Alt+Ctrl+A = `\x1b \x01`)
- [ ] **Modifier parameter encoding** (for named keys with modifiers):
  - [ ] Parameter = `1 + modifier_bits` where Shift=1, Alt=2, Ctrl=4, Super=8
  - [ ] Example: Ctrl+Shift+Up = `ESC[1;6A` (1 + 1 + 4 = 6)
- [ ] **Application keypad mode** (DECKPAM/DECKPNM):
  - [ ] Normal: numpad keys send their character values
  - [ ] Application: numpad sends `ESCOp` through `ESCOy`, Enter = `ESCOM`, operators `ESCOk/m/j/n`
- [ ] Helper structs:
  - [ ] `LetterKey { term: u8, ss3: bool }` — named key with letter terminator
  - [ ] `TildeKey { num: u8 }` — named key with tilde terminator
  - [ ] `fn letter_key(key: NamedKey) -> Option<LetterKey>` — lookup table
  - [ ] `fn tilde_key(key: NamedKey) -> Option<TildeKey>` — lookup table
  - [ ] `fn ctrl_key_byte(key: &Key) -> Option<u8>` — Ctrl+key to C0 byte
- [ ] **Tests** (`oriterm/src/key_encoding/legacy.rs` `#[cfg(test)]`):
  - [ ] Arrow Up in normal mode produces `ESC[A`
  - [ ] Arrow Up in application cursor mode produces `ESCOA`
  - [ ] Ctrl+Up produces `ESC[1;5A` (modifier parameter)
  - [ ] Ctrl+C produces `\x03`
  - [ ] Ctrl+A produces `\x01`
  - [ ] Alt+A produces `\x1b A`
  - [ ] Alt+Backspace produces `\x1b \x7f`
  - [ ] Shift+Tab produces `ESC[Z`
  - [ ] Shift+F5 produces `ESC[15;2~`
  - [ ] Home in normal mode produces `ESC[H`
  - [ ] Home in application cursor mode produces `ESCOH`
  - [ ] F1 produces `ESCOP`, F5 produces `ESC[15~`
  - [ ] Enter produces `\r`
  - [ ] Numpad in application keypad mode sends ESC O sequences

---

## 8.2 Kitty Keyboard Protocol

Progressive enhancement keyboard protocol for modern terminal applications. Encodes keys in CSI u format with mode-dependent behavior.

**File:** `oriterm/src/key_encoding/kitty.rs`

**Reference:** `_old/src/key_encoding/kitty.rs`, Kitty keyboard protocol specification (https://sw.kovidgoyal.net/kitty/keyboard-protocol/)

- [ ] `encode_kitty(key: &KeyEvent, mode_flags: u8, term_mode: TermMode) -> Option<Vec<u8>>`
  - [ ] Main entry point: encodes key events using CSI u format
  - [ ] `mode_flags` from `term.keyboard_mode_stack` top entry
  - [ ] Returns `None` for plain printable chars without modifiers when REPORT_ALL_KEYS inactive
- [ ] **Mode flags** (5 progressive enhancement levels):
  - [ ] Bit 0 — DISAMBIGUATE_ESC_CODES (1): use CSI u for ambiguous keys
  - [ ] Bit 1 — REPORT_EVENT_TYPES (2): report press/repeat/release
  - [ ] Bit 2 — REPORT_ALTERNATE_KEYS (4): report shifted/base key variants
  - [ ] Bit 3 — REPORT_ALL_KEYS_AS_ESC (8): encode all keys including plain text as CSI u
  - [ ] Bit 4 — REPORT_ASSOCIATED_TEXT (16): report text generated by key
- [ ] **CSI u encoding format**: `ESC [ keycode ; modifiers u`
  - [ ] Extended: `ESC [ keycode ; modifiers : event_type u`
  - [ ] With text: `ESC [ keycode ; modifiers : event_type ; text u`
- [ ] **Keycode mapping** (`fn kitty_codepoint(key: NamedKey) -> Option<u32>`):
  - [ ] Escape=27, Enter=13, Tab=9, Backspace=127
  - [ ] Insert=57348, Delete=57349, Left=57350, Right=57351, Up=57352, Down=57353
  - [ ] PageUp=57354, PageDown=57355, Home=57356, End=57357
  - [ ] F1=57364 through F35=57398
  - [ ] CapsLock=57358, ScrollLock=57359, NumLock=57360
  - [ ] Character keys: use Unicode codepoint directly
- [ ] **Modifier encoding**:
  - [ ] Modifier parameter = `1 + bits` where Shift=1, Alt=2, Ctrl=4, Super=8
  - [ ] Omit modifier parameter if value is 1 (no modifiers) and no event type needed
- [ ] **Event types** (when REPORT_EVENT_TYPES active):
  - [ ] 1 = press (omitted as default when REPORT_EVENT_TYPES not active)
  - [ ] 2 = repeat
  - [ ] 3 = release
  - [ ] Format: `ESC [ keycode ; modifiers : event_type u`
  - [ ] Key release events pass through app shortcuts to PTY when REPORT_EVENT_TYPES active
- [ ] **Mode stack management** (wired through VTE Handler trait):
  - [ ] `push_keyboard_mode(mode)` — push onto stack, apply
  - [ ] `pop_keyboard_modes(n)` — pop n entries, apply top or clear
  - [ ] `set_keyboard_mode(mode, behavior)` — Replace/Union/Difference on top
  - [ ] `report_keyboard_mode()` — respond `ESC[?{bits}u`
  - [ ] Stack save/restore on alt screen switch
  - [ ] Stack clear on terminal reset
- [ ] **Tests** (`oriterm/src/key_encoding/kitty.rs` `#[cfg(test)]`):
  - [ ] `'a'` with mode 1 (disambiguate): plain `a` (no encoding needed, not ambiguous)
  - [ ] Ctrl+A with mode 1: `ESC[97;5u` (codepoint 97, modifier 5)
  - [ ] Enter with mode 1: `ESC[13u` (disambiguated from legacy)
  - [ ] Escape with mode 1: `ESC[27u`
  - [ ] Key release with mode 2: `ESC[97;1:3u` (event type 3)
  - [ ] Key repeat with mode 2: `ESC[97;1:2u`
  - [ ] `'a'` with mode 8 (report all): `ESC[97u`
  - [ ] F1 with mode 1: `ESC[57364u`
  - [ ] Shift+A with mode 1: `ESC[97;2u`

---

## 8.3 Keyboard Input Dispatch

Route keyboard events through keybindings, then through key encoding, then to the PTY. Single decision tree: each input event handled by exactly one handler.

**File:** `oriterm/src/app/input_keyboard.rs`

**Reference:** `_old/src/app/input_keyboard.rs`

- [ ] `handle_keyboard_input(&mut self, event: &KeyEvent, modifiers: &Modifiers)`
  - [ ] Main entry point called from the winit event loop on `WindowEvent::KeyboardInput`
- [ ] **Dispatch priority** (first match wins):
  1. [ ] Check keybindings table: if key+modifiers match a bound action, execute the action and return
  2. [ ] Check Kitty keyboard mode on active tab:
     - [ ] Read `keyboard_mode_stack` from active tab's terminal state
     - [ ] If Kitty mode active: call `encode_kitty()`, send result to PTY
     - [ ] If REPORT_EVENT_TYPES active: also send release events
  3. [ ] Fall through to legacy encoding:
     - [ ] Call `encode_legacy()`, send result to PTY
  4. [ ] If encoding returns None: key not handled (bare modifier press, etc.)
- [ ] **Cursor blink reset**:
  - [ ] On any keypress that sends to PTY: reset cursor blink timer (cursor becomes visible)
- [ ] **Scroll to bottom on input**:
  - [ ] If display_offset > 0 (viewing scrollback): scroll to live position on keypress
- [ ] **Smart Ctrl+C**:
  - [ ] If selection exists and Ctrl+C pressed: copy selection to clipboard, do NOT send SIGINT
  - [ ] If no selection and Ctrl+C pressed: send `\x03` to PTY
- [ ] **IME handling** (`WindowEvent::Ime`):
  - [ ] `Ime::Commit(text)`: send committed text bytes to PTY
  - [ ] `Ime::Preedit(text, cursor)`: display composition text at cursor position (overlay rendering)
  - [ ] `Ime::Enabled` / `Ime::Disabled`: track IME state, suppress raw key events during composition
  - [ ] Position IME candidate window near terminal cursor (call `window.set_ime_cursor_area()`)
  - [ ] Don't send raw key events to PTY during active IME preedit
- [ ] **Tests** (`oriterm/src/app/input_keyboard.rs` `#[cfg(test)]`):
  - [ ] Keybinding takes priority over PTY send
  - [ ] Kitty mode takes priority over legacy encoding
  - [ ] Ctrl+C with selection copies, without selection sends `\x03`
  - [ ] IME commit sends text to PTY

---

## 8.4 Section Completion

- [ ] All 8.1-8.3 items complete
- [ ] `cargo test -p oriterm --target x86_64-pc-windows-gnu` — key encoding tests pass
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] All printable characters encoded correctly
- [ ] Arrow keys work in both normal and application cursor modes
- [ ] F1-F12 function keys produce correct sequences
- [ ] Ctrl+letter sends correct C0 control codes
- [ ] Alt+key sends ESC prefix correctly
- [ ] Modifier combinations on special keys produce correct parameter encoding
- [ ] Numpad keys work in both normal and application keypad modes
- [ ] Kitty keyboard protocol level 1+ supported (all 5 mode flags)
- [ ] Key release/repeat events reported when REPORT_EVENT_TYPES active
- [ ] Keybinding dispatch has priority over PTY encoding
- [ ] IME commit text reaches PTY
- [ ] Smart Ctrl+C works (copy if selection, SIGINT if not)

**Exit Criteria:** All standard terminal applications receive correct key input. vim, tmux, htop, and other apps work with correct modifier handling. Kitty protocol apps (e.g., kitty-based tools) receive properly encoded events.
