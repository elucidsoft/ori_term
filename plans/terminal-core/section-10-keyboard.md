---
section: "10"
title: Keyboard Protocol
status: complete
goal: Complete keyboard input handling including all key sequences, modifiers, and Kitty keyboard protocol
sections:
  - id: "10.1"
    title: Key Encoding
    status: complete
  - id: "10.2"
    title: Application Mode Keys
    status: complete
  - id: "10.3"
    title: Kitty Keyboard Protocol
    status: complete
  - id: "10.4"
    title: IME Support
    status: not-started
  - id: "10.5"
    title: Completion Checklist
    status: in-progress
---

# Section 10: Keyboard Protocol

**Status:** Complete (10.4 IME deferred)
**Goal:** Complete keyboard input handling: encode all keys correctly for terminal
applications, support application cursor/keypad modes, implement Kitty keyboard
protocol for modern apps, and handle IME input.

**Inspired by:**
- Ghostty's comprehensive key encoding with Kitty protocol support
- Alacritty's key binding system and input handling
- Kitty keyboard protocol specification (progressive enhancement)

**Implementation:** Key encoding logic lives in `src/key_encoding.rs` as pure functions
(no PTY access, fully unit-testable). `app.rs` calls `encode_key()` with the key,
modifiers, terminal mode, text, location, and event type. All legacy inline key
dispatch has been replaced.

---

## 10.1 Key Encoding

Correctly encode all key events for the terminal.

- [x] Standard key encoding (legacy xterm/VT):
  - [x] Arrow keys: `ESC[A/B/C/D` (normal), `ESCOA/B/C/D` (application)
  - [x] Function keys F1-F12: `ESCOP` - `ESC[24~` with correct numbering
  - [x] Home/End: `ESC[H`/`ESC[F` (normal), `ESCOH`/`ESCOF` (application)
  - [x] Insert/Delete/PageUp/PageDown: `ESC[2~`/`ESC[3~`/`ESC[5~`/`ESC[6~`
- [x] Modifier encoding:
  - [x] Ctrl+letter: send control code (Ctrl+A = 0x01, Ctrl+C = 0x03, etc.)
  - [x] Ctrl+special: modify parameter `ESC[1;5A` (Ctrl+Up)
  - [x] Alt+key: send `ESC` prefix then key (Alt+a = `ESC a`)
  - [x] Shift+special: `ESC[1;2A` (Shift+Up)
  - [x] Modifier parameter: `1+modifier_bits` where Shift=1, Alt=2, Ctrl=4, Super=8
  - [x] Alt+Backspace: `ESC DEL`
  - [x] Shift+Tab: `ESC[Z` (backtab)
  - [x] Ctrl+Space: NUL (0x00)
  - [x] Alt+Ctrl combinations: ESC prefix + C0 byte
- [x] Numpad keys: same as regular keys unless application keypad mode
- [x] Application keypad mode (DECKPAM): numpad sends `ESCO` sequences
- [ ] Bracketed paste: `ESC[200~` ... `ESC[201~` wrapping pasted text

**Files:** `src/key_encoding.rs` (encode_key, encode_legacy, ctrl_key_byte, letter_key, tilde_key)

**Ref:** Alacritty input handling, xterm key encoding tables

---

## 10.2 Application Mode Keys

Handle DECSET/DECRST mode changes that affect key encoding.

- [x] DECCKM (DECSET 1): Application cursor keys
  - [x] Normal: `ESC[A` for Up
  - [x] Application: `ESCOA` for Up
  - [x] Modifiers override SS3 → CSI format (Ctrl+Up = `ESC[1;5A` even in app mode)
- [x] DECKPAM/DECKPNM: Application/Normal keypad
  - [x] Application: numpad keys send `ESCOp` through `ESCOy`, Enter `ESCOM`, operators `ESCOk/m/j/n`
  - [x] Normal: numpad keys send their character values
- [ ] DECSET 2004: Bracketed paste mode
  - [ ] Already tracked in TermMode, wire to paste handling
- [ ] DECSET 1007: Alternate scroll
  - [ ] When in alternate screen, convert mouse wheel to Up/Down arrow sequences

**Files:** `src/key_encoding.rs` (encode_numpad_app)

**Ref:** Ghostty mode-dependent key encoding, xterm mode documentation

---

## 10.3 Kitty Keyboard Protocol

Progressive enhancement keyboard protocol for modern terminal applications.

- [x] Protocol flags (5 bits in TermMode, bits 16-20):
  - [x] DISAMBIGUATE_ESC_CODES (bit 16)
  - [x] REPORT_EVENT_TYPES (bit 17)
  - [x] REPORT_ALTERNATE_KEYS (bit 18)
  - [x] REPORT_ALL_KEYS_AS_ESC (bit 19)
  - [x] REPORT_ASSOCIATED_TEXT (bit 20)
- [x] CSI u encoding: `CSI keycode ; modifiers u`
  - [x] Named keys use Kitty-defined codepoints (Escape=27, Enter=13, functional keys 57348+)
  - [x] Character keys use Unicode codepoint
  - [x] Printable chars with no mods still sent as plain text (unless REPORT_ALL_KEYS)
- [x] Event types: `CSI keycode ; modifiers : event-type u`
  - [x] 1 = press (omitted as default), 2 = repeat, 3 = release
  - [x] Key release events pass through app shortcuts to PTY when REPORT_EVENT_TYPES active
- [x] Mode stack management (Handler trait methods):
  - [x] `push_keyboard_mode(mode)`: push onto stack, apply
  - [x] `pop_keyboard_modes(n)`: pop n entries, apply top or clear
  - [x] `set_keyboard_mode(mode, behavior)`: Replace/Union/Difference on top
  - [x] `report_keyboard_mode()`: respond `ESC[?{bits}u`
- [x] Stack save/restore on alt screen switch
- [x] Stack clear on terminal reset

**Files:** `src/key_encoding.rs` (encode_kitty, kitty_codepoint), `src/term_mode.rs` (flags + From<KeyboardModes>), `src/term_handler.rs` (Handler methods), `src/tab.rs` (keyboard_mode_stack)

**Ref:** https://sw.kovidgoyal.net/kitty/keyboard-protocol/, Ghostty Kitty protocol impl

---

## 10.4 IME Support

Input Method Editor for CJK and complex script input. **Deferred.**

- [ ] Enable IME on the winit window
- [ ] Handle `WindowEvent::Ime` events:
  - [ ] `Ime::Preedit(text, cursor)` — show composition text at cursor
  - [ ] `Ime::Commit(text)` — send committed text to PTY
  - [ ] `Ime::Enabled` / `Ime::Disabled` — track IME state
- [ ] Render preedit text: overlay at cursor position with underline
- [ ] Position IME candidate window near cursor (set IME position on cursor move)
- [ ] Don't send raw key events during IME composition

**Ref:** winit IME handling, Alacritty IME support

---

## 10.5 Completion Checklist

- [x] All printable characters encoded correctly
- [x] Arrow keys work in normal and application cursor modes
- [x] F1-F12 function keys work
- [x] Ctrl+letter sends correct control codes
- [x] Alt+key sends ESC prefix
- [x] Modifier combinations on special keys (Ctrl+Shift+Up, etc.)
- [x] Numpad keys work in normal and application keypad modes
- [ ] Bracketed paste wraps pasted text
- [x] Kitty keyboard protocol level 1+ supported
- [ ] IME input works for CJK text
- [x] Key bindings don't conflict with terminal shortcuts

**Tests:** 26 unit tests in `key_encoding.rs` covering Ctrl+letter, Alt prefix, modifier
encoding for named keys, APP_CURSOR mode, APP_KEYPAD numpad, Kitty CSI u encoding,
event types (repeat/release), and legacy release suppression.

**Exit Criteria:** All standard terminal applications receive correct key input.
vim, tmux, and other apps work with correct modifier handling. ~~IME works for
CJK input.~~ (deferred)
