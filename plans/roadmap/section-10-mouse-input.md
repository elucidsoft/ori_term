---
section: 10
title: Mouse Input & Reporting
status: not-started
tier: 3
goal: Mouse reporting for terminal apps + mouse selection state machine
sections:
  - id: "10.1"
    title: Mouse Selection State Machine
    status: not-started
  - id: "10.2"
    title: Mouse Reporting
    status: not-started
  - id: "10.3"
    title: Section Completion
    status: not-started
---

# Section 10: Mouse Input & Reporting

**Status:** Not Started
**Goal:** Implement the mouse input layer: a state machine for tracking selection gestures, and mouse event reporting to the PTY for terminal applications that request it (vim, tmux, htop, etc.). Mouse reporting supports all three encoding formats (X10 normal, UTF-8, SGR) and all tracking modes.

**Crate:** `oriterm` (binary)
**Dependencies:** `winit` (mouse events), `oriterm_core` (TermMode, Grid)
**Reference:** `_old/src/app/mouse_report.rs`, `_old/src/app/mouse_selection.rs`, `_old/src/app/input_mouse.rs`

**Prerequisite:** Section 07 complete (Selection model and rendering). Section 03 complete (PTY send channel). Section 02 complete (TermMode flags for mouse mode detection).

---

## 10.1 Mouse Selection State Machine

Centralized state machine for tracking mouse gesture state. Coordinates between selection creation (Section 08) and mouse reporting (10.2), ensuring clean separation of concerns.

**File:** `oriterm/src/app/mouse_selection.rs`

**Reference:** `_old/src/app/mouse_selection.rs` — carries forward click counting, grid press handling, drag endpoint updates, auto-scroll.

- [ ] `SelectionState` struct
  - [ ] Fields:
    - `selection: Option<Selection>` — current active selection (None when no selection)
    - `click_count: u8` — 1, 2, or 3 (for multi-click detection)
    - `last_click_time: Option<Instant>` — timestamp of last left-click
    - `last_click_pos: Option<(usize, usize)>` — (col, row) of last click in cell coordinates
    - `last_click_window: Option<WindowId>` — window of last click (multi-click must be in same window)
    - `left_mouse_down: bool` — left button is currently held
    - `drag_threshold_met: bool` — true once mouse has moved 1/4 cell from touchdown
    - `touchdown_pos: Option<PhysicalPosition<f64>>` — raw pixel position of initial press
- [ ] `handle_press(&mut self, window_id: WindowId, pos: PhysicalPosition<f64>, grid: &Grid, modifiers: &Modifiers) -> SelectionAction`
  - [ ] Convert pixel position to cell coordinates
  - [ ] Detect click count: increment if same pos + same window + within 500ms, else reset to 1
  - [ ] Set `left_mouse_down = true`, `drag_threshold_met = false`, `touchdown_pos = Some(pos)`
  - [ ] Based on click count:
    - [ ] 1: Create `Selection::new_char()` (or Block if Alt held)
    - [ ] 2: Compute word boundaries, create `Selection::new_word()`
    - [ ] 3: Compute logical line boundaries, create `Selection::new_line()`
  - [ ] Shift+click: extend existing selection instead of creating new
  - [ ] Return `SelectionAction::Started` or `SelectionAction::Extended`
- [ ] `handle_drag(&mut self, pos: PhysicalPosition<f64>, grid: &Grid) -> SelectionAction`
  - [ ] If `!left_mouse_down`: return `SelectionAction::None`
  - [ ] Check drag threshold: if not yet met, compute distance from touchdown
    - [ ] Threshold = cell_width / 4
    - [ ] If below threshold: return `SelectionAction::None` (no selection yet)
    - [ ] If met: set `drag_threshold_met = true`
  - [ ] Convert pixel position to cell coordinates
  - [ ] Update selection endpoint based on mode:
    - [ ] Char: set endpoint directly
    - [ ] Word: compute word boundaries at drag position, use nearest boundary relative to anchor
    - [ ] Line: compute logical line boundaries at drag position, use appropriate end
    - [ ] Block: set endpoint directly (rectangular bounds computed from anchor + endpoint)
  - [ ] Handle auto-scroll: if mouse above/below grid, scroll viewport and extend selection
  - [ ] Return `SelectionAction::Extended`
- [ ] `handle_release(&mut self) -> SelectionAction`
  - [ ] Set `left_mouse_down = false`
  - [ ] If CopyOnSelect enabled and selection is non-empty: trigger copy
  - [ ] Return `SelectionAction::Finalized`
- [ ] `clear(&mut self)`
  - [ ] Set `selection = None`
  - [ ] Reset all tracking state
- [ ] `current_selection(&self) -> Option<&Selection>`
  - [ ] Returns reference to current selection (for rendering, text extraction)
- [ ] `SelectionAction` enum
  - [ ] `None` — no action taken (below drag threshold, etc.)
  - [ ] `Started` — new selection created
  - [ ] `Extended` — existing selection endpoint updated
  - [ ] `Cleared` — selection was cleared
  - [ ] `Finalized` — selection complete (mouse released)
- [ ] **Tests** (`oriterm/src/app/mouse_selection.rs` `#[cfg(test)]`):
  - [ ] Press creates Char selection at correct position
  - [ ] Double-press creates Word selection with boundaries
  - [ ] Triple-press creates Line selection
  - [ ] Drag below threshold returns None
  - [ ] Drag above threshold returns Extended
  - [ ] Release sets left_mouse_down to false
  - [ ] Clear resets all state
  - [ ] Rapid clicks cycle 1 -> 2 -> 3 -> 1
  - [ ] Shift+press extends existing selection

---

## 10.2 Mouse Reporting

Encode mouse events and send to PTY when terminal applications request mouse tracking. Supports all three encoding formats and all tracking modes.

**File:** `oriterm/src/app/mouse_report.rs`

**Reference:** `_old/src/app/mouse_report.rs` — carries forward SGR, UTF-8, and normal encoding with modifier bits.

- [ ] **Mouse tracking modes** (checked via TermMode flags):
  - [ ] `MOUSE_REPORT_CLICK` (DECSET 1000) — report button press/release only
  - [ ] `MOUSE_REPORT_DRAG` (DECSET 1002) — report press/release + drag motion (button held)
  - [ ] `MOUSE_REPORT_MOTION` (DECSET 1003) — report all motion (even without button)
  - [ ] No flag set: mouse events are local-only (selection, no PTY reporting)
- [ ] **Mouse encoding modes** (checked via TermMode flags):
  - [ ] `SGR_MOUSE` (DECSET 1006) — preferred: `ESC[<code;col;row M/m` (no coordinate limit)
  - [ ] `UTF8_MOUSE` (DECSET 1005) — coordinates UTF-8 encoded (larger range than normal)
  - [ ] Default (X10 normal) — `ESC[M cb cx cy` (coordinates limited to 223)
- [ ] `send_mouse_report(&mut self, tab_id: TabId, button: u8, col: usize, line: usize, pressed: bool)`
  - [ ] Main entry point: encodes and sends mouse event to PTY
  - [ ] **Button encoding**:
    - [ ] 0 = left, 1 = middle, 2 = right, 3 = release (normal/UTF-8 only)
    - [ ] 64 = scroll up, 65 = scroll down
    - [ ] Add 32 for motion events (drag/move reporting)
  - [ ] **Modifier bits** (added to button code):
    - [ ] +4 if Shift held
    - [ ] +8 if Alt held
    - [ ] +16 if Ctrl held
  - [ ] **SGR encoding** (`ESC[<code;col+1;row+1 M` for press, `m` for release):
    - [ ] Format: `\x1b[<{code};{col+1};{row+1}{suffix}` where suffix is `M` (press) or `m` (release)
    - [ ] Use stack-allocated buffer (`[u8; 32]`) for zero-allocation formatting
    - [ ] No coordinate limit (arbitrary terminal sizes supported)
  - [ ] **UTF-8 encoding** (`ESC[M` + UTF-8 encoded values):
    - [ ] Code byte: `button_code + 32`, UTF-8 encoded
    - [ ] Column: `col + 1 + 32`, UTF-8 encoded
    - [ ] Row: `line + 1 + 32`, UTF-8 encoded
    - [ ] Skip report if any coordinate exceeds Unicode scalar range (U+10FFFF)
    - [ ] Max buffer: 15 bytes (`ESC[M` + up to 3x4 UTF-8 bytes)
  - [ ] **Normal (X10) encoding** (`ESC[M Cb Cx Cy`):
    - [ ] `Cb = 32 + button_code`
    - [ ] `Cx = min(col + 1, 223) + 32`
    - [ ] `Cy = min(line + 1, 223) + 32`
    - [ ] Coordinates clamped to 223 (single-byte limit)
- [ ] **Mouse mode priority over selection**:
  - [ ] When any MOUSE_REPORT mode is active: mouse events go to PTY, NOT to selection
  - [ ] **Shift bypasses mouse reporting**: Shift+click always does local selection even when app has mouse mode
  - [ ] This is the standard terminal convention (Shift overrides app mouse)
- [ ] **Motion deduplication**:
  - [ ] Track last reported cell position (`last_mouse_report_pos: Option<(usize, usize)>`)
  - [ ] Only report motion events when cell position actually changes
  - [ ] Prevents flooding PTY with redundant reports during smooth mouse movement
- [ ] **Alternate scroll mode** (DECSET 1007):
  - [ ] When in alternate screen buffer: convert scroll wheel events to arrow key sequences
  - [ ] Scroll up -> N x `ESC[A` (Up arrow), scroll down -> N x `ESC[B` (Down arrow)
  - [ ] Allows scrolling in `less`, `man`, etc. without mouse reporting
- [ ] **Mouse event dispatch** (integration with event loop):
  - [ ] `handle_mouse_input(&mut self, event: &MouseEvent, window_id: WindowId)`
  - [ ] Check if mouse position is in grid area (not tab bar, not resize handle)
  - [ ] Check if any MOUSE_REPORT mode is active on the focused tab
  - [ ] If mouse mode active and Shift not held: encode and send to PTY
  - [ ] If mouse mode not active or Shift held: delegate to selection state machine (10.1)
  - [ ] Scroll wheel: always check alternate scroll mode first
- [ ] **Tests** (`oriterm/src/app/mouse_report.rs` `#[cfg(test)]`):
  - [ ] SGR encoding: left click at (5, 10) produces `\x1b[<0;6;11M`
  - [ ] SGR encoding: release produces `\x1b[<0;6;11m`
  - [ ] SGR encoding: right click produces `\x1b[<2;6;11M`
  - [ ] SGR encoding: Shift+left click produces `\x1b[<4;6;11M`
  - [ ] SGR encoding: scroll up produces `\x1b[<64;6;11M`
  - [ ] Normal encoding: left click at (5, 10) produces `\x1b[M\x20\x26\x2b`
  - [ ] Normal encoding: coordinates clamped at 223
  - [ ] UTF-8 encoding: coordinates encoded as UTF-8
  - [ ] UTF-8 encoding: large coordinates (> 223) produce multi-byte UTF-8
  - [ ] Motion dedup: same cell position not reported twice
  - [ ] Shift held: mouse event goes to selection, not PTY
  - [ ] Alternate scroll: scroll wheel in alt screen sends arrow keys

---

## 10.3 Section Completion

- [ ] All 10.1-10.2 items complete
- [ ] `cargo test -p oriterm --target x86_64-pc-windows-gnu` — mouse tests pass
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] Mouse selection state machine handles all gesture types (single/double/triple click, drag, release)
- [ ] Drag threshold prevents accidental selection
- [ ] Mouse reporting sends correct sequences for all three encoding formats (SGR, UTF-8, X10)
- [ ] All tracking modes work: click-only, drag, all-motion
- [ ] Modifier bits correct in mouse reports (Shift, Alt, Ctrl)
- [ ] Scroll wheel events reported correctly
- [ ] Shift bypasses mouse reporting for local selection
- [ ] Motion events deduplicated (only report on cell change)
- [ ] Alternate scroll mode converts scroll to arrow keys in alt screen
- [ ] Mouse mode and selection mode coexist correctly (mutual exclusion with Shift override)
- [ ] **Integration tests**:
  - [ ] vim with `set mouse=a`: mouse clicks position cursor
  - [ ] tmux: mouse clicks select panes
  - [ ] htop: scroll wheel scrolls process list
  - [ ] Shift+click in vim creates local selection (not sent to vim)

**Exit Criteria:** Mouse reporting works correctly for all terminal applications that use it. vim, tmux, htop, and other mouse-aware apps receive correct mouse events. Selection and reporting coexist cleanly with Shift-override convention.
