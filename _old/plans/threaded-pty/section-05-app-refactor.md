---
section: "05"
title: App-Level Refactor
status: not-started
goal: Update every grid/terminal access in app/ to go through the lock
sections:
  - id: "05.1"
    title: Event loop refactor
    status: not-started
  - id: "05.2"
    title: Mouse selection refactor
    status: not-started
  - id: "05.3"
    title: Input, search, hover refactors
    status: not-started
  - id: "05.4"
    title: Tab management refactor
    status: not-started
---

# Section 05: App-Level Refactor

**Status:** Not Started
**Goal:** Every single access to grid/palette/mode/cursor_shape in `src/app/` goes through `tab.terminal.lock()`. No exceptions. No "we'll fix this one later." COMPLETE conversion.

---

## 05.1 Event Loop Refactor (`event_loop.rs`)

**Current `PtyOutput` handler** (lines 46-98): This entire block is replaced by the `Wakeup` handler. The new handler:

1. Reads title/bell/notification state through the lock
2. Updates tab bar dirty flag
3. Manages bell badge state
4. Queues redraw

**Current keyboard handler** (lines 250-358): Accesses `tab.mode`, `tab.grid().display_offset`, `tab.send_pty()`. All need lock or channel.

Specific changes:
- [ ] Line 256: `tab.mode.contains(TermMode::REPORT_EVENT_TYPES)` → `tab.terminal.lock().mode.contains(...)`
- [ ] Line 329: `tab.grid().display_offset != 0` → `tab.terminal.lock().active_grid().display_offset != 0`
- [ ] Line 331: `tab.scroll_to_bottom()` → acquires lock internally
- [ ] Line 332: `tab.clear_selection()` → stays on Tab (selection is main-thread only)
- [ ] Line 348: `tab.mode` → `tab.terminal.lock().mode`
- [ ] Line 354: `tab.send_pty(&bytes)` → `tab.send_input(&bytes)`
- [ ] Line 374: `tab.mode.contains(TermMode::FOCUS_IN_OUT)` → `tab.terminal.lock().mode.contains(...)`
- [ ] Line 380: `tab.send_pty(seq)` → `tab.send_input(seq)`

**Optimization**: For the keyboard handler, read `mode` once at the start of the handler scope:

```rust
let (mode, display_offset) = {
    let term = tab.terminal.lock();
    (term.mode, term.active_grid().display_offset)
};
// Use local copies — no lock held during key encoding/dispatch
```

- [ ] Replace `PtyOutput` match arm with `Wakeup` handler
- [ ] Refactor keyboard handler to read mode/offset through lock
- [ ] Refactor focus handler to read mode through lock
- [ ] Verify: no `tab.grid()` calls remain in `event_loop.rs`
- [ ] Verify: no `tab.mode` direct access remains (goes through lock)

---

## 05.2 Mouse Selection Refactor (`mouse_selection.rs`)

This file has the **densest** grid access — 15+ calls to `tab.grid()`. Every one needs the lock.

**Pattern**: Most selection operations read grid dimensions and row content. Group reads under a single lock hold:

```rust
// Example: handle_left_click
fn handle_left_click(&mut self, window_id: WindowId, ...) {
    let tab = self.tabs.get_mut(&tab_id)?;
    let term = tab.terminal.lock();
    let grid = term.active_grid();

    let cols = grid.cols;
    let lines = grid.lines;
    let abs_row = grid.viewport_to_absolute(line);
    let row = grid.absolute_row(abs_row);
    // ... all grid reads under single lock

    // Selection mutation doesn't need the lock (it's on Tab)
    drop(term);
    tab.set_selection(Selection { ... });
}
```

Exhaustive list of `tab.grid()` accesses in `mouse_selection.rs`:

- [ ] Line ~51: `tab.grid().absolute_row(abs_row)` — hyperlink check → lock
- [ ] Line ~63: `tab.grid()` — implicit URL check → lock
- [ ] Line ~89: `tab.grid()` — click bounds check (cols, lines) → lock
- [ ] Line ~109: `StableRowIndex::from_absolute(tab.grid(), abs_row)` → lock
- [ ] Line ~136: `selection::word_boundaries(tab.grid(), ...)` → lock
- [ ] Line ~155: `tab.grid()` — triple-click line selection → lock
- [ ] Line ~209-214: `tab.grid().cols/lines/viewport_to_absolute` — drag tracking → lock
- [ ] Line ~223: `selection::word_boundaries(tab.grid(), ...)` — word extend → lock
- [ ] Line ~242: `tab.grid()` — drag bounds → lock
- [ ] Line ~287-288: `tab.grid().lines/display_offset` — auto-scroll → lock

**Strategy**: Each handler function acquires the lock once at the top, reads everything it needs, drops the lock, then does selection mutations on Tab.

- [ ] Refactor every selection handler to lock-read-drop-mutate pattern
- [ ] Verify: no `tab.grid()` calls remain in `mouse_selection.rs`
- [ ] Verify: selection operations still correct under lock (no stale data between lock/unlock)

---

## 05.3 Input, Search, Hover Refactors

### `input_keyboard.rs`
- [ ] Line ~70: `tab.grid().lines` — page size for scroll → lock
- [ ] Line ~76: `tab.grid().lines` — page size for scroll → lock
- [ ] Line ~134: `tab.send_pty(text.as_bytes())` → `tab.send_input(text.as_bytes())`

### `input_mouse.rs`
- [ ] Line ~395: `tab.send_pty(seq)` → `tab.send_input(seq)`
- [ ] All `tab.mode` reads → through lock
- [ ] All `tab.grid()` reads for mouse coordinate mapping → through lock

### `mouse_report.rs`
- [ ] All `tab.send_pty(...)` → `tab.send_input(...)`
- [ ] All `tab.mode` reads → through lock

### `search_ui.rs`
- [ ] Line ~104: `tab.grid()` — search query update → lock
- [ ] Line ~109: `tab.grid_mut()` — scroll to match → lock (write)
- [ ] `tab.open_search()` / `tab.close_search()` — stays on Tab
- [ ] `tab.update_search_query()` — needs lock for grid access

### `hover_url.rs`
- [ ] Line ~43: `tab.grid()` — URL detection → lock
- [ ] Line ~68: `tab.grid()` — URL lookup → lock

### `cursor_hover.rs`
- [ ] All grid access → through lock

### `mouse_coord.rs`
- [ ] Grid dimension reads → through lock

- [ ] Complete audit: `grep -rn "tab\.grid\(\)\|tab\.grid_mut\(\)\|tab\.mode\|tab\.palette\|tab\.cursor_shape" src/app/` returns ZERO direct accesses

---

## 05.4 Tab Management Refactor (`tab_management.rs`)

- [ ] `new_tab_in_window()` — creates Tab, no grid access needed
- [ ] `close_tab()` — sends `TabMsg::Shutdown`, then `tab.shutdown()`
- [ ] Tab reordering — no grid access
- [ ] `apply_config_reload()` — needs lock to update palette, cursor shape

### `config_reload.rs`
- [ ] `tab.apply_color_config(...)` → acquires lock internally
- [ ] `tab.set_cursor_shape(...)` → acquires lock internally
- [ ] Any grid resize during config reload → acquires lock

### `window_management.rs`
- [ ] Window creation → no grid access
- [ ] `handle_resize()` → `tab.resize()` → acquires lock for grid resize + calls `pty_master.resize()`
- [ ] `handle_scale_factor_changed()` → may trigger resize → through lock

---

## 05.N Completion Checklist

- [ ] `grep -rn "tab\.grid\b" src/app/` → ZERO hits (all through lock)
- [ ] `grep -rn "tab\.grid_mut\b" src/app/` → ZERO hits
- [ ] `grep -rn "tab\.mode\b" src/app/` → ZERO hits (all through lock)
- [ ] `grep -rn "tab\.palette\b" src/app/` → ZERO hits
- [ ] `grep -rn "tab\.cursor_shape\b" src/app/` → ZERO hits
- [ ] `grep -rn "tab\.title\b" src/app/` → ZERO hits (title on TerminalState)
- [ ] `grep -rn "send_pty" src/` → ZERO hits
- [ ] Every lock acquisition is scoped minimally (no holding lock during I/O or rendering)
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` passes
- [ ] `cargo test` passes

**Exit Criteria:** Complete. Every single terminal state access in `src/app/` goes through `tab.terminal.lock()`. No direct field access on shared state. No exceptions.

---

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented.
