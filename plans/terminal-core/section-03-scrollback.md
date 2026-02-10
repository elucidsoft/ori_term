---
section: "03"
title: Scrollback Buffer
status: in-progress
goal: Implement a ring-buffer scrollback with efficient memory usage and viewport scrolling
sections:
  - id: "03.1"
    title: Ring Buffer Storage
    status: in-progress
  - id: "03.2"
    title: Viewport Scrolling
    status: complete
  - id: "03.3"
    title: Scroll Events
    status: complete
  - id: "03.4"
    title: Completion Checklist
    status: in-progress
---

# Section 03: Scrollback Buffer

**Status:** In Progress (functional Vec-based scrollback, ring buffer upgrade pending)
**Goal:** Lines scrolled off the top of the screen are preserved in a scrollback
buffer, allowing the user to scroll back through history.

**Inspired by:**
- Alacritty's ring buffer with O(1) rotation (`grid/storage.rs`)
- Ghostty's PageList with page-based memory management

**Implemented in:** `src/grid/mod.rs` (scrollback fields + scroll_up_in_region), `src/app.rs` (mouse wheel + keyboard scroll)

**What was built:**
- Vec-based scrollback with `max_scrollback: 10_000`
- `display_offset` for viewport positioning
- Mouse wheel scroll (3 lines per tick)
- Shift+PageUp/Down/Home/End keyboard shortcuts
- Viewport anchoring: when scrolled up, new output doesn't jump viewport (increments display_offset)
- Auto-scroll to live on keyboard input
- Alternate screen forces display_offset=0, no scrollback accumulation
- ED 3 clears scrollback

**Remaining:** Replace Vec with ring buffer (O(1) rotation instead of Vec::remove(0) which is O(n)). This is a performance optimization tracked in Section 15.

---

## 03.1 Ring Buffer Storage

Replace `Vec<Row>` with a ring buffer that reuses memory.

- [ ] Define `Storage` struct
  ```rust
  pub struct Storage {
      inner: Vec<Row>,         // physical buffer (scrollback + visible)
      zero: usize,             // logical start of visible area
      visible_lines: usize,    // number of visible lines
      len: usize,              // total active lines (scrollback + visible)
  }
  ```

- [ ] `rotate(count)` -- O(1) scroll by adjusting `zero`, no memory copy
- [ ] `push()` -- add line at bottom (scroll oldest off if at capacity)
- [ ] Index mapping: `physical_index = (zero + logical_index) % inner.len()`
- [ ] `max_scrollback` configurable (default: 10,000 lines)
- [ ] When scrollback is full, oldest lines are overwritten (ring semantics)

- [ ] Integrate with Grid:
  - [ ] `Grid::scroll_up()` rotates storage instead of discarding
  - [ ] Old top row becomes part of scrollback history
  - [ ] Bottom row is reset to blank

- [ ] Memory strategy:
  - [ ] Pre-allocate visible lines
  - [ ] Grow scrollback lazily up to `max_scrollback`
  - [ ] Shrink when explicitly requested (ED 3 -- erase scrollback)

**Ref:** Alacritty `grid/storage.rs` -- ring buffer with `zero` rotation pointer

---

## 03.2 Viewport Scrolling

Allow the user to view scrollback history.

- [ ] Add `display_offset: usize` to Grid
  - [ ] 0 = showing live terminal (bottom of scrollback)
  - [ ] N = scrolled up N lines from bottom
  - [ ] Clamped to `0..=scrollback_lines`

- [ ] Rendering reads from `(zero - display_offset)` for viewport
- [ ] When `display_offset > 0`, new output doesn't move viewport (user stays in place)
- [ ] When `display_offset == 0`, new output is visible immediately
- [ ] Auto-scroll to bottom on:
  - [ ] User keyboard input to PTY
  - [ ] Explicit scroll-to-bottom command

- [ ] Alternate screen has no scrollback:
  - [ ] `display_offset` forced to 0 on alt screen
  - [ ] No scrollback accumulation on alt screen

**Ref:** Alacritty `display_offset`, Ghostty viewport concept

---

## 03.3 Scroll Events

Wire scrollback to user input.

- [ ] Mouse wheel events:
  - [ ] Scroll up: `display_offset += scroll_amount`
  - [ ] Scroll down: `display_offset -= scroll_amount` (min 0)
  - [ ] Default scroll amount: 3 lines per wheel tick
  - [ ] When in alternate screen with ALTERNATE_SCROLL mode, convert wheel to
        up/down arrow key sequences (for less, vim, etc.)

- [ ] Keyboard scroll shortcuts:
  - [ ] Shift+PageUp: scroll up one page
  - [ ] Shift+PageDown: scroll down one page
  - [ ] Shift+Home: scroll to top of scrollback
  - [ ] Shift+End: scroll to bottom (live)

- [ ] Visual indicator:
  - [ ] When scrolled up, show a scroll position indicator (optional, can be deferred)

---

## 03.4 Completion Checklist

- [ ] Scrollback preserves lines that scroll off the top
- [ ] Mouse wheel scrolls through history
- [ ] Shift+PageUp/PageDown works
- [ ] New output appears at bottom while scrolled up (viewport stays)
- [ ] Typing scrolls back to bottom
- [ ] Alternate screen has no scrollback
- [ ] ED 3 clears scrollback
- [ ] Scrollback configurable (max lines)
- [ ] Memory usage stays bounded
- [ ] No visible performance impact with 10,000+ lines of scrollback

**Exit Criteria:** User can scroll through terminal history with mouse wheel and
keyboard shortcuts. Scrollback survives across multiple screens of output.
