---
section: "15"
title: Performance
status: in-progress
goal: Optimize rendering, parsing, and memory to handle heavy terminal workloads smoothly
sections:
  - id: "15.1"
    title: Damage Tracking
    status: not-started
  - id: "15.2"
    title: Parsing Performance
    status: in-progress
  - id: "15.3"
    title: Memory Optimization
    status: not-started
  - id: "15.4"
    title: Rendering Performance
    status: in-progress
  - id: "15.5"
    title: Benchmarks
    status: not-started
  - id: "15.6"
    title: Completion Checklist
    status: not-started
---

# Section 15: Performance

**Status:** In Progress (GPU rendering pipeline is optimized, but no damage tracking
or ring buffer yet)
**Goal:** Terminal handles heavy workloads (large file cats, rapid output, complex TUIs)
without lag, dropped frames, or excessive memory usage.

**Inspired by:**
- Alacritty's "fastest terminal emulator" design: batched rendering, ring buffer, SIMD parsing
- Ghostty's page-based memory management and damage tracking
- VTE library's parser performance (state machine, SIMD table lookup)

**Current state:** wgpu GPU-accelerated rendering with instanced draw calls (80-byte
stride), glyph texture atlas with lazy rasterization and ASCII pre-cache, two-pass
pipeline (background + foreground). `Vec<Row>` scrollback with O(n) removal at front.
No damage tracking — full instance buffer rebuild every frame. No explicit frame
rate limiting (wgpu vsync handles pacing). PTY reads are processed as they arrive
with a redraw request per batch.

---

## 15.1 Damage Tracking

Only redraw cells that changed since last frame.

**Why:** Currently `build_grid_instances()` in `src/gpu/renderer.rs` iterates every
visible cell every frame (~120 cols x 30 rows = 3,600 cells), building instance data
even when nothing changed. For idle terminals this is wasted GPU work.

- [ ] Per-row dirty flag on `Row` struct (`src/grid/row.rs`):
  - [ ] Add `dirty: bool` field (or use a `BitVec` on Grid for cache-friendliness)
  - [ ] Mark dirty on: `put_char`, `put_wide_char`, any cell write
  - [ ] Mark dirty on: `erase_line`, `erase_display`, `erase_chars`
  - [ ] Mark all rows in scroll region dirty on: `scroll_up`, `scroll_down`
  - [ ] Mark old + new cursor rows dirty on cursor movement
  - [ ] Mark all visible rows dirty on: scroll position change, resize, font change
- [ ] Instance buffer caching:
  - [ ] Keep previous frame's instance buffer
  - [ ] Only regenerate instances for dirty rows
  - [ ] Splice updated row instances into the buffer
  - [ ] Clear dirty flags after render
- [ ] Full redraw triggers (mark everything dirty):
  - [ ] Window resize
  - [ ] Font size change
  - [ ] Color scheme change
  - [ ] Scroll position change (viewport shift)
  - [ ] Selection change (could optimize to just affected rows)
- [ ] Optimization: skip `frame.present()` entirely when nothing is dirty and
  cursor hasn't blinked
- [ ] Measure: add optional FPS counter and dirty-cell percentage to debug overlay

**Complexity:** Medium. The main challenge is ensuring every mutation path marks
the right rows dirty. Missing a path causes stale rendering.

**Ref:** Ghostty damage tracking, Alacritty dirty state tracking, ratatui diff

---

## 15.2 Parsing Performance

Optimize VTE sequence parsing throughput.

**Current state:** PTY reader thread reads into a buffer and sends `PtyOutput`
events. The main thread feeds bytes through both a raw `vte::Parser` (for OSC 7/133)
and the high-level `vte::ansi::Processor`. This is already reasonably fast — vte uses
a state machine with table-driven dispatch.

- [x] Batch processing: entire PTY read buffer processed in one `processor.advance()` call
- [x] PTY reader sends `Vec<u8>` chunks (not byte-by-byte)
- [ ] Increase PTY read buffer size:
  - [ ] Current: system default (typically 4KB–8KB per `read()`)
  - [ ] Target: 64KB buffer for high-throughput scenarios
  - [ ] Use `BufReader` with explicit capacity on the PTY reader
- [ ] Fast path for printable ASCII runs:
  - [ ] Detect consecutive ASCII characters (0x20–0x7E) in the input
  - [ ] Write entire run to grid cells without per-character VTE dispatch
  - [ ] This is the dominant case for `cat large_file.txt`
  - [ ] Requires bypassing vte for the fast path — may not be worth the complexity
  - [ ] Alternative: profile to see if vte is actually the bottleneck
- [ ] Reduce allocations in hot path:
  - [ ] `term_handler.rs` `input()` already writes directly to grid (no String alloc)
  - [ ] Audit for hidden allocations (String formatting in log macros, etc.)
  - [ ] Ensure log macros are no-ops when logging is disabled
- [ ] Throttle rendering during heavy output:
  - [ ] Don't request redraw for every `PtyOutput` event
  - [ ] Coalesce: process all pending `PtyOutput` events before requesting one redraw
  - [ ] Or: time-based throttle — at most one redraw per 16ms (~60fps)
  - [ ] This prevents the event loop from being starved by rapid output

**Ref:** Alacritty parsing optimization, vte crate performance, Ghostty batching

---

## 15.3 Memory Optimization

Control memory usage, especially for scrollback.

**Current state:** Scrollback uses `Vec<Row>` in `src/grid/mod.rs`. When scrollback
exceeds `max_scrollback`, the oldest row is removed with `self.scrollback.remove(0)`,
which is O(n) — copying all remaining rows. At 10,000 lines this is measurable.

- [ ] Ring buffer for scrollback:
  ```rust
  struct ScrollbackRing {
      inner: Vec<Row>,
      head: usize,    // index of newest row
      len: usize,     // number of used slots
      capacity: usize,
  }
  ```
  - [ ] O(1) push: increment head, overwrite oldest
  - [ ] O(1) index: `inner[(head - offset) % capacity]`
  - [ ] Pre-allocate to `max_scrollback` capacity
  - [ ] Current `remove(0)` is O(n) — ring buffer eliminates this
  - [ ] Alacritty uses this pattern in `grid/storage.rs`
- [ ] Row memory optimization:
  - [x] Row occupancy tracking (`occ` field) already avoids processing blank cells
  - [x] `CellExtra` uses `Option<Arc>` — zero cost when not needed
  - [ ] Consider compact representation for all-default rows (just store width)
  - [ ] Consider `SmallVec` for rows shorter than a threshold
- [ ] Scrollback memory estimates:
  - [ ] 24 bytes/cell x 120 cols x 10,000 rows = ~28.8 MB
  - [ ] With `CellExtra` on <1% of cells, this is close to theoretical minimum
  - [ ] For 100k lines: ~288 MB — may want compressed old rows
- [ ] Grid resize memory:
  - [ ] Reuse existing `Row` allocations when shrinking/growing
  - [ ] Currently creates new rows — could pool and reuse

**Ref:** Alacritty `grid/storage.rs` ring buffer, Ghostty `PageList` paging

---

## 15.4 Rendering Performance

Optimize the GPU rendering pipeline.

**Current state:** wgpu instanced rendering with two pipelines (background quads
and foreground glyph textures). Instance buffer rebuilt every frame. Glyph atlas
uses row-based shelf packing in a 1024x1024 R8Unorm texture. ASCII glyphs
pre-cached. sRGB-correct pipeline with linear alpha blending and optional
luminance-based alpha correction.

- [x] Instance-driven rendering: two draw calls per frame (bg + fg)
- [x] Glyph atlas with lazy rasterization and ASCII pre-cache
- [x] sRGB-correct rendering pipeline (`src/gpu/pipeline.rs`)
- [x] Premultiplied alpha blending for compositor transparency
- [ ] Instance buffer optimizations:
  - [ ] With damage tracking (15.1), only rebuild instances for dirty rows
  - [ ] Use `wgpu::BufferUsages::COPY_DST` for partial buffer updates
  - [ ] Or use a persistent mapped buffer and update only changed regions
- [ ] Glyph atlas improvements:
  - [ ] Current: 1024x1024 R8Unorm texture — good for ~2000+ unique glyphs
  - [ ] If atlas fills up: grow to 2048x2048 or create additional atlas pages
  - [ ] Track atlas utilization; warn in debug log when >80% full
  - [ ] Consider R8Unorm → RGBA8 for color emoji support (Section 19 prerequisite)
- [ ] Frame pacing:
  - [x] wgpu presentation handles VSync automatically
  - [ ] Avoid rendering when nothing changed (requires damage tracking)
  - [ ] Use `window.request_redraw()` only when state changes (not every event)
  - [ ] Track: did any PTY output arrive? Did cursor blink? Did selection change?
- [ ] Draw call reduction:
  - [ ] Currently 2 draw calls (bg + fg) — already minimal
  - [ ] Tab bar, grid, and overlays share the same pipelines
  - [ ] Could add a third "overlay" pipeline for search bar / dropdown
    (already partially done)
- [ ] Skip rendering off-screen content:
  - [ ] Don't generate instances for cells that are fully clipped
  - [ ] Relevant when window is partially off-screen

**Ref:** Ghostty renderer optimizations, Alacritty performance design

---

## 15.5 Benchmarks

Establish performance baselines and regression testing.

- [ ] Throughput benchmark:
  - [ ] `cat large_file.txt` — measure time to process N MB of text
  - [ ] Target: >100 MB/s parsing throughput
  - [ ] Use a synthetic test: write 100MB of random ASCII to PTY, measure wall time
  - [ ] Compare with Alacritty, Ghostty, Windows Terminal
- [ ] Rendering benchmark:
  - [ ] Full-screen colored text: measure FPS
  - [ ] Target: 60fps with full screen of colored text and attributes
  - [ ] Rapidly scrolling output (`yes | head -100000`): measure frame drops
  - [ ] Stress test: 256-color gradient filling the screen
- [ ] Memory benchmark:
  - [ ] Memory usage with 10k lines of scrollback
  - [ ] Memory usage with 100k lines of scrollback
  - [ ] Memory per tab (baseline overhead)
  - [ ] Memory growth over time (detect leaks)
- [ ] Latency benchmark:
  - [ ] Keypress to screen update latency
  - [ ] Target: <5ms (perceived instant)
  - [ ] Use `typometer` or similar tool
  - [ ] Measure: time from `KeyboardInput` event to `frame.present()` call
- [ ] Regression testing:
  - [ ] Criterion-based microbenchmarks for grid operations, parsing
  - [ ] Run on CI, compare against baseline
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
