---
section: "04"
title: Resize Handling
status: complete
goal: Dynamic grid resize with PTY notification and text reflow on column change
sections:
  - id: "04.1"
    title: Window-to-Grid Resize
    status: complete
  - id: "04.2"
    title: PTY Resize Notification
    status: complete
  - id: "04.3"
    title: Text Reflow
    status: complete
  - id: "04.4"
    title: Completion Checklist
    status: complete
---

# Section 04: Resize Handling

**Status:** Complete
**Goal:** When the window resizes, the terminal grid resizes to match and the
PTY is notified of the new dimensions. Text reflows intelligently on column changes.

**Inspired by:**
- Alacritty's grid reflow (`grid/resize.rs`) with wide-char handling
- Ghostty's resize with semantic prompt awareness

**Implemented in:** `src/grid/mod.rs` (Grid::resize, reflow_grow_cols, reflow_shrink_cols), `src/grid/row.rs` (content_len, split_off, append, cells, cells_mut), `src/app.rs` (handle_resize), `src/tab.rs` (Tab::resize)

**What was built:**
- Dynamic grid resize following Ghostty's approach:
  - Row shrink: trim trailing blank rows first, push excess to scrollback
  - Row grow: add empty rows (don't pull scrollback unless cursor at bottom)
  - Column shrink/grow trigger text reflow
  - Reset scroll region after resize
- Text reflow on column change (04.3):
  - reflow_grow_cols: merges WRAPLINE-flagged rows back together when columns increase
  - reflow_shrink_cols: re-wraps lines exceeding new width with WRAPLINE flags
  - Wide character boundary handling (LEADING_WIDE_CHAR_SPACER at split points)
  - Cursor position tracked through reflow
  - Overflow rows go to scrollback
  - 5 dedicated reflow tests
- WindowEvent::Resized handler computes new cols/rows from pixel dimensions
- PTY resize notification via `pty_master.resize()`
- Both primary and alternate grids resized

---

## 04.1 Window-to-Grid Resize

Calculate new grid dimensions from window pixel size and cell metrics.

- [ ] On `window_event::Resized(new_physical_size)`:
  - [ ] Calculate available area: subtract tab bar height, padding, borders
  - [ ] New cols = `available_width / cell_width`
  - [ ] New rows = `available_height / cell_height`
  - [ ] Guard against zero dimensions (Alacritty: `if size == 0 { return; }`)
  - [ ] Call `grid.resize(new_cols, new_rows, reflow=true)`

- [ ] Store current grid dimensions for size comparison
- [ ] Only resize if dimensions actually changed
- [ ] Preserve cursor position relative to content (not absolute coordinates)

**Ref:** Alacritty `event.rs` zero-dimension guard, resize pipeline

---

## 04.2 PTY Resize Notification

Tell the child process about the new terminal size.

- [ ] After grid resize, notify PTY of new dimensions:
  - [ ] Windows (ConPTY): `portable-pty` `MasterPty::resize(PtySize)`
  - [ ] Unix: `ioctl(fd, TIOCSWINSZ, &winsize)` (handled by portable-pty)
- [ ] Include both character dimensions (cols, rows) and pixel dimensions
- [ ] `PtySize { rows, cols, pixel_width, pixel_height }`
- [ ] Never send 0x0 resize (crashes ConPTY on Windows)
- [ ] Store `MasterPty` handle in Tab for resize access (currently only kept as `_pty_master`)

**Ref:** Alacritty `tty/mod.rs` OnResize, WezTerm PtySize

---

## 04.3 Text Reflow

When columns change, reflow wrapped lines to fit the new width.

- [ ] **Column increase (grow)**:
  - [ ] Iterate rows in reverse
  - [ ] If row has `WRAPLINE` flag: merge with next row (unwrap)
  - [ ] Continue merging until line fits in new width or no more wrapped segments
  - [ ] Handle wide characters at boundaries (don't split a wide char)
  - [ ] Pull lines from scrollback if visible area has space after unwrapping

- [ ] **Column decrease (shrink)**:
  - [ ] Iterate rows
  - [ ] If row content exceeds new width: split at new width boundary
  - [ ] Set `WRAPLINE` flag on the split row
  - [ ] Push excess content to new row below
  - [ ] Handle wide characters at split point:
    - [ ] If a wide char would be split, insert `LEADING_WIDE_CHAR_SPACER` and
          move the wide char to the next line
  - [ ] Content that overflows visible area goes to scrollback

- [ ] **Row increase/decrease**:
  - [ ] Increase: pull lines from scrollback history
  - [ ] Decrease: push lines to scrollback history
  - [ ] Keep cursor at same content position

- [ ] Cursor reflow:
  - [ ] Track which content the cursor was on before reflow
  - [ ] After reflow, place cursor at the same content position in the new layout

**Ref:** Alacritty `grid/resize.rs` grow_columns/shrink_columns, LEADING_WIDE_CHAR_SPACER

---

## 04.4 Completion Checklist

- [x] Resizing the window resizes the grid
- [x] PTY receives new dimensions on resize
- [x] Shell prompt redraws correctly after resize
- [x] Text reflows when columns change (long lines wrap/unwrap)
- [x] Wide characters handled at reflow boundaries
- [x] Cursor position preserved through resize
- [x] No crash on zero-dimension resize
- [x] No crash on rapid resize sequences
- [x] Alternate screen resizes correctly (no reflow in alt screen)
- [x] vim/htop redraw correctly after resize (they query new size)

**Exit Criteria:** Resizing the window produces correct terminal behavior -- text
reflows, the shell adapts, and full-screen apps redraw properly.
