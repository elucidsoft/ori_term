---
section: "15"
title: Performance
status: not-started
goal: Optimize rendering, parsing, and memory to handle heavy terminal workloads smoothly
sections:
  - id: "15.1"
    title: Damage Tracking
    status: not-started
  - id: "15.2"
    title: Parsing Performance
    status: not-started
  - id: "15.3"
    title: Memory Optimization
    status: not-started
  - id: "15.4"
    title: Rendering Performance
    status: not-started
  - id: "15.5"
    title: Benchmarks
    status: not-started
  - id: "15.6"
    title: Completion Checklist
    status: not-started
---

# Section 15: Performance

**Status:** Not Started
**Goal:** Terminal handles heavy workloads (large file cats, rapid output, complex TUIs)
without lag, dropped frames, or excessive memory usage.

**Inspired by:**
- Alacritty's "fastest terminal emulator" design: batched rendering, ring buffer, SIMD parsing
- Ghostty's page-based memory management and damage tracking
- VTE library's parser performance (state machine, SIMD table lookup)

**Current state:** CPU softbuffer rendering, pixel-by-pixel alpha blending, `Vec<Row>`
scrollback (not ring buffer), no damage tracking (full redraw every frame), no frame
rate limiting.

---

## 15.1 Damage Tracking

Only redraw cells that changed since last frame.

- [ ] Per-row dirty flag: mark rows as dirty when cells are modified
- [ ] Dirty sources:
  - [ ] Character write (`put_char`, `put_wide_char`)
  - [ ] Erase operations (`erase_display`, `erase_line`)
  - [ ] Scroll operations (mark all rows in scroll region)
  - [ ] Cursor movement (mark old and new cursor rows)
  - [ ] Scroll position change (mark all rows)
- [ ] Render only dirty rows:
  - [ ] Keep previous frame buffer
  - [ ] Only re-render cells in dirty rows
  - [ ] Clear dirty flags after render
- [ ] Full redraw triggers: resize, font change, scroll position change
- [ ] Measure: frames per second counter, dirty cell percentage

**Ref:** Ghostty damage tracking, Alacritty dirty state tracking

---

## 15.2 Parsing Performance

Optimize VTE sequence parsing throughput.

- [ ] Profile parsing: identify hot paths in `term_handler.rs`
- [ ] Batch processing: process entire PTY read buffer in one call
  - [ ] Already doing this with `processor.advance()`
  - [ ] Ensure we read large chunks from PTY (4KB+ per read)
- [ ] Fast path for printable ASCII:
  - [ ] Detect runs of plain ASCII characters (0x20-0x7E)
  - [ ] Write entire run to grid without per-character dispatch
  - [ ] This is the most common case for terminal output
- [ ] Reduce allocations in hot path:
  - [ ] Avoid creating String/Vec during normal character processing
  - [ ] Pre-allocate buffers for common operations
- [ ] PTY read buffer size: 4KB minimum, 64KB for high throughput
- [ ] Throttle rendering during heavy output:
  - [ ] Don't redraw for every PTY read
  - [ ] Batch output processing, redraw at frame intervals (~16ms)

**Ref:** Alacritty parsing optimization, vte crate performance

---

## 15.3 Memory Optimization

Control memory usage, especially for scrollback.

- [ ] Ring buffer for scrollback (replace `Vec<Row>` with ring):
  - [ ] `Storage { inner: Vec<Row>, zero: usize, len: usize }`
  - [ ] O(1) rotation: adjust `zero` pointer, no memory copies
  - [ ] Fixed capacity: when full, oldest row is overwritten
  - [ ] Current Vec-based approach does `remove(0)` which is O(n)
- [ ] Row memory:
  - [ ] Row occupancy tracking (`occ` field) for efficient reset
  - [ ] Only allocate `CellExtra` when needed (already using `Option<Arc>`)
  - [ ] Consider compact row representation for blank rows
- [ ] Scrollback memory limit:
  - [ ] Default 10,000 lines (configurable)
  - [ ] Memory estimate: ~24 bytes/cell * 200 cols * 10k rows = ~48MB
  - [ ] For very large scrollback, consider compressing old rows
- [ ] Grid resize: reuse existing row allocations when possible

**Ref:** Alacritty `grid/storage.rs` ring buffer, Ghostty `PageList` paging

---

## 15.4 Rendering Performance

Optimize the rendering pipeline.

- [ ] CPU rendering optimizations (if keeping softbuffer):
  - [ ] SIMD alpha blending (process 4 pixels at a time)
  - [ ] Batch background fills (fill entire row background at once)
  - [ ] Skip rendering blank rows (only background)
  - [ ] Pre-compute fg/bg colors for common cases
- [ ] GPU rendering optimizations (if using wgpu):
  - [ ] Instance buffer: one draw call for all cell backgrounds
  - [ ] Instance buffer: one draw call for all glyphs
  - [ ] Glyph atlas: minimize texture switches
  - [ ] Depth/stencil: avoid overdraw
- [ ] Frame pacing:
  - [ ] VSync: render at display refresh rate
  - [ ] Don't render faster than 60fps (or display rate)
  - [ ] When idle (no output), don't redraw at all
  - [ ] Use `window.request_redraw()` only when state changes
- [ ] Double buffering:
  - [ ] Render to back buffer while displaying front buffer
  - [ ] Swap on VSync

**Ref:** Ghostty renderer optimizations, Alacritty performance design

---

## 15.5 Benchmarks

Establish performance baselines and regression testing.

- [ ] Throughput benchmark:
  - [ ] `cat large_file.txt` — measure time to process N MB of text
  - [ ] Target: >100 MB/s parsing throughput
  - [ ] Compare with Alacritty, Ghostty, Windows Terminal
- [ ] Rendering benchmark:
  - [ ] Full-screen colored text: measure FPS
  - [ ] Target: 60fps with full screen of colored text
  - [ ] Rapidly scrolling output: measure frame drops
- [ ] Memory benchmark:
  - [ ] Memory usage with 10k lines of scrollback
  - [ ] Memory usage with 100k lines of scrollback
  - [ ] Memory per tab
- [ ] Latency benchmark:
  - [ ] Keypress to screen update latency
  - [ ] Target: <5ms (perceived instant)
  - [ ] Use `typometer` or similar tool
- [ ] Regression testing:
  - [ ] Run benchmarks on CI
  - [ ] Alert on >10% regression

**Ref:** Alacritty benchmark suite, Ghostty performance testing

---

## 15.6 Completion Checklist

- [ ] `cat` of 100MB file completes without noticeable lag
- [ ] 60fps maintained with full screen of colored text
- [ ] Scrollback doesn't cause O(n) operations (ring buffer)
- [ ] Memory usage bounded by scrollback limit
- [ ] Damage tracking reduces unnecessary rendering
- [ ] Frame pacing prevents excessive CPU/GPU usage when idle
- [ ] Keypress latency under 5ms
- [ ] No visible jank during rapid output (e.g., `yes | head -100000`)
- [ ] Benchmarks established and documented

**Exit Criteria:** Terminal handles heavy workloads (large file output, rapid
scrolling, complex TUIs) smoothly at 60fps with bounded memory usage.
