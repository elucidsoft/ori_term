---
section: 19
title: Event Routing & Render Scheduling
status: not-started
tier: 4
goal: Coordinate systems, 7-layer input dispatch, frame budgeting, cursor blink scheduling
sections:
  - id: "19.1"
    title: Coordinate Systems
    status: not-started
  - id: "19.2"
    title: Event Routing + Input Dispatch
    status: not-started
  - id: "19.3"
    title: Render Scheduling
    status: not-started
  - id: "19.4"
    title: Section Completion
    status: not-started
---

# Section 19: Event Routing & Render Scheduling

**Status:** 📋 Planned
**Goal:** Coordinate systems, 7-layer input dispatch, frame budgeting, cursor blink scheduling. This section covers the event routing pipeline and render scheduling that tie input, state, and GPU together.

**Crate:** `oriterm` (binary only — no core changes)

**Reference:** `_old/src/app/render_coord.rs`, `_old/src/app/mouse_coord.rs`, `_old/src/app/event_loop.rs`, `_old/src/app/input_mouse.rs`, `_old/src/app/input_keyboard.rs`

---

## 19.1 Coordinate Systems

Multiple coordinate systems coexist: pixel (window-relative), cell (grid position), and tab bar (button/tab positions). Correct mapping between them is critical for click handling, selection, and rendering.

**File:** `oriterm/src/app/render_coord.rs`, `oriterm/src/app/mouse_coord.rs`

**Reference:** `_old/src/app/render_coord.rs`, `_old/src/app/mouse_coord.rs`

- [ ] Window pixel layout (top to bottom):
  ```
  ┌─────────────────────────────────────────────────┐
  │ TAB BAR (TAB_BAR_HEIGHT × scale pixels)         │
  ├─────────────────────────────────────────────────┤
  │ GRID_PADDING_TOP × scale pixels                 │
  ├─────────────────────────────────────────────────┤
  │                                                 │
  │ Terminal Grid (cell_height × scale per row)      │
  │                                                 │
  ├─────────────────────────────────────────────────┤
  │ GRID_PADDING_BOTTOM × scale pixels              │
  └─────────────────────────────────────────────────┘
  ```
  With `GRID_PADDING_LEFT × scale` on the left side.
- [ ] `grid_top(&self) -> f32` — pixel Y where the terminal grid starts:
  - [ ] `TAB_BAR_HEIGHT * scale + GRID_PADDING_TOP * scale`
- [ ] `grid_dims_for_size(width: u32, height: u32) -> (usize, usize)` — compute grid columns and rows:
  - [ ] `cols = (width - GRID_PADDING_LEFT * scale) / (cell_width * scale)`
  - [ ] `rows = (height - TAB_BAR_HEIGHT * scale - GRID_PADDING_TOP * scale - GRID_PADDING_BOTTOM * scale) / (cell_height * scale)`
  - [ ] Floor division, minimum 1×1
- [ ] `pixel_to_cell(pos: PhysicalPosition<f64>) -> Option<(usize, usize)>` — convert pixel to grid cell:
  - [ ] Returns `None` if above grid (in tab bar), left of grid (in padding), or cell dimensions are 0
  - [ ] `col = (x - GRID_PADDING_LEFT * scale) / (cell_width * scale)`
  - [ ] `line = (y - grid_top()) / (cell_height * scale)`
  - [ ] Clamped to grid bounds
- [ ] `pixel_to_side(pos: PhysicalPosition<f64>) -> Side` — which half of a cell:
  - [ ] `Left` if cursor is in the left half, `Right` if in the right half
  - [ ] Used for selection boundary precision
- [ ] Tab bar coordinate mapping:
  - [ ] Tab X position: `TAB_LEFT_MARGIN * scale + tab_index * tab_width`
  - [ ] Close button X: `tab_x + tab_width - CLOSE_BUTTON_RIGHT_PAD * scale - CLOSE_BUTTON_WIDTH * scale`
  - [ ] New tab button X: `TAB_LEFT_MARGIN * scale + tab_count * tab_width`
  - [ ] Window controls X: `window_width - CONTROLS_ZONE_WIDTH * scale`
- [ ] Rebuild tab bar cache:
  - [ ] `rebuild_tab_bar_cache(&mut self, window_id: WindowId, active_tab_id: TabId)`
  - [ ] Extracts `Vec<(TabId, String)>` — tab ID + effective title for each tab
  - [ ] Extracts `Vec<bool>` — bell badges (true if tab has bell AND isn't active)
  - [ ] Stored as `cached_tab_info` and `cached_bell_badges` on App
  - [ ] Rebuilt when `tab_bar_dirty` is true, before rendering
- [ ] Windows-specific: Aero Snap hit rects:
  - [ ] `update_snap_hit_rects(window_id, bar_w, tab_count)` — passes interactive regions to OS via `set_client_rects()` so Windows knows which areas are clickable title bar

---

## 19.2 Event Routing + Input Dispatch

Input events follow a strict priority chain. Each layer can intercept and consume an event. Only the active tab receives PTY input. This decision tree was one of the most bug-prone areas of the old prototype.

**File:** `oriterm/src/app/event_loop.rs`, `oriterm/src/app/input_mouse.rs`, `oriterm/src/app/input_keyboard.rs`

**Reference:** `_old/src/app/event_loop.rs`, `_old/src/app/input_mouse.rs`, `_old/src/app/input_keyboard.rs`

- [ ] Keyboard input dispatch (in order, first match wins):
  1. [ ] **Key release**: skip entirely unless Kitty `REPORT_EVENT_TYPES` mode is active on the active tab (check via `Tab::mode()` — lock-free)
  2. [ ] **Settings window**: only Escape (close settings). All other keys consumed silently.
  3. [ ] **Context menu open**: only Escape (dismiss menu). All other keys consumed.
  4. [ ] **Search mode active** (`search_active == Some(window_id)`):
     - [ ] Escape → close search
     - [ ] Enter → next match (Shift+Enter → prev match)
     - [ ] Backspace → pop character from query
     - [ ] Printable character → append to query
     - [ ] All keys consumed — never reach PTY
  5. [ ] **Escape during active drag**: cancel drag, return tab to original position
  6. [ ] **Keybinding lookup** — check against configured bindings:
     - [ ] Build modifier mask from current `ModifiersState`
     - [ ] Look up `(logical_key, modifiers)` in binding table
     - [ ] If match found: `execute_action(action, window_id, event_loop)`
     - [ ] Actions include: `NewTab`, `CloseTab`, `NextTab`, `PrevTab`, `Copy`, `Paste`, `ScrollPageUp/Down`, `ZoomIn/Out`, `Search`, `DuplicateTab`, `MoveTabToNewWindow`
     - [ ] If action handled: consume key (return early)
  7. [ ] **PTY dispatch** (only reached if no binding matched):
     - [ ] Reset cursor blink timer (key press = show cursor)
     - [ ] If active tab has non-zero `display_offset`: scroll to bottom (back to live output)
     - [ ] Clear selection (typing clears selection)
     - [ ] Encode key via `key_encoding::encode_key()` using `Tab::mode()` (lock-free mode check)
     - [ ] Send encoded bytes to active tab's PTY: `tab.send_pty(&bytes)`
- [ ] Mouse input dispatch — left click (in order):
  1. [ ] **Context menu open**: hit-test menu → execute action or dismiss
  2. [ ] **Mouse reporting mode active** (any of TermMode::MOUSE_* flags, checked via `Tab::mode()` — lock-free):
     - [ ] Skip if Shift is held (Shift overrides mouse reporting for local selection)
     - [ ] Skip if settings window
     - [ ] Convert pixel to cell, encode button, send to PTY
     - [ ] Consume event — no local handling
  3. [ ] **Right-click**: context menu dispatch (different menus for tab bar vs grid area)
  4. [ ] **Settings window**: `handle_settings_mouse()` (row click = select scheme)
  5. [ ] **Resize border**: `drag_resize_window(direction)` for frameless window edge dragging
  6. [ ] **Tab bar hit** (via `TabBarHit`): dispatch per hit type (see tab bar hit testing section)
  7. [ ] **Grid area**: `handle_grid_press()`:
     - [ ] Detect click count (1 = char, 2 = word, 3 = line) via timing + position
     - [ ] `Alt+click`: block selection mode
     - [ ] `Ctrl+click`: open URL (check OSC 8 hyperlink first, then implicit URL detection)
     - [ ] `Shift+click`: extend existing selection
     - [ ] Otherwise: start new selection
- [ ] Mouse move dispatch:
  1. [ ] Context menu hover: update `hovered` field, redraw if changed
  2. [ ] URL hover detection (if Ctrl held): `detect_hover_url()` → update `hover_hyperlink` and underline range
  3. [ ] Mouse motion reporting (if mouse reporting mode + button held): send motion to PTY, consume
  4. [ ] Selection drag (if left button held and not consumed by motion reporting): `update_selection_drag()`
  5. [ ] Tab bar hover: update `hover_hit`, manage `tab_width_lock`
  6. [ ] Drag state machine updates: advance `DragPhase` (see drag state machine section)
- [ ] Mouse wheel:
  - [ ] If mouse reporting mode active: encode as scroll button codes (64=up, 65=down)
  - [ ] Else if alt screen + alternate scroll mode: send arrow key sequences
  - [ ] Else: normal scrollback scroll
- [ ] `TermEvent` handling (from PTY reader thread):
  - [ ] `Wakeup(tab_id)`:
    - [ ] Clear wakeup coalescing flag: `tab.clear_wakeup()`
    - [ ] Set `tab.set_grid_dirty(true)`
    - [ ] Lock terminal briefly to check: `title_dirty`, `bell_start`, drain notifications
    - [ ] If title changed: `tab_bar_dirty = true`
    - [ ] Bell badge: set on inactive tabs that rang bell, clear when tab becomes active
    - [ ] Invalidate URL cache
    - [ ] Add window to `pending_redraw` set
  - [ ] `PtyExited(tab_id)`: close the tab
  - [ ] `ConfigReload`: apply config changes (see config reload section)

---

## 19.3 Render Scheduling

Rendering is driven by `about_to_wait()`, not `RedrawRequested`. This avoids WM_PAINT starvation on Windows (where the OS can delay RedrawRequested indefinitely during resize). Frame budget is 8ms (~120 FPS cap).

**File:** `oriterm/src/app/event_loop.rs`

**Reference:** `_old/src/app/event_loop.rs`

- [ ] Dirty state aggregation — any of these trigger a render:
  - [ ] `pending_redraw: HashSet<WindowId>` — windows with pending redraws (from Wakeup events)
  - [ ] `tab_bar_dirty: bool` — tab bar needs rebuild (hover change, tab added/removed, title change)
  - [ ] `grid_dirty` — any active tab's grid has been updated by PTY reader
  - [ ] `has_bell_badge` — any tab has a bell badge (needs animated pulse)
  - [ ] `anim_active` — tab animation offsets are non-zero (decaying after drag)
  - [ ] `cursor_blink_dirty` — cursor blink state changed (visible ↔ hidden transition)
- [ ] Frame budget: `Duration::from_millis(8)` (~120 FPS):
  - [ ] Only render if `last_render_time.elapsed() >= frame_budget`
  - [ ] Prevents burning CPU when PTY output is continuous
- [ ] Render pass:
  - [ ] Clear `pending_redraw`
  - [ ] For each window: `render_window(window_id)`
  - [ ] Update `last_render_time`
- [ ] Control flow scheduling:
  - [ ] If needs render: `ControlFlow::WaitUntil(now + remaining_budget)` — wake up when budget allows next frame
  - [ ] If idle with cursor blink: compute next blink transition time, `ControlFlow::WaitUntil(next_toggle)`
    - [ ] `interval_ms = config.cursor_blink_interval_ms.max(1)`
    - [ ] `elapsed_ms = cursor_blink_reset.elapsed().as_millis()`
    - [ ] `next_toggle_ms = ((elapsed_ms / interval_ms) + 1) * interval_ms`
    - [ ] `sleep_ms = next_toggle_ms - elapsed_ms`
  - [ ] If fully idle (no blink, no animation): `ControlFlow::Wait` — sleep until next event
- [ ] `cursor_blink_visible(&self) -> bool`:
  - [ ] If blink disabled: always true
  - [ ] `(elapsed_ms / interval_ms) % 2 == 0` — even intervals = visible, odd = hidden
  - [ ] `cursor_blink_reset` is reset on every key press (typing always shows cursor)
- [ ] Performance stats (periodic logging):
  - [ ] Every 5 seconds: log renders/sec, PTY wakeups/sec, cursor moves/sec, about_to_wait/sec
  - [ ] Helps diagnose contention and rendering bottlenecks
- [ ] `render_window(window_id)`:
  - [ ] Lock active tab's terminal (via `Arc<FairMutex>`)
  - [ ] Build `FrameParams` struct with all immutable data needed for GPU:
    - [ ] Grid, palette, mode, cursor shape, selection, search — from terminal lock
    - [ ] Hover state, drag state, tab bar info — from App
    - [ ] Scale, cursor visibility, dirty flags
    - [ ] Context menu reference (for dropdown rendering)
    - [ ] Opacity, minimum contrast, alpha blending — from color config
  - [ ] **Release terminal lock** before GPU work
  - [ ] Pass `FrameParams` to `GpuRenderer`
  - [ ] Present frame
  - [ ] Measure render time, log if > 5ms

---

## 19.4 Section Completion

- [ ] All 19.1–19.3 items complete
- [ ] Coordinate systems: pixel → cell, tab bar layout, grid padding, side detection
- [ ] Event routing: 7-layer keyboard dispatch, 7-layer mouse dispatch, search/menu interception
- [ ] Render scheduling: about_to_wait coalescing, 8ms frame budget, cursor blink scheduling
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — clean build
- [ ] `cargo clippy -p oriterm -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings

**Exit Criteria:** Input events are routed through a strict priority chain with no ambiguity. Render scheduling coalesces dirty state and respects frame budget. Cursor blink is driven by ControlFlow timing, not polling. All coordinate system conversions are correct and DPI-aware.
