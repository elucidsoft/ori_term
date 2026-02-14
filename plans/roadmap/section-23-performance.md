---
section: 23
title: Performance & Damage Tracking
status: not-started
tier: 5
goal: Optimize rendering, parsing, and memory for heavy terminal workloads
sections:
  - id: "23.1"
    title: Damage Tracking
    status: not-started
  - id: "23.2"
    title: Parsing Performance
    status: not-started
  - id: "23.3"
    title: Memory Optimization
    status: not-started
  - id: "23.4"
    title: Rendering Performance
    status: not-started
  - id: "23.5"
    title: Benchmarks
    status: not-started
  - id: "23.6"
    title: Section Completion
    status: not-started
---

# Section 23: Performance & Damage Tracking

**Status:** Not Started
**Goal:** Terminal handles heavy workloads (large file cats, rapid output, complex TUIs) without lag, dropped frames, or excessive memory usage. Every optimization must be measurable — no speculative "optimization" without profiling.

**Crate:** `oriterm_core` (grid damage tracking, ring buffer), `oriterm` (rendering, parsing pipeline)
**Dependencies:** `criterion` (benchmarks), `wgpu` (GPU), `vte` (parser)

**Reference:**
- Alacritty's "fastest terminal emulator" design: batched rendering, ring buffer, SIMD parsing
- Ghostty's page-based memory management and damage tracking
- VTE library's parser performance (state machine, table-driven dispatch)

**Current state (from old codebase):** wgpu GPU-accelerated rendering with instanced draw calls (80-byte stride), glyph texture atlas with lazy rasterization and ASCII pre-cache, two-pass pipeline (background + foreground). `Vec<Row>` scrollback with O(n) removal at front. No damage tracking — full instance buffer rebuild every frame. No explicit frame rate limiting (wgpu vsync handles pacing). PTY reads are processed as they arrive with a redraw request per batch.

---

## 23.1 Damage Tracking

Only redraw cells that changed since last frame. Currently `build_grid_instances()` iterates every visible cell every frame (~120 cols x 30 rows = 3,600 cells), building instance data even when nothing changed. For idle terminals this is wasted GPU work.

**Files:** `oriterm_core/src/grid/dirty.rs`, `oriterm_core/src/grid/row.rs`, `oriterm/src/gpu/renderer.rs`

**Reference:** `_old/src/grid/dirty.rs`, `_old/src/grid/row.rs`, `_old/src/gpu/renderer.rs`, Ghostty damage tracking, Alacritty dirty state tracking

### Per-Row Dirty Flag

- [ ] Add dirty tracking to the grid (BitVec on Grid for cache-friendliness, or `dirty: bool` per Row):
  - [ ] Mark dirty on: `put_char`, `put_wide_char`, any cell write via `IndexMut`
  - [ ] Mark dirty on: `erase_line`, `erase_display`, `erase_chars`
  - [ ] Mark all rows in scroll region dirty on: `scroll_up`, `scroll_down`
  - [ ] Mark old cursor row + new cursor row dirty on cursor movement
  - [ ] Mark all visible rows dirty on: scroll position change, resize, font change, color scheme change
- [ ] Correctness invariant: every code path that mutates a cell must mark the containing row dirty. Missing a path causes stale rendering — this is the main risk.

### Instance Buffer Caching

- [ ] Keep previous frame's instance buffer (Vec of instance data)
- [ ] Only regenerate instances for dirty rows
- [ ] Splice updated row instances into the cached buffer at the correct offset
  - [ ] Each row maps to a contiguous range in the instance buffer
  - [ ] Row N starts at offset `N * cols` in the instance array (approximately — wide chars and tab bar may shift offsets)
- [ ] Clear dirty flags after render (in `drain()` or after instance buffer is uploaded)
- [ ] On first frame or after a full-redraw trigger, rebuild the entire buffer

### Full Redraw Triggers

These events mark everything dirty (conservative — can optimize individual cases later):

- [ ] Window resize
- [ ] Font size change (Ctrl+Plus/Minus)
- [ ] Color scheme / palette change
- [ ] Scroll position change (viewport shift via scrollback navigation)
- [ ] Selection change (could optimize to just affected rows in the future)
- [ ] Alt screen swap (enter/exit alternate screen buffer)
- [ ] Tab switch (different tab has entirely different grid)

### Skip Present When Clean

- [ ] If no rows are dirty AND cursor has not blinked AND no overlay state changed, skip `frame.present()` entirely
- [ ] Track a `needs_redraw: bool` flag that is set by any mutation and cleared after present
- [ ] Reduces GPU power usage and heat when terminal is idle

### Debug Overlay

- [ ] Optional FPS counter and dirty-row percentage in debug overlay
- [ ] Toggled via config flag or keyboard shortcut
- [ ] Shows: current FPS, dirty rows this frame, total instance count, atlas utilization

- [ ] **Tests** (`oriterm_core/src/grid/dirty.rs` `#[cfg(test)]`):
  - [ ] New grid: nothing dirty after initial build
  - [ ] `put_char` marks the cursor row dirty
  - [ ] `erase_line` marks the target row dirty
  - [ ] `scroll_up` marks all rows in scroll region dirty
  - [ ] Cursor move marks both old and new rows dirty
  - [ ] `drain()` returns dirty row indices and resets all to clean
  - [ ] After drain, `is_any_dirty()` returns false
  - [ ] Resize marks all rows dirty

---

## 23.2 Parsing Performance

Optimize VTE sequence parsing throughput for high-volume output.

**Files:** `oriterm/src/tab/mod.rs` (PTY processing), `oriterm_core/src/term_handler/mod.rs` (VTE handler)

**Reference:** `_old/src/tab/mod.rs`, `_old/src/term_handler/mod.rs`, Alacritty parsing optimization, vte crate performance

### Batch Processing (already done in old codebase)

- [ ] Entire PTY read buffer processed in one `processor.advance()` call (not byte-by-byte)
- [ ] PTY reader sends `Vec<u8>` chunks via channel or shared buffer

### Increase PTY Read Buffer

- [ ] Current: system default (typically 4KB-8KB per `read()`)
- [ ] Target: 64KB buffer for high-throughput scenarios
- [ ] Use `BufReader::with_capacity(65536, pty_reader)` on the PTY reader
- [ ] Larger buffers reduce syscall overhead and allow vte to process longer runs

### Fast ASCII Path

- [ ] Detect consecutive printable ASCII characters (0x20-0x7E) in the input stream
- [ ] Write entire ASCII run to grid cells without per-character VTE state machine dispatch
- [ ] This is the dominant case for `cat large_file.txt` (>95% of bytes are printable ASCII)
- [ ] Implementation options:
  - [ ] Option A: Pre-scan buffer for ASCII runs before feeding to vte — write directly to grid
  - [ ] Option B: Optimize the vte handler's `input()` method to batch ASCII writes (memcpy-style)
  - [ ] Option C: Profile first to confirm vte dispatch is actually the bottleneck (it may not be)
- [ ] Decision: profile before implementing. If vte's table-driven dispatch is already fast enough, skip this optimization.

### Reduce Allocations in Hot Path

- [ ] `input()` handler already writes directly to grid (no String allocation) — verify this in the new codebase
- [ ] Audit for hidden allocations:
  - [ ] String formatting in `log::debug!()` / `log::trace!()` macros — ensure these are compiled out at default log level
  - [ ] Temporary `Vec` allocations in scroll/reflow operations
  - [ ] `format!()` calls in error paths (acceptable since errors are rare)
- [ ] Ensure log macros are zero-cost when the log level is not enabled (the `log` crate handles this, but verify custom macros)

### Throttle Rendering During Heavy Output

- [ ] Do not request a redraw for every PTY output chunk
- [ ] Coalesce: process all pending PTY output events before requesting one redraw
  - [ ] Drain the channel/queue of all available PTY data, process all of it, then request exactly one redraw
- [ ] Time-based throttle: at most one redraw per 16ms (~60fps) during sustained output
  - [ ] Track `last_redraw_time` and skip redraw requests that arrive within the window
  - [ ] This prevents the event loop from being starved by rapid output (e.g., `yes | head -1000000`)
- [ ] After heavy output subsides, ensure a final redraw is requested to show the latest state

- [ ] **Tests:**
  - [ ] Processing a 1MB ASCII buffer does not allocate (use a custom allocator or `#[global_allocator]` tracking in a benchmark)
  - [ ] Rendering throttle limits redraws to ~60fps during sustained output (timing test)
  - [ ] All PTY data is processed even when rendering is throttled (no data loss)

---

## 23.3 Memory Optimization

Control memory usage, especially for scrollback. The primary target is replacing the O(n) `Vec::remove(0)` with an O(1) ring buffer.

**Files:** `oriterm_core/src/grid/ring.rs`, `oriterm_core/src/grid/row.rs`, `oriterm_core/src/grid/mod.rs`

**Reference:** `_old/src/grid/ring.rs`, `_old/src/grid/row.rs`, Alacritty `grid/storage.rs` ring buffer, Ghostty `PageList` paging

### Ring Buffer for Scrollback

- [ ] Replace `Vec<Row>` scrollback with a ring buffer:
  ```rust
  struct ScrollbackRing {
      inner: Vec<Row>,
      head: usize,     // index of newest row
      len: usize,      // number of used slots
      capacity: usize, // max scrollback lines
  }
  ```
- [ ] O(1) push: increment head (wrapping), overwrite oldest row in place
  - [ ] Current `Vec::remove(0)` is O(n) — copies all remaining rows on every scroll
  - [ ] At 10,000 lines of scrollback, this is measurably slow
- [ ] O(1) index: `inner[(head - offset) % capacity]` — no shifting
- [ ] Pre-allocate to `max_scrollback` capacity at tab creation
  - [ ] Avoids incremental `Vec` growth and reallocation during use
  - [ ] Memory is committed upfront — acceptable since scrollback is bounded
- [ ] `clear()` resets `len` and `head` without deallocating (reuse the allocation)
- [ ] Alacritty uses this exact pattern in `grid/storage.rs` — reference their implementation

### Row Memory Optimization

- [ ] Row occupancy tracking (`occ` field) already avoids processing blank trailing cells — carry this forward
- [ ] `CellExtra` uses `Option<Box<CellExtra>>` — zero cost (8 bytes, null pointer) when not needed
  - [ ] Less than 1% of cells typically have `CellExtra` (only cells with colored underlines, hyperlinks, or combining marks)
- [ ] Consider compact representation for all-default rows:
  - [ ] Rows that are entirely blank (e.g., after `erase_display`) could store just the column count
  - [ ] Expand to full `Vec<Cell>` on first write
  - [ ] Trade-off: adds branching on every cell access — profile before implementing
- [ ] Consider `SmallVec<[Cell; 80]>` for rows shorter than 80 columns:
  - [ ] Avoids heap allocation for small terminal windows
  - [ ] Trade-off: `SmallVec` has overhead and may not be worth it for 24-byte cells

### Scrollback Memory Estimates

- [ ] 24 bytes/cell x 120 cols x 10,000 rows = ~28.8 MB per tab
- [ ] With `CellExtra` on <1% of cells, actual usage is close to this theoretical minimum
- [ ] For 100,000 lines: ~288 MB — consider compressed/compacted old rows for very large scrollback
  - [ ] Option: store rows older than N lines in a compressed format (e.g., run-length encode blank cells)
  - [ ] Deferred — 10K lines is the default and 28.8 MB is acceptable

### Grid Resize Memory Reuse

- [ ] When grid shrinks (fewer columns or rows), reuse existing `Row` allocations:
  - [ ] Truncate rows in place rather than creating new ones
  - [ ] Pool freed rows for reuse on subsequent growth
- [ ] When grid grows, extend existing rows rather than allocating new ones where possible
- [ ] Currently creates entirely new rows on resize — inefficient for frequent resize events

- [ ] **Tests** (`oriterm_core/src/grid/ring.rs` `#[cfg(test)]`):
  - [ ] Push rows into ring, verify retrieval order (newest first via index 0)
  - [ ] Ring wraps: push `capacity + 10` rows, only `capacity` retained, oldest evicted
  - [ ] Index wraps correctly: `get(0)` is newest, `get(len-1)` is oldest
  - [ ] Clear resets length to 0, allocation is preserved
  - [ ] Pre-allocation: `inner.capacity() == max_scrollback` after construction
  - [ ] Integration: `grid.scroll_up()` pushes evicted row to ring buffer
  - [ ] Memory: ring buffer does not grow beyond `capacity`

---

## 23.4 Rendering Performance

Optimize the GPU rendering pipeline for minimal CPU and GPU overhead per frame.

**Files:** `oriterm/src/gpu/renderer.rs`, `oriterm/src/gpu/instance_writer.rs`, `oriterm/src/gpu/atlas.rs`, `oriterm/src/gpu/state.rs`

**Reference:** `_old/src/gpu/renderer.rs`, `_old/src/gpu/instance_writer.rs`, `_old/src/gpu/atlas.rs`, Ghostty renderer optimizations, Alacritty performance design

### Instance Buffer Partial Updates

- [ ] With damage tracking (23.1), only rebuild instances for dirty rows
- [ ] Use `wgpu::Queue::write_buffer_with()` or `write_buffer()` for partial buffer updates
  - [ ] Calculate byte offset for the dirty row's instance range
  - [ ] Upload only the changed region, not the entire buffer
- [ ] Alternative: persistent mapped buffer with `wgpu::BufferUsages::MAP_WRITE | COPY_SRC`
  - [ ] Map once, write dirty regions, unmap before draw
  - [ ] May have better performance for frequent small updates
- [ ] Measure: compare full-buffer upload vs. partial update latency

### Glyph Atlas Growth

- [ ] Current: 1024x1024 R8Unorm texture — good for ~2,000+ unique glyphs
- [ ] If atlas fills up (shelf packer returns `None`):
  - [ ] Strategy A: grow to 2048x2048 (copy existing data, allocate new texture)
  - [ ] Strategy B: create additional atlas pages (multi-texture, requires bind group changes)
  - [ ] Strategy C: evict least-recently-used glyphs (complex, may cause re-rasterization)
  - [ ] Recommend Strategy A for simplicity — 2048x2048 holds ~8,000+ glyphs
- [ ] Track atlas utilization: log at debug level when >80% full
- [ ] Future: RGBA8 atlas for color emoji support (requires additional texture or atlas page)

### Frame Pacing

- [ ] wgpu presentation handles VSync automatically (already in place)
- [ ] Avoid rendering when nothing changed (requires damage tracking from 23.1):
  - [ ] Track: did any PTY output arrive since last frame?
  - [ ] Track: did cursor blink state change?
  - [ ] Track: did selection change?
  - [ ] Track: did any overlay (search bar, settings) update?
  - [ ] If nothing changed, do not call `request_redraw()` and skip the entire render pass
- [ ] Use `window.request_redraw()` only in response to actual state changes, not every event loop iteration
- [ ] Idle terminal should use near-zero CPU (only wake for cursor blink timer)

### Draw Call Reduction

- [ ] Currently 2 draw calls per frame (background quads + foreground glyph quads) — already minimal
- [ ] Tab bar, grid, and overlays share the same two pipelines — no additional draw calls needed
- [ ] If overlay pipeline is added (search bar, dropdown menus), limit to one additional draw call
- [ ] Batch all instances into a single buffer per pipeline (already the approach)

### Skip Off-Screen Content

- [ ] Do not generate instances for cells that are fully clipped (outside the viewport)
- [ ] Relevant when the window is partially off-screen or when rendering a sub-region
- [ ] For normal rendering, all visible cells are "on-screen" by definition — this optimization matters primarily for overlapping UI elements or partially visible rows at the edges

- [ ] **Tests:**
  - [ ] Partial buffer update produces the same visual result as full rebuild (visual regression test)
  - [ ] Atlas growth succeeds without visual artifacts (glyph coordinates remain valid)
  - [ ] Idle terminal with no PTY output does not trigger redraws (measure redraw count over 5 seconds)

---

## 23.5 Benchmarks

Establish performance baselines and regression testing. Every optimization in this section must be validated by benchmarks.

**Files:** `oriterm_core/benches/grid.rs`, `oriterm/benches/rendering.rs` (new benchmark crates)

**Reference:** Alacritty benchmark suite, Ghostty performance testing, `criterion` crate

### Throughput Benchmark

- [ ] `cat large_file.txt` — measure time to process N MB of text through the VTE parser and into the grid
- [ ] Target: >100 MB/s parsing throughput (Alacritty achieves ~200 MB/s)
- [ ] Synthetic test: generate 100MB of random printable ASCII, feed through `vte::Processor` and handler, measure wall time
- [ ] Include a mixed test: ASCII + escape sequences (colors, cursor movement) to measure realistic throughput
- [ ] Compare with Alacritty, Ghostty, Windows Terminal using the same test file

### Rendering Benchmark

- [ ] Full-screen colored text: measure FPS with `criterion` or manual frame timing
- [ ] Target: sustained 60fps with full screen of colored text and attributes
- [ ] Rapidly scrolling output (`yes | head -100000`): measure frame drops and rendering latency
- [ ] Stress test: 256-color gradient filling the entire screen (every cell has a unique color)
- [ ] Measure: instance buffer build time, GPU submit time, present time (separate each phase)

### Memory Benchmark

- [ ] Memory usage with 10,000 lines of scrollback — measure resident set size (RSS)
  - [ ] Expected: ~28.8 MB for the grid (24 bytes x 120 cols x 10K rows)
- [ ] Memory usage with 100,000 lines of scrollback
  - [ ] Expected: ~288 MB — document whether this is acceptable or if compression is needed
- [ ] Memory per tab (baseline overhead excluding scrollback)
  - [ ] Measure with an empty tab (0 lines of output)
- [ ] Memory growth over time: run terminal for 10 minutes with periodic output, measure RSS at intervals
  - [ ] Detect leaks: RSS should stabilize once scrollback is full (ring buffer evicts old rows)

### Latency Benchmark

- [ ] Keypress to screen update latency
- [ ] Target: <5ms from `KeyboardInput` event to `frame.present()` call (perceived instant)
- [ ] Measurement approach:
  - [ ] Instrument: timestamp at `KeyboardInput` event receipt, timestamp at `frame.present()` call
  - [ ] Log the delta for each keypress over a typing session
  - [ ] Report: p50, p95, p99 latencies
- [ ] External measurement: use `typometer` or equivalent tool for end-to-end latency (includes display lag)

### Regression Testing

- [ ] Criterion-based microbenchmarks for:
  - [ ] `grid.put_char()` — single character write
  - [ ] `grid.scroll_up()` — scroll one line
  - [ ] `grid.erase_display(All)` — full screen clear
  - [ ] `vte::Processor::advance()` — parse 64KB buffer
  - [ ] `build_grid_instances()` — full instance buffer build
- [ ] Run on CI, compare against stored baseline
- [ ] Alert on >10% regression (fail the CI build or post a warning)
- [ ] Store baseline results in the repository (JSON or criterion's built-in storage)

- [ ] **Tests:**
  - [ ] All benchmarks compile and run without error
  - [ ] Throughput benchmark completes within a reasonable time (not hung)
  - [ ] Memory benchmark reports stable RSS after scrollback fills

---

## 23.6 Section Completion

- [ ] All 23.1-23.5 items complete
- [ ] `cat` of 100MB file completes without noticeable lag
- [ ] 60fps maintained with full screen of colored text
- [ ] Scrollback uses ring buffer — no O(n) operations
- [ ] Memory usage bounded by scrollback limit (no unbounded growth)
- [ ] Damage tracking reduces unnecessary rendering (idle terminal uses near-zero CPU)
- [ ] Frame pacing prevents excessive CPU/GPU usage when idle
- [ ] Keypress latency under 5ms (p95)
- [ ] No visible jank during rapid output (`yes | head -100000`)
- [ ] Benchmarks established, documented, and running on CI
- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo bench` — all benchmarks run without error

**Exit Criteria:** Terminal handles heavy workloads (large file output, rapid scrolling, complex TUIs) smoothly at 60fps with bounded memory usage. Performance is measured, baselined, and regression-tested.
