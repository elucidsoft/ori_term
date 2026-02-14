---
section: 12
title: Resize & Reflow
status: not-started
tier: 3
goal: Dynamic grid resize with PTY notification and text reflow on column change
sections:
  - id: "12.1"
    title: Window-to-Grid Resize
    status: not-started
  - id: "12.2"
    title: PTY Resize Notification
    status: not-started
  - id: "12.3"
    title: Grid Row Resize
    status: not-started
  - id: "12.4"
    title: Text Reflow
    status: not-started
  - id: "12.5"
    title: Alternate Screen Resize
    status: not-started
  - id: "12.6"
    title: Section Completion
    status: not-started
---

# Section 12: Resize & Reflow

**Status:** Not Started
**Goal:** When the window resizes, the terminal grid resizes to match and the PTY is notified of the new dimensions. Text reflows intelligently on column changes, preserving wrapped line continuity and cursor position.

**Crate:** `oriterm_core` (Grid::resize, reflow), `oriterm` (window resize handler, PTY notification)
**Reference:** `_old/src/grid/reflow.rs`, `_old/plans/terminal-core/section-04-resize.md`, Alacritty `grid/resize.rs`, Ghostty's cell-by-cell reflow

**Prerequisite:** Section 01 (Grid with rows, scrollback, dirty tracking), Section 03 (PTY handle for resize notification)

**Inspired by:**
- Alacritty's grid reflow (`grid/resize.rs`) with wide-char handling
- Ghostty's unified cell-by-cell rewriting approach (used in old prototype)

---

## 12.1 Window-to-Grid Resize

Calculate new grid dimensions from window pixel size and cell metrics. Dispatch resize to grid and PTY.

**File:** `oriterm/src/app/event_loop.rs` (resize handler)

- [ ] On `WindowEvent::Resized(new_physical_size)`:
  - [ ] Calculate available area: subtract tab bar height, padding (top, bottom, left, right)
  - [ ] New cols = `available_width / cell_width` (integer division)
  - [ ] New rows = `available_height / cell_height` (integer division)
  - [ ] Zero dimension guard: `if new_cols == 0 || new_rows == 0 { return; }`
  - [ ] Only resize if dimensions actually changed (compare against stored grid dims)
- [ ] Store current grid dimensions for size comparison
- [ ] Call `grid.resize(new_cols, new_rows, reflow=true)` for primary grid
- [ ] Call `grid.resize(new_cols, new_rows, reflow=false)` for alternate grid (no reflow in alt screen)
- [ ] Notify PTY of new dimensions (see 12.2)
- [ ] Reconfigure GPU surface if pixel dimensions changed
- [ ] Mark all rows dirty for full redraw after resize

**Ref:** Alacritty `event.rs` zero-dimension guard, resize pipeline

---

## 12.2 PTY Resize Notification

Tell the child process about the new terminal size so it can redraw.

**File:** `oriterm/src/tab.rs` (Tab::resize)

- [ ] After grid resize, notify PTY of new dimensions:
  - [ ] Windows (ConPTY): `portable-pty` `MasterPty::resize(PtySize)` or `resize_pty()` on handle
  - [ ] Unix: `ioctl(fd, TIOCSWINSZ, &winsize)` (handled internally by portable-pty)
- [ ] `PtySize { rows: u16, cols: u16, pixel_width: u16, pixel_height: u16 }`
  - [ ] Include both character dimensions (cols, rows) and pixel dimensions
- [ ] Never send 0x0 resize — crashes ConPTY on Windows
  - [ ] Guard: `if rows == 0 || cols == 0 { return; }`
- [ ] Store PTY master handle in Tab for resize access
- [ ] Resize both primary and alternate grids, but only send one PTY notification (the shell sees unified dimensions)

**Ref:** Alacritty `tty/mod.rs` OnResize, WezTerm PtySize

---

## 12.3 Grid Row Resize

Handle vertical dimension changes: adding/removing rows with scrollback interaction.

**File:** `oriterm_core/src/grid/reflow.rs` (or `grid/mod.rs`)

**Reference:** `_old/src/grid/reflow.rs` (resize_rows)

- [ ] `Grid::resize_rows(&mut self, new_lines: usize)`
- [ ] **Row decrease (shrinking)**:
  - [ ] Prefer trimming trailing blank rows first (don't push empty rows to scrollback)
  - [ ] `count_trailing_blank_rows(max: usize) -> usize` — count blank rows from bottom, below cursor
  - [ ] After trimming blanks: push remaining excess top rows to scrollback
  - [ ] Adjust cursor row: `cursor.row = cursor.row.saturating_sub(rows_pushed_to_scrollback)`
  - [ ] Ensure at least `new_lines` rows in viewport (pad with empty if needed)
- [ ] **Row increase (growing)**:
  - [ ] If cursor at bottom of screen: pull lines from scrollback history (restore hidden content)
    - [ ] `from_scrollback = delta.min(scrollback.len())`
    - [ ] Pop from scrollback back, prepend to viewport
    - [ ] Adjust cursor row: `cursor.row += from_scrollback`
  - [ ] If cursor in middle: append empty rows at bottom (don't disturb scrollback)
- [ ] Resize dirty tracker to match new line count
- [ ] **Tests**:
  - [ ] Shrink: trailing blank rows trimmed first
  - [ ] Shrink: non-blank rows pushed to scrollback, cursor adjusted
  - [ ] Grow: empty rows added when cursor in middle
  - [ ] Grow: scrollback pulled when cursor at bottom
  - [ ] Zero-size guard: resize(0, 0) is no-op

---

## 12.4 Text Reflow

When columns change, reflow wrapped lines to fit the new width. Uses Ghostty-style cell-by-cell rewriting.

**File:** `oriterm_core/src/grid/reflow.rs`

**Reference:** `_old/src/grid/reflow.rs` (reflow_cols), Alacritty `grid/resize.rs`

- [ ] `Grid::resize(&mut self, new_cols: usize, new_lines: usize, reflow: bool)`
  - [ ] Guards: early return if 0x0 or dimensions unchanged
  - [ ] With reflow + column change:
    - [ ] Growing cols: reflow first (unwrap), then adjust rows
    - [ ] Shrinking cols: adjust rows first, then reflow (wrap)
    - [ ] Order matters: growing unwraps before row adjustment to avoid losing content; shrinking wraps after row adjustment to handle overflow correctly
  - [ ] Without reflow: resize rows, then resize each row's cell count
  - [ ] Reset scroll region after resize: `scroll_top = 0, scroll_bottom = new_lines - 1`
  - [ ] Clamp cursor: `cursor.row.min(lines - 1)`, `cursor.col.min(cols - 1)`
  - [ ] Clamp display_offset to scrollback length
  - [ ] Resize dirty tracker
- [ ] `Grid::reflow_cols(&mut self, new_cols: usize)` — unified cell-by-cell rewriting
  - [ ] Collect all rows: scrollback + visible, in order
  - [ ] Track cursor position in the unified list (absolute index + column)
  - [ ] Create output rows at new column width
  - [ ] For each source row:
    - [ ] Determine if wrapped: `WRAPLINE` flag set at old column boundary
    - [ ] Content length: wrapped rows use full width, non-wrapped rows trim trailing blanks
    - [ ] For each source cell:
      - [ ] Skip `WIDE_CHAR_SPACER` cells (regenerated at new positions)
      - [ ] Skip `LEADING_WIDE_CHAR_SPACER` cells (regenerated at new boundaries)
      - [ ] If cell doesn't fit in current output row: wrap to next row
        - [ ] Wide char at boundary: insert `LEADING_WIDE_CHAR_SPACER` padding
        - [ ] Set `WRAPLINE` flag on boundary cell
      - [ ] Write cell to output (strip old `WRAPLINE` flag)
      - [ ] Write wide char spacer in next column if wide
    - [ ] Non-wrapped source row: finalize output row
    - [ ] Wrapped source row: continue filling output row (unwrapping)
  - [ ] Track cursor through reflow: map old (abs_row, col) to new (abs_row, col)
  - [ ] Split result into scrollback + visible
  - [ ] Update cursor position from tracked coordinates
- [ ] **Wide char boundary handling**:
  - [ ] If wide char would be split at new column boundary:
    - [ ] Insert `LEADING_WIDE_CHAR_SPACER` at end of current row
    - [ ] Move wide char to start of next row
  - [ ] If grid too narrow for wide chars (`new_cols < 2`): treat as narrow
- [ ] **Cursor reflow**:
  - [ ] Track which content position the cursor was on before reflow
  - [ ] After reflow: place cursor at same content position in new layout
  - [ ] If cursor was on a spacer cell: adjust to the preceding wide char
  - [ ] If cursor past content: clamp to end of row
- [ ] Ensure all output rows have correct column count
- [ ] Ensure at least one row exists after reflow
- [ ] **Tests** (`oriterm_core/src/grid/reflow.rs` `#[cfg(test)]`):
  - [ ] Column increase: wrapped lines unwrap (WRAPLINE cleared, content merged)
  - [ ] Column decrease: long lines re-wrap (WRAPLINE set, content split)
  - [ ] Wide char at shrink boundary: `LEADING_WIDE_CHAR_SPACER` inserted, wide char moved to next row
  - [ ] Cursor preservation: cursor on 'X' before reflow is on 'X' after reflow
  - [ ] Scrollback reflow: content pushed to scrollback on shrink, pulled on grow
  - [ ] Empty grid: reflow produces at least one row
  - [ ] No-op: same column count does not modify grid

---

## 12.5 Alternate Screen Resize

Resize the alternate screen buffer without reflow (full-screen apps manage their own layout).

- [ ] When alternate screen is active: `grid.resize(new_cols, new_lines, reflow=false)`
  - [ ] Rows truncated or extended with blank cells
  - [ ] No WRAPLINE manipulation, no content merging/splitting
  - [ ] Cursor clamped to new bounds
- [ ] Apps like vim, htop, tmux send their own redraw commands after receiving SIGWINCH/ConPTY resize
- [ ] Both primary and alternate grids resized on every window resize
  - [ ] Primary: with reflow
  - [ ] Alternate: without reflow

---

## 12.6 Section Completion

- [ ] All 12.1-12.5 items complete
- [ ] `cargo test -p oriterm_core` — reflow tests pass
- [ ] `cargo clippy -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] Resizing the window resizes the grid with correct new dimensions
- [ ] PTY receives new dimensions on resize (no 0x0)
- [ ] Shell prompt redraws correctly after resize
- [ ] Text reflows when columns change (long lines wrap/unwrap)
- [ ] Wide characters handled at reflow boundaries (LEADING_WIDE_CHAR_SPACER)
- [ ] Cursor position preserved through resize and reflow
- [ ] No crash on zero-dimension resize
- [ ] No crash on rapid resize sequences
- [ ] Alternate screen resizes correctly (no reflow, cursor clamped)
- [ ] vim/htop/tmux redraw correctly after resize

**Exit Criteria:** Resizing the window produces correct terminal behavior -- text reflows, the shell adapts, full-screen apps redraw properly, and cursor position is preserved through resize operations.
