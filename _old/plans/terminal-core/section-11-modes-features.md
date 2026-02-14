---
section: "11"
title: Terminal Modes & Advanced Features
status: in-progress
goal: Complete terminal mode support and advanced features like mouse reporting, cursor styles, hyperlinks, and images
sections:
  - id: "11.1"
    title: Mouse Reporting
    status: complete
  - id: "11.2"
    title: Cursor Styles
    status: complete
  - id: "11.3"
    title: Focus Events
    status: complete
  - id: "11.4"
    title: Synchronized Output
    status: complete
  - id: "11.5"
    title: Hyperlinks
    status: complete
  - id: "11.6"
    title: Image Protocol
    status: not-started
  - id: "11.7"
    title: Completion Checklist
    status: in-progress
---

# Section 11: Terminal Modes & Advanced Features

**Status:** In Progress (11.1–11.4 complete, 11.5–11.6 deferred)
**Goal:** Complete the terminal's mode support and implement advanced features
that modern applications expect.

**Inspired by:**
- Ghostty's comprehensive mode handling (`modes.zig`) and feature support
- Alacritty's mouse reporting and cursor style support
- WezTerm's image protocol and hyperlink support

**Current state:** Mouse reporting (SGR, UTF-8, normal encoding), cursor styles
(block/beam/underline), focus events, and synchronized output are all implemented.
Hyperlinks (11.5) and image protocol (11.6) are deferred.

---

## 11.1 Mouse Reporting

Report mouse events to applications (for vim, tmux, htop, etc.).

- [x] Mouse reporting modes (DECSET):
  - [ ] 9: X10 mouse reporting (button press only) — not implemented
  - [x] 1000: Normal tracking (press + release)
  - [x] 1002: Button-event tracking (press + release + drag)
  - [x] 1003: Any-event tracking (all motion)
- [x] Mouse encoding formats:
  - [x] Default: `ESC[M Cb Cx Cy` (limited to 223 columns)
  - [x] UTF-8 (DECSET 1005): UTF-8 encoded coordinates
  - [x] SGR (DECSET 1006): `ESC[<Cb;Cx;Cy M/m` — preferred, no coordinate limit
  - [ ] URXVT (DECSET 1015): `ESC[Cb;Cx;Cy M` — not implemented
- [x] Button encoding: left=0, middle=1, right=2, wheel up=64, wheel down=65
- [x] Modifier encoding: Shift+4, Alt+8, Ctrl+16 added to button byte
- [x] When mouse reporting active, don't handle mouse events for selection
  - [x] Shift+click bypasses mouse reporting (allows selection)
- [x] Scroll events reported as button 64/65
- [x] Alternate scroll mode: scroll converted to arrow keys in alt screen
- [x] Motion dedup: only report when cell position changes (`last_mouse_cell`)
- [x] `TermMode::ANY_MOUSE` helper constant

**Implementation:** `app.rs` — `send_mouse_report()`, intercepts in `handle_mouse_input`,
`handle_cursor_moved`, `handle_mouse_wheel`. `term_mode.rs` — `ANY_MOUSE` constant.

**Ref:** Ghostty mouse handling, Alacritty mouse reporting, xterm mouse protocol docs

---

## 11.2 Cursor Styles

Support different cursor shapes and blinking.

- [x] Cursor shapes via DECSCUSR (CSI Ps SP q):
  - [x] 0/1: blinking block
  - [x] 2: steady block
  - [x] 3: blinking underline
  - [x] 4: steady underline
  - [x] 5: blinking bar (I-beam)
  - [x] 6: steady bar
- [x] Store cursor shape in terminal state (`tab.cursor_shape`)
- [x] Render cursor according to shape:
  - [x] Block: filled rectangle over cell (inverts text color)
  - [x] Underline: 2px bar at bottom of cell
  - [x] Bar: 2px vertical bar at left of cell
- [ ] Blinking: toggle visibility on timer (default 530ms) — deferred
  - [ ] Reset blink timer on cursor movement
  - [ ] Configurable blink rate
- [x] OSC 12: set cursor color (already handled in `dynamic_color_sequence`)
- [ ] Save/restore cursor style with DECSC/DECRC — deferred

**Implementation:** `gpu/renderer.rs` — `cursor_shape` in `FrameParams`, shape-aware
rendering in `build_grid_instances`. Conditional text inversion only for Block.
`term_handler.rs` — `set_cursor_style`/`set_cursor_shape` already handled by vte.

**Ref:** Ghostty cursor rendering, Alacritty cursor style handling

---

## 11.3 Focus Events

Report focus in/out to applications.

- [x] DECSET 1004: Enable focus event reporting
- [x] When window gains focus: send `ESC[I` to PTY
- [x] When window loses focus: send `ESC[O` to PTY
- [x] Handle winit `WindowEvent::Focused(bool)`
- [x] Only send events when mode flag is set
- [x] Settings window excluded from focus reporting
- [ ] Visual: dim terminal slightly when unfocused (optional) — deferred

**Implementation:** `app.rs` — `WindowEvent::Focused` arm in `window_event`.

**Ref:** Alacritty focus event handling, xterm focus mode

---

## 11.4 Synchronized Output

Prevent partial frame rendering during rapid output.

- [x] Mode 2026 (SyncUpdate): handled internally by vte 0.15 `Processor`
  - [x] vte buffers handler calls between BSU/ESU and dispatches as one batch
  - [x] Since `process_output` calls `processor.advance()` in a loop then requests
    one redraw, synchronized output works correctly out of the box
- [x] Explicit documentation comments in `set_private_mode`/`unset_private_mode`

**Implementation:** No code changes needed — vte 0.15 handles it. Comments added
to `term_handler.rs` for documentation.

**Ref:** Ghostty synchronized output, terminal.app sync protocol

---

## 11.5 Hyperlinks

OSC 8 hyperlink support for clickable URLs.

- [x] Parse OSC 8 sequences: `OSC 8 ; params ; uri ST` (handled by vte)
  - [x] Start hyperlink: `OSC 8 ; id=foo ; https://example.com ST`
  - [x] End hyperlink: `OSC 8 ; ; ST`
- [x] Store hyperlink in `CellExtra` for cells within the hyperlink span
- [x] Rendering: dotted underline on hyperlinked text, solid underline on hover
- [x] Mouse hover: detect when Ctrl held and cursor is over a hyperlinked cell
  - [x] Change cursor to pointing hand (`CursorIcon::Pointer`)
- [x] Ctrl+click: open URL in default browser (platform Command)
  - [x] URL scheme validation (http/https/ftp/file only)
- [x] Auto-detect plain-text URLs in terminal output (implicit URL detection)
  - [x] Regex-based URL detection across soft-wrapped logical lines
  - [x] Lazy detection on Ctrl+hover/click with per-logical-line caching
  - [x] Ctrl+hover shows pointer cursor + solid underline on full URL span
  - [x] Ctrl+click opens detected URL in default browser
  - [x] Skips cells with existing OSC 8 hyperlinks
  - [x] Handles Wikipedia-style parenthesized URLs, strips trailing punctuation

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

- [x] Mouse reporting works in vim, tmux, htop
- [x] SGR mouse encoding supported (no coordinate limits)
- [x] Shift+click bypasses mouse reporting for selection
- [x] Cursor shape changes (block, underline, bar)
- [ ] Cursor blinking toggles on timer — deferred
- [x] Focus events sent when window focused/unfocused
- [x] Synchronized output prevents flicker (vte handles internally)
- [x] OSC 8 hyperlinks render and are clickable (Ctrl+click)
- [ ] Kitty image protocol displays inline images — deferred (11.6)
- [ ] All modes persist across save/restore cursor — deferred

**Exit Criteria:** tmux, vim, and htop all have working mouse support. Cursor
styles change correctly. Applications can detect focus changes.
