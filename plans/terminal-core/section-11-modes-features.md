---
section: "11"
title: Terminal Modes & Advanced Features
status: not-started
goal: Complete terminal mode support and advanced features like mouse reporting, cursor styles, hyperlinks, and images
sections:
  - id: "11.1"
    title: Mouse Reporting
    status: not-started
  - id: "11.2"
    title: Cursor Styles
    status: not-started
  - id: "11.3"
    title: Focus Events
    status: not-started
  - id: "11.4"
    title: Synchronized Output
    status: not-started
  - id: "11.5"
    title: Hyperlinks
    status: not-started
  - id: "11.6"
    title: Image Protocol
    status: not-started
  - id: "11.7"
    title: Completion Checklist
    status: not-started
---

# Section 11: Terminal Modes & Advanced Features

**Status:** Not Started
**Goal:** Complete the terminal's mode support and implement advanced features
that modern applications expect.

**Inspired by:**
- Ghostty's comprehensive mode handling (`modes.zig`) and feature support
- Alacritty's mouse reporting and cursor style support
- WezTerm's image protocol and hyperlink support

**Current state:** `TermMode` bitflags defined with SHOW_CURSOR, APP_CURSOR,
APP_KEYPAD, LINE_WRAP, ORIGIN, INSERT, ALT_SCREEN, mouse flags, BRACKETED_PASTE,
etc. Basic DECSET/DECRST handling in `term_handler.rs`. No mouse reporting,
no cursor style changes, no focus events, no image support.

---

## 11.1 Mouse Reporting

Report mouse events to applications (for vim, tmux, htop, etc.).

- [ ] Mouse reporting modes (DECSET):
  - [ ] 9: X10 mouse reporting (button press only)
  - [ ] 1000: Normal tracking (press + release)
  - [ ] 1002: Button-event tracking (press + release + drag)
  - [ ] 1003: Any-event tracking (all motion)
- [ ] Mouse encoding formats:
  - [ ] Default: `ESC[M Cb Cx Cy` (limited to 223 columns)
  - [ ] UTF-8 (DECSET 1005): UTF-8 encoded coordinates
  - [ ] SGR (DECSET 1006): `ESC[<Cb;Cx;Cy M/m` — preferred, no coordinate limit
  - [ ] URXVT (DECSET 1015): `ESC[Cb;Cx;Cy M`
- [ ] Button encoding: left=0, middle=1, right=2, wheel up=64, wheel down=65
- [ ] Modifier encoding: Shift+4, Alt+8, Ctrl+16 added to button byte
- [ ] When mouse reporting active, don't handle mouse events for selection
  - [ ] Shift+click bypasses mouse reporting (allows selection)

**Ref:** Ghostty mouse handling, Alacritty mouse reporting, xterm mouse protocol docs

---

## 11.2 Cursor Styles

Support different cursor shapes and blinking.

- [ ] Cursor shapes via DECSCUSR (CSI Ps SP q):
  - [ ] 0/1: blinking block
  - [ ] 2: steady block
  - [ ] 3: blinking underline
  - [ ] 4: steady underline
  - [ ] 5: blinking bar (I-beam)
  - [ ] 6: steady bar
- [ ] Store cursor shape in terminal state
- [ ] Render cursor according to shape:
  - [ ] Block: filled rectangle over cell
  - [ ] Underline: thin bar at bottom of cell
  - [ ] Bar: thin vertical bar at left of cell
- [ ] Blinking: toggle visibility on timer (default 530ms)
  - [ ] Reset blink timer on cursor movement
  - [ ] Configurable blink rate
- [ ] OSC 12: set cursor color
- [ ] Save/restore cursor style with DECSC/DECRC

**Ref:** Ghostty cursor rendering, Alacritty cursor style handling

---

## 11.3 Focus Events

Report focus in/out to applications.

- [ ] DECSET 1004: Enable focus event reporting
- [ ] When window gains focus: send `ESC[I` to PTY
- [ ] When window loses focus: send `ESC[O` to PTY
- [ ] Handle winit `WindowEvent::Focused(bool)`
- [ ] Only send events when mode flag is set
- [ ] Visual: dim terminal slightly when unfocused (optional)

**Ref:** Alacritty focus event handling, xterm focus mode

---

## 11.4 Synchronized Output

Prevent partial frame rendering during rapid output.

- [ ] DCS synchronized output:
  - [ ] Begin sync: `ESC P = 1 s ESC \` — start buffering
  - [ ] End sync: `ESC P = 2 s ESC \` — flush and render
- [ ] When sync mode active:
  - [ ] Buffer PTY output, don't trigger redraws
  - [ ] On end sync: process all buffered output, then redraw once
- [ ] Timeout: if sync mode not ended within ~100ms, flush anyway (prevent hangs)
- [ ] This eliminates flicker for applications that update many cells per frame

**Ref:** Ghostty synchronized output, terminal.app sync protocol

---

## 11.5 Hyperlinks

OSC 8 hyperlink support for clickable URLs.

- [ ] Parse OSC 8 sequences: `OSC 8 ; params ; uri ST`
  - [ ] Start hyperlink: `OSC 8 ; id=foo ; https://example.com ST`
  - [ ] End hyperlink: `OSC 8 ; ; ST`
- [ ] Store hyperlink in `CellExtra` for cells within the hyperlink span
- [ ] Rendering: underline hyperlinked text (or change color on hover)
- [ ] Mouse hover: detect when cursor is over a hyperlinked cell
  - [ ] Show URL in status bar or tooltip
  - [ ] Change cursor to pointing hand
- [ ] Ctrl+click or click: open URL in default browser
- [ ] Auto-detect URLs in terminal output (optional, configurable)

**Ref:** Ghostty hyperlink support, WezTerm hyperlink handling, OSC 8 spec

---

## 11.6 Image Protocol

Display images inline in the terminal.

- [ ] Kitty image protocol (preferred):
  - [ ] Image transmission via APC sequences
  - [ ] Support: direct (base64), file path, shared memory
  - [ ] Image placement: position, size, z-index
  - [ ] Image operations: display, delete, animate
- [ ] Sixel graphics (legacy):
  - [ ] Parse sixel data from DCS sequences
  - [ ] Render sixel images as bitmaps in the terminal grid
- [ ] Image storage:
  - [ ] Cache decoded images in memory
  - [ ] Evict when scrolled out of view or explicitly deleted
  - [ ] Memory limit for image cache
- [ ] Rendering: composite images over cell backgrounds in render pass

**Ref:** Kitty image protocol spec, Ghostty image support, WezTerm image protocols

---

## 11.7 Completion Checklist

- [ ] Mouse reporting works in vim, tmux, htop
- [ ] SGR mouse encoding supported (no coordinate limits)
- [ ] Shift+click bypasses mouse reporting for selection
- [ ] Cursor shape changes (block, underline, bar)
- [ ] Cursor blinking toggles on timer
- [ ] Focus events sent when window focused/unfocused
- [ ] Synchronized output prevents flicker
- [ ] OSC 8 hyperlinks render and are clickable
- [ ] Kitty image protocol displays inline images
- [ ] All modes persist across save/restore cursor

**Exit Criteria:** tmux, vim, and htop all have working mouse support. Cursor
styles change correctly. Applications can detect focus changes.
