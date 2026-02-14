---
section: 22
title: Terminal Modes
status: not-started
tier: 5
goal: Comprehensive DECSET/DECRST mode support, mode interactions, image protocol
sections:
  - id: "22.1"
    title: Mouse Reporting Modes
    status: not-started
  - id: "22.2"
    title: Cursor Styles
    status: not-started
  - id: "22.3"
    title: Focus Events
    status: not-started
  - id: "22.4"
    title: Synchronized Output
    status: not-started
  - id: "22.5"
    title: Hyperlinks
    status: not-started
  - id: "22.6"
    title: Comprehensive Mode Table
    status: not-started
  - id: "22.7"
    title: Image Protocol
    status: not-started
  - id: "22.8"
    title: Section Completion
    status: not-started
---

# Section 22: Terminal Modes

**Status:** Not Started
**Goal:** Complete, correct DECSET/DECRST mode support with proper mode interactions, mouse reporting, cursor styles, hyperlinks, and image protocol. This section is the authoritative reference for every terminal mode ori_term must handle.

**Crate:** `oriterm` (binary) and `oriterm_core` (mode flags, state)
**Dependencies:** `vte` (parser + handler), `unicode-width`, `regex` (URL detection)

**Reference:**
- Ghostty's comprehensive mode handling (`modes.zig`) and feature support
- Alacritty's mouse reporting and cursor style support
- WezTerm's image protocol and hyperlink support
- xterm ctlseqs documentation

---

## 22.1 Mouse Reporting Modes

Report mouse events to applications (vim, tmux, htop, etc.). The terminal must support multiple reporting modes and encoding formats, with correct priority and interaction semantics.

**Files:** `oriterm/src/app/input_mouse.rs`, `oriterm/src/app/mouse_report.rs`, `oriterm_core/src/term_mode.rs`

**Reference:** `_old/src/app/input_mouse.rs`, `_old/src/app/mouse_report.rs`, `_old/src/term_mode.rs`

- [ ] Mouse reporting modes (DECSET):
  - [ ] 9: X10 mouse reporting (button press only, legacy)
  - [ ] 1000: Normal tracking (press + release)
  - [ ] 1002: Button-event tracking (press + release + drag with button held)
  - [ ] 1003: Any-event tracking (all motion, even without button held)
  - [ ] Modes are mutually exclusive — enabling one disables the others
- [ ] Mouse encoding formats:
  - [ ] Default: `ESC[M Cb Cx Cy` (X10-compatible, limited to 223 columns/rows)
  - [ ] UTF-8 (DECSET 1005): UTF-8 encoded coordinates (extends range)
  - [ ] SGR (DECSET 1006): `ESC[<Cb;Cx;Cy M/m` — preferred, no coordinate limit, distinguishes press (`M`) from release (`m`)
  - [ ] URXVT (DECSET 1015): `ESC[Cb;Cx;Cy M` — decimal encoding, no release distinction
  - [ ] Encoding modes are mutually exclusive — enabling one disables the others
- [ ] Button encoding: left=0, middle=1, right=2, wheel up=64, wheel down=65
- [ ] Modifier encoding: Shift adds 4, Alt adds 8, Ctrl adds 16 to button byte
- [ ] Shift+click bypasses mouse reporting (allows selection even when app captures mouse)
  - [ ] When mouse reporting active, normal clicks go to the application
  - [ ] When Shift held, clicks go to selection logic instead
- [ ] Motion dedup: only report motion when the cell position changes
  - [ ] Track `last_mouse_cell: Option<(usize, usize)>` to avoid redundant reports
- [ ] Alternate scroll mode (DECSET 1007):
  - [ ] When in alternate screen buffer and this mode is set, scroll events are converted to arrow key sequences (Up/Down) instead of being reported as mouse scroll
  - [ ] Enables scrolling in programs like `less`, `man` that don't handle mouse scroll
- [ ] `TermMode::ANY_MOUSE` helper constant — union of all mouse reporting mode flags
- [ ] **Tests** (`oriterm_core/src/term_mode.rs` `#[cfg(test)]`):
  - [ ] Enabling mode 1003 disables 1000 and 1002
  - [ ] SGR encoding produces correct escape sequence for button press and release
  - [ ] Default encoding clamps coordinates to 223
  - [ ] Shift+click flag bypasses mouse reporting
  - [ ] Motion dedup suppresses duplicate cell positions
  - [ ] Alternate scroll converts wheel to arrow keys in alt screen

---

## 22.2 Cursor Styles

Support different cursor shapes, blinking, and cursor color.

**Files:** `oriterm_core/src/grid/cursor.rs`, `oriterm/src/gpu/renderer.rs`, `oriterm_core/src/term_handler/cursor.rs`

**Reference:** `_old/src/grid/cursor.rs`, `_old/src/gpu/renderer.rs`, `_old/src/term_handler/cursor.rs`

- [ ] Cursor shapes via DECSCUSR (CSI Ps SP q):
  - [ ] 0: default (reset to config default, typically blinking block)
  - [ ] 1: blinking block
  - [ ] 2: steady block
  - [ ] 3: blinking underline
  - [ ] 4: steady underline
  - [ ] 5: blinking bar (I-beam)
  - [ ] 6: steady bar
- [ ] Store cursor shape in terminal state (grid cursor or tab state)
- [ ] Render cursor according to shape:
  - [ ] Block: filled rectangle over cell, invert text color beneath
  - [ ] Underline: 2px horizontal bar at bottom of cell
  - [ ] Bar: 2px vertical bar at left edge of cell
  - [ ] Hollow block: unfilled rectangle outline (used when window is unfocused)
- [ ] Blinking: toggle cursor visibility on a timer
  - [ ] Default blink interval: 530ms (matches xterm)
  - [ ] Reset blink timer on any cursor movement (cursor always visible immediately after move)
  - [ ] Configurable blink rate via config
  - [ ] Only blink when the cursor shape is a blinking variant (1, 3, 5)
  - [ ] When blinking is disabled (steady variants 2, 4, 6), cursor is always visible
- [ ] OSC 12: set cursor color
  - [ ] `OSC 12 ; <color-spec> ST` — set cursor color to specified color
  - [ ] Color spec: named color, `#RRGGBB`, `rgb:RR/GG/BB`
  - [ ] Reset: `OSC 112 ST` — reset to default cursor color
- [ ] Save/restore cursor style with DECSC/DECRC
  - [ ] DECSC saves: position, template cell (fg/bg/flags), cursor shape, origin mode, charset
  - [ ] DECRC restores all saved state; if nothing saved, resets to defaults
- [ ] **Tests** (`oriterm_core/src/grid/cursor.rs` `#[cfg(test)]`):
  - [ ] DECSCUSR 0 resets to default shape
  - [ ] DECSCUSR 1-6 sets correct shape and blink flag
  - [ ] Save/restore round-trips cursor position, shape, and template
  - [ ] Restore with no prior save resets to defaults

---

## 22.3 Focus Events

Report window focus changes to applications that request them.

**Files:** `oriterm/src/app/event_loop.rs`, `oriterm_core/src/term_mode.rs`

**Reference:** `_old/src/app/event_loop.rs`, `_old/src/term_mode.rs`

- [ ] DECSET 1004: enable focus event reporting
- [ ] When window gains focus: send `ESC[I` to PTY
- [ ] When window loses focus: send `ESC[O` to PTY
- [ ] Handle winit `WindowEvent::Focused(bool)` in event loop
- [ ] Only send focus events when the mode flag is set
- [ ] Settings/overlay windows excluded from focus reporting (only terminal window)
- [ ] Visual: dim terminal slightly when unfocused (optional enhancement)
  - [ ] Configurable opacity reduction (e.g., 0.8x alpha) when window loses focus
- [ ] **Tests** (`oriterm_core/src/term_mode.rs` `#[cfg(test)]`):
  - [ ] Focus event mode flag toggles correctly with DECSET/DECRST 1004
  - [ ] Focus in produces `\x1b[I`, focus out produces `\x1b[O`
  - [ ] No output when mode is not set

---

## 22.4 Synchronized Output

Prevent partial frame rendering during rapid output.

**Files:** `oriterm/src/tab/mod.rs` (or equivalent VTE processing path)

**Reference:** `_old/src/tab/mod.rs`, `_old/src/term_handler/mod.rs`

- [ ] Mode 2026 (SyncUpdate): handled internally by vte 0.15 `Processor`
  - [ ] vte buffers handler calls between BSU (Begin Synchronized Update) and ESU (End Synchronized Update), dispatching as one batch
  - [ ] Since VTE processing calls `processor.advance()` in a loop then requests one redraw, synchronized output works correctly without additional application logic
- [ ] Explicit documentation comments in `set_private_mode`/`unset_private_mode` noting that Mode 2026 is handled by vte internally
- [ ] **Tests:**
  - [ ] Verify that vte processes BSU/ESU sequences without error
  - [ ] Verify that a redraw is only requested after the ESU, not during buffered output

---

## 22.5 Hyperlinks

OSC 8 hyperlink support for clickable URLs, plus implicit URL detection in terminal output.

**Files:** `oriterm_core/src/cell.rs` (CellExtra, Hyperlink), `oriterm/src/url_detect.rs`, `oriterm/src/app/input_mouse.rs`

**Reference:** `_old/src/cell.rs`, `_old/src/url_detect.rs`, `_old/src/app/cursor_hover.rs`

- [ ] Parse OSC 8 sequences (handled by vte):
  - [ ] Start hyperlink: `OSC 8 ; id=foo ; https://example.com ST`
  - [ ] End hyperlink: `OSC 8 ; ; ST` (empty URI closes the link)
  - [ ] `id` parameter: optional, groups cells into the same hyperlink (e.g., across line wraps)
- [ ] Store hyperlink in `CellExtra` for cells within the hyperlink span
  - [ ] `Hyperlink { id: Option<String>, uri: String }`
  - [ ] `CellExtra::hyperlink: Option<Hyperlink>` — only allocated when needed
- [ ] Rendering:
  - [ ] Hyperlinked text: dotted underline (visual cue)
  - [ ] Hovered hyperlink (Ctrl held + cursor over link): solid underline on full URL span
- [ ] Mouse hover detection:
  - [ ] When Ctrl held and cursor is over a hyperlinked cell, change cursor to `CursorIcon::Pointer`
  - [ ] Detect the full span of the hyperlink (all contiguous cells with the same hyperlink ID)
- [ ] Ctrl+click: open URL in default browser
  - [ ] URL scheme validation: only allow `http`, `https`, `ftp`, `file` schemes
  - [ ] Platform-specific open: `ShellExecuteW` on Windows, `xdg-open` on Linux, `open` on macOS
- [ ] Implicit URL detection (plain-text URLs without OSC 8):
  - [ ] Regex-based URL detection across soft-wrapped logical lines
  - [ ] Lazy detection: only run regex on Ctrl+hover or Ctrl+click, not every frame
  - [ ] Per-logical-line caching of detected URLs to avoid re-running regex
  - [ ] Ctrl+hover shows pointer cursor + solid underline on the full detected URL span
  - [ ] Ctrl+click opens detected URL in default browser
  - [ ] Skip cells that already have an OSC 8 hyperlink (explicit links take priority)
  - [ ] Handle Wikipedia-style parenthesized URLs, strip trailing punctuation (`.`, `,`, `)` when unbalanced)
- [ ] **Tests** (`oriterm_core/src/cell.rs` `#[cfg(test)]`, `oriterm/src/url_detect.rs` `#[cfg(test)]`):
  - [ ] OSC 8 start/end correctly sets and clears hyperlink on cells
  - [ ] Hyperlink ID groups cells across line wraps
  - [ ] URL scheme validation rejects `javascript:`, allows `https:`
  - [ ] Implicit URL regex matches `https://example.com`, `http://foo.bar/baz?q=1`
  - [ ] Trailing punctuation stripped: `https://example.com.` detects `https://example.com`
  - [ ] Balanced parentheses preserved: `https://en.wikipedia.org/wiki/Foo_(bar)` detected correctly

---

## 22.6 Comprehensive Mode Table

Complete reference of every DECSET/DECRST private mode and standard mode that ori_term must handle. This sub-section is the authoritative table — all mode-related code should be traceable back to an entry here.

**Files:** `oriterm_core/src/term_mode.rs`

**Reference:** `_old/src/term_mode.rs`, xterm ctlseqs, Ghostty `modes.zig`

### Private Modes (DECSET/DECRST — `CSI ? Pm h` / `CSI ? Pm l`)

| Mode | Name | Description |
|------|------|-------------|
| 1 | DECCKM | Application cursor keys. When set, cursor keys send `ESC O A/B/C/D` instead of `ESC [ A/B/C/D`. |
| 6 | DECOM | Origin mode. When set, cursor addressing is relative to the scroll region. CUP/HVP are offset by the scroll region top. |
| 7 | DECAWM | Auto-wrap mode. When set, characters written past the right margin wrap to the next line. When reset, writing at the right margin overwrites the last cell. |
| 9 | X10 Mouse | X10 mouse reporting. Reports button press only (no release, no motion). Legacy — prefer mode 1000+. |
| 25 | DECTCEM | Text cursor enable. When set, cursor is visible. When reset, cursor is hidden. |
| 45 | Reverse Wraparound | When set, backspace at column 0 wraps to the end of the previous line (if that line was auto-wrapped). |
| 47 | Alt Screen (47) | Switch to alternate screen buffer. Does NOT save/restore cursor. Legacy — prefer 1049. |
| 1000 | Normal Mouse | Normal mouse tracking. Reports button press and release events. |
| 1002 | Button Mouse | Button-event mouse tracking. Like 1000, but also reports motion while a button is held. |
| 1003 | Any Mouse | Any-event mouse tracking. Reports all motion, even without a button held. |
| 1004 | Focus Events | When set, terminal sends `ESC[I` on focus in and `ESC[O` on focus out. |
| 1005 | UTF-8 Mouse | UTF-8 mouse encoding. Extends coordinate range using UTF-8 encoding. |
| 1006 | SGR Mouse | SGR mouse encoding. `ESC[<Cb;Cx;Cy M/m` format. Preferred — no coordinate limits, distinguishes press from release. |
| 1007 | Alt Scroll | Alternate scroll mode. In alt screen, scroll events converted to Up/Down arrow key sequences. |
| 1015 | URXVT Mouse | URXVT mouse encoding. Decimal coordinates, no press/release distinction. |
| 1047 | Alt Screen (1047) | Switch to alternate screen buffer. Clears alt screen on enter. Does NOT save/restore cursor. |
| 1048 | Save Cursor | Save cursor position (same as DECSC). Paired with DECRC on reset. |
| 1049 | Alt Screen (1049) | Switch to alternate screen buffer. Saves cursor on enter (DECSC), restores on leave (DECRC), clears alt screen. This is the standard mode used by full-screen applications. |
| 2004 | Bracketed Paste | When set, pasted text is wrapped in `ESC[200~` ... `ESC[201~` so applications can distinguish paste from typed input. |
| 2026 | Sync Output | Synchronized output. Handled internally by vte — BSU/ESU bracket a batch of updates. |

### Standard Modes (`CSI Pm h` / `CSI Pm l`)

| Mode | Name | Description |
|------|------|-------------|
| 4 | IRM | Insert/Replace mode. When set, characters are inserted (shifting existing chars right). When reset, characters overwrite. |
| 20 | LNM | Linefeed/New Line mode. When set, LF also performs CR. When reset, LF only moves down. |

### Application Keypad (DECKPAM/DECKPNM)

- [ ] `ESC =` (DECKPAM): Application keypad mode. Numpad keys send application sequences.
- [ ] `ESC >` (DECKPNM): Normal keypad mode. Numpad keys send their face values.
- [ ] Store as flag in `TermMode`

### Mode Interactions

- [ ] Mouse modes (9, 1000, 1002, 1003) are mutually exclusive — enabling one implicitly disables the others
- [ ] Mouse encoding modes (1005, 1006, 1015) are mutually exclusive
- [ ] Alt screen swap (1049) saves/restores the full keyboard mode stack (application cursor, bracketed paste, mouse modes)
  - [ ] Applications entering alt screen get a clean mode slate
  - [ ] Exiting alt screen restores the modes that were active before entering
- [ ] DECTCEM (25) is independent of alt screen — cursor visibility persists across screen switches
- [ ] Origin mode (6) interacts with scroll regions — cursor clamped to scroll region when set

### Save/Restore Modes (XTSAVE/XTRESTORE)

- [ ] `CSI ? Pm s` (XTSAVE): save current state of mode `Pm`
- [ ] `CSI ? Pm r` (XTRESTORE): restore previously saved state of mode `Pm`
- [ ] Store saved mode state in a `HashMap<u16, bool>` or similar
- [ ] Used by applications that want to temporarily change a mode and then restore it

### Implementation Checklist

- [ ] Define all modes as constants/flags in `TermMode`
- [ ] `set_private_mode` handles all modes in the table above
- [ ] `unset_private_mode` handles all modes in the table above
- [ ] Mode interactions enforced (mutual exclusion, alt screen save/restore)
- [ ] XTSAVE/XTRESTORE implemented for all applicable modes
- [ ] Unknown modes logged at debug level and ignored (no panic)
- [ ] **Tests** (`oriterm_core/src/term_mode.rs` `#[cfg(test)]`):
  - [ ] Setting each mode flag and verifying it is set
  - [ ] Mutual exclusion: setting mode 1003 clears 1000 and 1002
  - [ ] Alt screen enter/exit saves and restores mode state
  - [ ] XTSAVE/XTRESTORE round-trip for each mode
  - [ ] Unknown mode number does not panic

---

## 22.7 Image Protocol

Display images inline in the terminal. This is a deferred feature — document the design but do not implement until higher-priority sections are complete.

**Files:** (to be determined — likely `oriterm_core/src/image.rs`, `oriterm/src/gpu/render_image.rs`)

**Reference:** Kitty image protocol spec, Ghostty image support, WezTerm image protocols

- [ ] Kitty image protocol (preferred):
  - [ ] Image transmission via APC sequences (`ESC_P ... ESC\`)
  - [ ] Transmission methods: direct (base64 payload), file path reference, shared memory
  - [ ] Image placement: position (cell coordinates), size (cells or pixels), z-index (above/below text)
  - [ ] Image operations: display, delete by ID, delete by position, animate (frame sequences)
  - [ ] Image IDs and placement IDs for managing multiple images
  - [ ] Chunked transmission for large images (multiple APC sequences with `m=1` continuation flag)
- [ ] Sixel graphics (legacy):
  - [ ] Parse sixel data from DCS sequences (`DCS P1;P2;P3 q <sixel-data> ST`)
  - [ ] Decode sixel pixel rows (6 pixels per character, palette-based)
  - [ ] Render sixel images as bitmaps placed in the terminal grid
  - [ ] Scrolling: sixel images scroll with the text
- [ ] Image storage:
  - [ ] Cache decoded images in memory (GPU texture or CPU bitmap)
  - [ ] Evict images when scrolled out of view or explicitly deleted
  - [ ] Configurable memory limit for image cache (default: 256 MB)
  - [ ] Reference counting: multiple placements can reference the same image data
- [ ] Rendering:
  - [ ] Composite images over cell backgrounds in the GPU render pass
  - [ ] Separate texture bind group for image atlas (distinct from glyph atlas)
  - [ ] Z-ordering: images can render above or below text depending on placement flags
  - [ ] Clip images to cell boundaries

**Status:** Deferred. This is a complex feature that requires significant GPU pipeline changes. Document the design here for future implementation.

---

## 22.8 Section Completion

- [ ] All 22.1-22.7 items complete (excluding 22.7 Image Protocol if still deferred)
- [ ] Mouse reporting works in vim, tmux, htop with all encoding formats
- [ ] SGR mouse encoding supported (no coordinate limits)
- [ ] Shift+click bypasses mouse reporting for selection
- [ ] Cursor shape changes work (block, underline, bar) with blinking
- [ ] Focus events sent when window gains/loses focus
- [ ] Synchronized output prevents flicker (vte handles internally)
- [ ] OSC 8 hyperlinks render and are clickable (Ctrl+click)
- [ ] Implicit URL detection works on plain-text URLs
- [ ] All modes in the comprehensive mode table are implemented
- [ ] Mode interactions (mutual exclusion, alt screen save/restore) are correct
- [ ] XTSAVE/XTRESTORE work for applicable modes
- [ ] `cargo test` — all mode tests pass
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings

**Exit Criteria:** Every mode in the comprehensive mode table is implemented and tested. tmux, vim, htop, and other TUI applications have fully working mode support including mouse, cursor styles, and focus events.
