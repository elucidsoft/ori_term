---
section: "10"
title: Keyboard Protocol
status: not-started
goal: Complete keyboard input handling including all key sequences, modifiers, and Kitty keyboard protocol
sections:
  - id: "10.1"
    title: Key Encoding
    status: not-started
  - id: "10.2"
    title: Application Mode Keys
    status: not-started
  - id: "10.3"
    title: Kitty Keyboard Protocol
    status: not-started
  - id: "10.4"
    title: IME Support
    status: not-started
  - id: "10.5"
    title: Completion Checklist
    status: not-started
---

# Section 10: Keyboard Protocol

**Status:** Not Started
**Goal:** Complete keyboard input handling: encode all keys correctly for terminal
applications, support application cursor/keypad modes, implement Kitty keyboard
protocol for modern apps, and handle IME input.

**Inspired by:**
- Ghostty's comprehensive key encoding with Kitty protocol support
- Alacritty's key binding system and input handling
- Kitty keyboard protocol specification (progressive enhancement)

**Current state:** Basic key handling in `app.rs`: printable characters sent as UTF-8,
Enter/Backspace/Tab/Escape send control codes, arrow keys send basic sequences,
APP_CURSOR mode for arrows, F1-F12, Insert, PageUp/Down, Home/End. Missing: modifier
combinations, numpad, many special keys, Kitty protocol.

---

## 10.1 Key Encoding

Correctly encode all key events for the terminal.

- [ ] Standard key encoding (legacy xterm/VT):
  - [ ] Arrow keys: `ESC[A/B/C/D` (normal), `ESCOA/B/C/D` (application)
  - [ ] Function keys F1-F12: `ESCOP` - `ESC[24~` with correct numbering
  - [ ] Home/End: `ESC[H`/`ESC[F` or `ESC[1~`/`ESC[4~` depending on mode
  - [ ] Insert/Delete/PageUp/PageDown: `ESC[2~`/`ESC[3~`/`ESC[5~`/`ESC[6~`
- [ ] Modifier encoding:
  - [ ] Ctrl+letter: send control code (Ctrl+A = 0x01, Ctrl+C = 0x03, etc.)
  - [ ] Ctrl+special: modify parameter `ESC[1;5A` (Ctrl+Up)
  - [ ] Alt+key: send `ESC` prefix then key (Alt+a = `ESC a`)
  - [ ] Shift+special: `ESC[1;2A` (Shift+Up)
  - [ ] Modifier parameter: `1+modifier_bits` where Shift=1, Alt=2, Ctrl=4, Super=8
- [ ] Numpad keys: same as regular keys unless application keypad mode
- [ ] Application keypad mode (DECKPAM): numpad sends `ESCO` sequences
- [ ] Bracketed paste: `ESC[200~` ... `ESC[201~` wrapping pasted text

**Ref:** Alacritty input handling, xterm key encoding tables

---

## 10.2 Application Mode Keys

Handle DECSET/DECRST mode changes that affect key encoding.

- [ ] DECCKM (DECSET 1): Application cursor keys
  - [ ] Normal: `ESC[A` for Up
  - [ ] Application: `ESCOA` for Up
  - [ ] Already partially implemented
- [ ] DECKPAM/DECKPNM: Application/Normal keypad
  - [ ] Application: numpad keys send `ESCOp` through `ESCOy`
  - [ ] Normal: numpad keys send their character values
- [ ] DECSET 2004: Bracketed paste mode
  - [ ] Already tracked in TermMode, wire to paste handling
- [ ] DECSET 1007: Alternate scroll
  - [ ] When in alternate screen, convert mouse wheel to Up/Down arrow sequences
  - [ ] For programs like less that expect arrow keys for scrolling

**Ref:** Ghostty mode-dependent key encoding, xterm mode documentation

---

## 10.3 Kitty Keyboard Protocol

Progressive enhancement keyboard protocol for modern terminal applications.

- [ ] Protocol levels (CSI > flags u):
  - [ ] Level 0: legacy (default)
  - [ ] Level 1: disambiguate escape codes
  - [ ] Level 2: report event types (press, repeat, release)
  - [ ] Level 3: report alternate keys
  - [ ] Level 4: report all keys as escape codes
- [ ] CSI u encoding: `CSI keycode ; modifiers u`
  - [ ] All keys get unique keycodes
  - [ ] Modifier encoding consistent across all keys
  - [ ] Disambiguates Ctrl+I from Tab, Ctrl+M from Enter, etc.
- [ ] Event types: `CSI keycode ; modifiers : event-type u`
  - [ ] 1 = press, 2 = repeat, 3 = release
- [ ] Push/pop keyboard mode stack: apps can push their desired mode, restore on exit
- [ ] Query current mode: `CSI ? u` -> respond with current flags

**Ref:** https://sw.kovidgoyal.net/kitty/keyboard-protocol/, Ghostty Kitty protocol impl

---

## 10.4 IME Support

Input Method Editor for CJK and complex script input.

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

- [ ] All printable characters encoded correctly
- [ ] Arrow keys work in normal and application cursor modes
- [ ] F1-F12 function keys work
- [ ] Ctrl+letter sends correct control codes
- [ ] Alt+key sends ESC prefix
- [ ] Modifier combinations on special keys (Ctrl+Shift+Up, etc.)
- [ ] Numpad keys work in normal and application keypad modes
- [ ] Bracketed paste wraps pasted text
- [ ] Kitty keyboard protocol level 1+ supported
- [ ] IME input works for CJK text
- [ ] Key bindings don't conflict with terminal shortcuts

**Exit Criteria:** All standard terminal applications receive correct key input.
vim, tmux, and other apps work with correct modifier handling. IME works for
CJK input.
