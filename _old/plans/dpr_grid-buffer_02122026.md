---
plan: "dpr_grid-buffer_02122026"
title: "Design Pattern Review: Grid Buffer"
status: draft
---

# Design Pattern Review: Grid Buffer

## ori_term Today

ori_term's grid buffer lives in `src/grid/mod.rs` as the `Grid` struct, with supporting types `Row` (`src/grid/row.rs`), `Cell` (`src/cell.rs`), and `Cursor` (`src/grid/cursor.rs`). The architecture follows Alacritty's proven dual-grid model: `Tab` (`src/tab.rs`) owns a `primary_grid` (with `VecDeque<Row>` scrollback, default 10,000 rows) and an `alt_grid` (full-screen apps, no scrollback), switched via DECSET 1049. Viewport rows are a `Vec<Row>` of fixed size `lines`. The `display_offset` field maps the user's scroll position by blending scrollback and viewport through `visible_row(line)`, while `absolute_row(abs_idx)` provides unified addressing for selection and search. Scrollback eviction increments `total_evicted` and adjusts `display_offset` to prevent viewport drift -- a detail that Alacritty and WezTerm also handle, confirming this is the right approach.

The cell representation is compact and well-designed: 24 bytes per cell (`char(4) + Color(4) + Color(4) + CellFlags(u16) + padding(2) + Option<Arc<CellExtra>>(8)`). The `CellExtra` out-of-line pattern keeps combining marks, hyperlinks, and custom underline colors off the hot path. Wide character handling uses explicit `WIDE_CHAR` + `WIDE_CHAR_SPACER` flags with `LEADING_WIDE_CHAR_SPACER` for boundary padding, matching Alacritty's approach. Soft-wrap tracking via `WRAPLINE` on the rightmost cell enables lossless reflow through cell-by-cell iteration (`reflow_cols`), which is Ghostty's pattern.

The gaps are concrete and measurable. First, **no row-level dirty tracking**: `build_grid_instances()` in `src/gpu/render_grid.rs` iterates every cell in every visible row every frame, building GPU instances even when only the cursor blinked. The `grid_dirty` flag on `Tab` is all-or-nothing -- a single character of PTY output forces a full grid rebuild. Second, **no unified row iterator**: selection (`src/selection.rs`) and search (`src/search.rs`) both manually compute `abs_row = scrollback.len() - display_offset + line` and call `absolute_row()` in separate loops, duplicating the scrollback-to-viewport boundary logic. Third, **scroll-up is O(lines)**: `scroll_up_in_region()` calls `self.rows.remove(top)` which is a memmove of `(lines - top)` Row pointers, then `self.rows.insert(bottom, ...)` which is another memmove. For a 50-line terminal this is negligible; for scroll regions or large viewports it adds up. Fourth, **`content_len()` is O(cols)**: every call scans from the rightmost column backward, and `reflow_cols()` calls it for every non-wrapped row, making reflow O(rows * cols).

## Prior Art

### Alacritty -- Ring Buffer with Occupied Tracking

Alacritty stores all rows (scrollback + visible) in a single contiguous `Vec<Row<T>>` with a `zero` offset for wrap-around indexing. Scroll operations rotate `zero` instead of moving memory, making `scroll_up` O(1) -- just clear a row and bump the offset. The `Storage` type implements `Index`/`IndexMut` with modular arithmetic to present a logical view where row 0 is always the top of the viewport.

The key insight is that ring buffer indexing converts scrollback management from a data movement problem (memmove N pointers) to an arithmetic problem (increment an offset). Alacritty's `occ` field on each row tracks the rightmost occupied column, so blank-row trimming during resize is O(1) per row rather than O(cols). The tradeoff is that serialization and iteration across the wrap boundary require careful index normalization, but for a terminal emulator where the primary access pattern is sequential line-by-line rendering, this is well worth it.

### Ghostty -- Page-Level Dirty Tracking and Style Interning

Ghostty organizes its grid into fixed-size `Page` nodes in a doubly-linked list. Each page is a contiguous memory block containing rows, cells, and metadata as a self-contained arena. The design enables two features that matter for ori_term: **page-level dirty flags** that let the renderer skip unchanged regions, and **per-page style interning** where a `StyleSet` maps unique attribute combinations to small IDs, storing a style ID per cell instead of full fg/bg/flags.

For ori_term, the page-linked-list structure itself is overkill (we don't need mmap for 10K-row scrollback on a Windows desktop app), but the dirty tracking and style interning patterns are directly applicable. A dirty flag per row (or per row-group) would let `build_grid_instances()` skip rebuilding instances for unchanged rows. Style interning could reduce per-cell memory from 24 bytes to ~12 bytes (char + style_id + extra_ptr), but this trades memory for indirection cost and adds complexity to the VTE input path. Worth measuring, not worth assuming.

### WezTerm -- Stable Row Identity and Compression

WezTerm maintains a `stable_row_index_offset` that monotonically increases as rows are evicted from scrollback, providing a persistent identity for every row that has ever existed. Selection anchors and search results store `StableRowIndex` values that remain valid across scrollback eviction, scroll, and resize -- unlike raw absolute indices which shift when scrollback wraps.

WezTerm also stores lines as `Vec<CellCluster>` where consecutive cells with identical attributes are grouped. This trades cell-level random access (O(clusters) instead of O(1)) for efficient text shaping (shape per cluster, not per cell) and memory savings on attribute-heavy output. For ori_term's GPU renderer, which needs per-cell random access for instance building, clustering would add overhead rather than save it. But stable row indexing is directly useful: ori_term's current `total_evicted`-based absolute addressing achieves the same goal but the conversion arithmetic is scattered across `render_grid.rs`, `selection.rs`, `search.rs`, and `url_detect.rs` instead of being encapsulated in one place.

## Proposed Best-of-Breed Design

### Core Idea

Keep ori_term's existing `Vec<Row>` viewport + `VecDeque<Row>` scrollback architecture, which is proven and straightforward. Layer three targeted improvements on top: (1) **row-level dirty tracking** inspired by Ghostty's page-dirty flags, adapted to ori_term's flat row storage; (2) **stable row identity** inspired by WezTerm's `StableRowIndex`, encapsulated as a newtype to replace scattered `total_evicted` arithmetic; (3) **O(1) viewport scroll** inspired by Alacritty's ring buffer, applied only to the viewport `Vec<Row>` via a `zero` offset to eliminate memmove in `scroll_up_in_region`.

This is not a rewrite. Each improvement is independently implementable and testable, touches a different subsystem, and produces measurable results. The Cell representation stays at 24 bytes. The scrollback stays as `VecDeque<Row>`. The GPU rendering pipeline stays unchanged except for using dirty flags to skip unchanged rows.

### Key Design Choices

1. **Row-level dirty bitset (Ghostty)**: Add a `BitVec` (or `Vec<bool>` for simplicity) to `Grid` sized to `lines`, where `dirty[line] = true` means that row's GPU instances need rebuilding. `put_char`, `put_wide_char`, `erase_*`, `scroll_up/down`, and `resize` set the relevant bits. `build_grid_instances()` only processes dirty rows, copying cached instances for clean rows. This replaces the all-or-nothing `grid_dirty` flag on `Tab`. For a typical terminal session where output arrives on 1-3 rows per frame, this skips 90%+ of instance building. Ghostty proved this works at page granularity; row granularity is finer-grained and better suited to ori_term's flat storage.

2. **StableRowIndex newtype (WezTerm)**: Replace bare `usize` absolute row indices in `SelectionPoint`, `SearchMatch`, and URL detection with `StableRowIndex(u64)`, a monotonically increasing counter. `Grid` tracks the current base offset (equivalent to `total_evicted + scrollback.len()` but computed once). All consumers convert to/from `StableRowIndex` at the boundary. This eliminates the repeated `scrollback.len() - display_offset + line` arithmetic in `render_grid.rs` (line 53), `selection.rs` (everywhere), and `search.rs` (line 153), and makes row identity survive scrollback eviction without manual offset adjustments.

3. **Viewport ring buffer (Alacritty)**: Replace `Vec<Row>` for the viewport with a fixed-capacity ring buffer indexed via a `zero` offset. `scroll_up_in_region` with `top=0` becomes O(1): clear the row at `zero`, push it to scrollback, advance `zero`. This eliminates the `self.rows.remove(top)` memmove (currently O(lines) per scroll). The scrollback remains `VecDeque<Row>` (its push/pop are already O(1)). For scroll regions (`top > 0`), the ring buffer doesn't help, but whole-screen scroll is the dominant case.

4. **Cached row content hash for search invalidation (novel)**: Store a lightweight hash (e.g., `u64` via `ahash`) per row that summarizes its text content. When `SearchState::update_query` runs, only re-extract text from rows whose hash changed since the last search. For a 10,000-row scrollback, this reduces search update cost from O(total_rows * cols) to O(changed_rows * cols). This is unique to ori_term -- reference emulators don't cache search state across frames because they don't have a persistent search UI.

5. **Occupied column tracking improvement (Alacritty)**: The existing `occ` field on `Row` is already Alacritty's pattern. Extend it by maintaining `occ` atomically during all mutation paths (currently `put_char` and `put_wide_char` update it, but `erase_chars` and `delete_chars` don't recalculate it). Make `content_len()` return `occ` in O(1) instead of scanning right-to-left in O(cols). This directly benefits `reflow_cols()` which calls `content_len()` on every non-wrapped row.

### What Makes ori_term's Approach Unique

ori_term's wgpu renderer creates GPU instance data (80-byte records) on the CPU and uploads them via `queue.write_buffer`. This is fundamentally different from Alacritty (OpenGL with vertex arrays built per-frame), Ghostty (Metal/OpenGL with SIMD-accelerated cell processing), and WezTerm (OpenGL with texture-based cell rendering). The instance-based approach means that **caching instances per row is natural**: each row produces a contiguous slice of instance bytes that can be stored and replayed. When a row's dirty flag is false, its cached instance bytes are memcpy'd into the frame buffer without re-processing any cells. This is something none of the reference emulators can do because their rendering pipelines don't produce cacheable per-row output.

ori_term's single-process, multi-tab architecture with ConPTY also means that scrollback pressure is multiplicative: 10 tabs * 10,000 rows * 80 columns * 24 bytes/cell = 192 MB. The reference emulators either run one process per window (Alacritty, Ghostty) or use a multiplexer model with compressed inactive tabs (WezTerm). For ori_term, the right response is not to compress (which adds latency to tab switching) but to ensure that inactive tab scrollback is never touched by the render loop. The dirty tracking system naturally achieves this: only the active tab's grid is processed, and even then only the dirty rows.

The frameless window with custom tab bar means that ori_term's renderer has to rebuild tab bar instances alongside grid instances. The `grid_dirty` / `tab_bar_dirty` split already exists in `FrameParams`; row-level dirty tracking extends this to sub-grid granularity without changing the overall frame preparation flow in `GpuRenderer::prepare_frame()`.

### Concrete Types & Interfaces

```rust
// --- src/grid/stable_index.rs ---

/// Monotonically increasing row identity that survives scrollback eviction.
/// Row 0 is the first row ever written to this grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableRowIndex(pub u64);

impl StableRowIndex {
    /// Convert a viewport line to a stable index.
    pub fn from_viewport(grid: &Grid, line: usize) -> Self {
        StableRowIndex(grid.stable_base + line as u64)
    }

    /// Convert a visible line (accounting for display_offset) to a stable index.
    pub fn from_visible(grid: &Grid, line: usize) -> Self {
        let abs = grid.scrollback.len().saturating_sub(grid.display_offset) + line;
        StableRowIndex(grid.total_evicted as u64 + abs as u64)
    }

    /// Convert back to an absolute row index (scrollback + viewport).
    /// Returns None if the row has been evicted from scrollback.
    pub fn to_absolute(self, grid: &Grid) -> Option<usize> {
        let evicted = grid.total_evicted as u64;
        if self.0 < evicted {
            return None; // Evicted
        }
        let abs = (self.0 - evicted) as usize;
        let total = grid.scrollback.len() + grid.lines;
        if abs < total { Some(abs) } else { None }
    }
}
```

```rust
// --- src/grid/ring.rs ---

/// Fixed-capacity ring buffer for viewport rows.
/// Provides O(1) rotation for scroll operations.
pub struct ViewportRing {
    rows: Vec<Row>,
    zero: usize,   // Logical row 0 is at this physical index
    len: usize,    // Always == capacity (viewport is always full)
}

impl ViewportRing {
    pub fn new(cols: usize, lines: usize) -> Self {
        let rows = (0..lines).map(|_| Row::new(cols)).collect();
        Self { rows, zero: 0, len: lines }
    }

    /// Logical index to physical index.
    fn physical(&self, logical: usize) -> usize {
        (self.zero + logical) % self.len
    }

    /// Rotate the ring: logical row 0 becomes the next row.
    /// Returns a mutable reference to the old row 0 (now logically at bottom)
    /// so the caller can push it to scrollback before clearing it.
    pub fn rotate_up(&mut self) -> &mut Row {
        let old_zero = self.zero;
        self.zero = (self.zero + 1) % self.len;
        &mut self.rows[old_zero]
    }

    pub fn row(&self, line: usize) -> &Row {
        &self.rows[self.physical(line)]
    }

    pub fn row_mut(&mut self, line: usize) -> &mut Row {
        let idx = self.physical(line);
        &mut self.rows[idx]
    }

    /// Iterator over logical rows 0..len.
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        (0..self.len).map(move |i| &self.rows[self.physical(i)])
    }
}

impl Index<usize> for ViewportRing {
    type Output = Row;
    fn index(&self, idx: usize) -> &Row {
        self.row(idx)
    }
}

impl IndexMut<usize> for ViewportRing {
    fn index_mut(&mut self, idx: usize) -> &mut Row {
        self.row_mut(idx)
    }
}
```

```rust
// --- src/grid/dirty.rs ---

/// Row-level dirty tracking for GPU instance caching.
pub struct DirtyTracker {
    /// Per-row dirty flag. True = needs GPU instance rebuild.
    bits: Vec<bool>,
    /// Cached GPU instance data per row (bg + fg byte slices).
    /// None = never built, Some = cached from previous frame.
    row_cache: Vec<Option<CachedRowInstances>>,
    /// True if any structural change occurred (resize, scroll, reflow).
    /// Forces full rebuild regardless of per-row flags.
    structural: bool,
}

/// Cached instance bytes for a single row's bg and fg passes.
struct CachedRowInstances {
    bg_bytes: Vec<u8>,
    fg_bytes: Vec<u8>,
}

impl DirtyTracker {
    pub fn new(lines: usize) -> Self {
        Self {
            bits: vec![true; lines],
            row_cache: vec![None; lines],
            structural: true,
        }
    }

    /// Mark a single row as dirty.
    pub fn mark_row(&mut self, line: usize) {
        if let Some(b) = self.bits.get_mut(line) {
            *b = true;
        }
    }

    /// Mark all rows as dirty (scroll, resize, theme change).
    pub fn mark_all(&mut self) {
        self.bits.fill(true);
        self.structural = true;
    }

    /// Mark a range of rows as dirty (scroll region operations).
    pub fn mark_range(&mut self, start: usize, end: usize) {
        for b in &mut self.bits[start..=end.min(self.bits.len().saturating_sub(1))] {
            *b = true;
        }
    }

    /// Check if a row needs rebuild.
    pub fn is_dirty(&self, line: usize) -> bool {
        self.structural || self.bits.get(line).copied().unwrap_or(true)
    }

    /// Clear dirty flags after a frame is built.
    pub fn clear(&mut self) {
        self.bits.fill(false);
        self.structural = false;
    }

    /// Resize tracking to match new viewport dimensions.
    pub fn resize(&mut self, new_lines: usize) {
        self.bits.resize(new_lines, true);
        self.row_cache.resize_with(new_lines, || None);
        self.structural = true;
    }

    /// Store cached instance data for a row after building it.
    pub fn cache_row(&mut self, line: usize, bg: &[u8], fg: &[u8]) {
        if let Some(entry) = self.row_cache.get_mut(line) {
            *entry = Some(CachedRowInstances {
                bg_bytes: bg.to_vec(),
                fg_bytes: fg.to_vec(),
            });
        }
    }

    /// Retrieve cached instance data for a clean row.
    pub fn cached_row(&self, line: usize) -> Option<&CachedRowInstances> {
        self.row_cache.get(line).and_then(|e| e.as_ref())
    }
}
```

```rust
// --- Updated Grid struct (src/grid/mod.rs) ---

pub struct Grid {
    viewport: ViewportRing,      // Was: rows: Vec<Row>
    pub cols: usize,
    pub lines: usize,
    pub cursor: Cursor,
    pub saved_cursor: Option<Cursor>,
    scroll_top: usize,
    scroll_bottom: usize,
    pub tab_stops: Vec<bool>,
    pub scrollback: VecDeque<Row>,
    pub max_scrollback: usize,
    pub display_offset: usize,
    pub total_evicted: usize,
    /// Stable base: total_evicted + scrollback.len(). Updated on scroll/evict.
    pub stable_base: u64,
    /// Row-level dirty tracking for incremental GPU updates.
    pub dirty: DirtyTracker,
}

impl Grid {
    /// O(1) full-screen scroll up: rotate ring, push old row to scrollback.
    pub fn scroll_up(&mut self, count: usize) {
        if self.scroll_top == 0 && self.scroll_bottom == self.lines.saturating_sub(1) {
            // Full-screen scroll: use ring rotation (O(1) per row).
            for _ in 0..count {
                // Rotate gives us the old top row (now at logical bottom).
                let old_top = self.viewport.rotate_up();
                let scrolled = std::mem::replace(old_top, Row::new(self.cols));
                // Push to scrollback.
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                    self.total_evicted += 1;
                    if self.display_offset > 0 {
                        self.display_offset = self.display_offset.saturating_sub(1);
                    }
                } else if self.display_offset > 0 {
                    self.display_offset += 1;
                }
                self.scrollback.push_back(scrolled);
                self.stable_base = self.total_evicted as u64 + self.scrollback.len() as u64;
            }
            self.dirty.mark_all(); // All visible rows shifted.
        } else {
            // Scroll region: fall back to remove/insert (O(region_size)).
            self.scroll_up_in_region(self.scroll_top, self.scroll_bottom, count);
        }
    }
}
```

```rust
// --- Updated SelectionPoint (src/selection.rs) ---

/// A point in stable grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: StableRowIndex,  // Was: usize
    pub col: usize,
    pub side: Side,
}

// All selection operations use StableRowIndex, eliminating manual
// total_evicted arithmetic at every call site.
```

```rust
// --- Updated build_grid_instances (src/gpu/render_grid.rs) ---

impl GpuRenderer {
    pub(super) fn build_grid_instances(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
        default_bg: &[f32; 4],
    ) {
        let grid = params.grid;
        // ... setup code unchanged ...

        for line in 0..grid.lines {
            if !grid.dirty.is_dirty(line) {
                // Row unchanged: copy cached instance data.
                if let Some(cached) = grid.dirty.cached_row(line) {
                    bg.extend_raw(&cached.bg_bytes);
                    fg.extend_raw(&cached.fg_bytes);
                    continue;
                }
            }

            // Build instances for this row (existing cell loop).
            let row_bg_start = bg.len();
            let row_fg_start = fg.len();
            // ... existing per-cell logic ...

            // Cache the built instances for next frame.
            grid.dirty.cache_row(
                line,
                &bg.as_bytes()[row_bg_start..],
                &fg.as_bytes()[row_fg_start..],
            );
        }
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation

- [ ] Add `DirtyTracker` struct in `src/grid/dirty.rs` with per-row `bool` vector, `mark_row`, `mark_all`, `mark_range`, `clear`, and `resize` methods.
- [ ] Integrate `DirtyTracker` into `Grid`: add `dirty` field, call `mark_row` from `put_char`, `put_wide_char`, `erase_line`, `erase_chars`, `insert_blank_chars`, `delete_chars`.
- [ ] Call `dirty.mark_all()` from `scroll_up`, `scroll_down`, `resize`, `clear_all`, `erase_display(All/Saved)`, `reflow_cols`.
- [ ] Call `dirty.mark_range()` from `scroll_up_in_region` and `scroll_down_in_region` (only the affected region).
- [ ] Add `dirty.resize()` call to `Grid::resize_rows()`.
- [ ] Update `build_grid_instances()` to check `grid.dirty.is_dirty(line)` and skip clean rows (no caching yet, just the skip path).
- [ ] Replace `Tab::grid_dirty` boolean with `grid.dirty.is_dirty_any()` check, or keep both (Tab-level for tab bar, Grid-level for row granularity).
- [ ] Fix `Row::occ` maintenance: update `occ` correctly in `erase_chars`, `delete_chars`, `erase_line`, `reset` so that `content_len()` can be replaced with `occ` for the common case.

### Phase 2: Core

- [ ] Add `StableRowIndex` newtype in `src/grid/stable_index.rs` with `from_viewport`, `from_visible`, `to_absolute` conversions.
- [ ] Add `stable_base: u64` field to `Grid`, updated in `scroll_up`, `scroll_down`, and scrollback eviction.
- [ ] Migrate `SelectionPoint::row` from `usize` to `StableRowIndex`; update `selection.rs` (contains, ordered, extract_text, word_boundaries, logical_line_start/end).
- [ ] Migrate `SearchMatch::start_row`/`end_row` from `usize` to `StableRowIndex`; update `search.rs` (find_matches, cell_match_type).
- [ ] Update `render_grid.rs` to compute `StableRowIndex` once per line and pass it to selection/search checks, replacing the inline `abs_row` arithmetic.
- [ ] Update `url_detect.rs` to use `StableRowIndex` for `DetectedUrl` segments.
- [ ] Add row instance caching to `DirtyTracker`: `cache_row()` / `cached_row()` methods, `extend_raw()` on `InstanceWriter` to replay cached bytes.
- [ ] Update `build_grid_instances()` to cache and replay per-row instance data for clean rows.

### Phase 3: Polish

- [ ] Implement `ViewportRing` in `src/grid/ring.rs` with `rotate_up`, `Index`/`IndexMut`, and `iter`.
- [ ] Replace `Grid::rows: Vec<Row>` with `Grid::viewport: ViewportRing`.
- [ ] Update all `self.rows[line]` accesses in `grid/mod.rs` to `self.viewport[line]`.
- [ ] Optimize `scroll_up` for the full-screen case: use `viewport.rotate_up()` for O(1) instead of `rows.remove(0)` + `rows.insert(bottom)`.
- [ ] Keep `scroll_up_in_region` with `top > 0` as the fallback path (ring rotation only helps when scrolling the entire viewport).
- [ ] Benchmark: measure frame time with dirty tracking vs. without on a `seq 100000` workload. Target: < 0.5ms for cursor-blink-only frames (currently rebuilds full grid).
- [ ] Benchmark: measure `scroll_up` latency with ring buffer vs. Vec remove/insert on a 200-line terminal.
- [ ] Add `content_hash: u64` field to `Row` for incremental search invalidation; update hash on any cell mutation. Benchmark search re-query time on 10K-row scrollback.

## References

- `src/grid/mod.rs` -- Grid struct, scroll operations, resize/reflow, put_char/put_wide_char
- `src/grid/row.rs` -- Row struct, occ tracking, content_len(), resize/truncate/grow
- `src/grid/cursor.rs` -- Cursor struct, template Cell
- `src/cell.rs` -- Cell (24 bytes), CellFlags, CellExtra, Arc pattern
- `src/tab.rs` -- Tab struct (dual grid, grid_dirty flag), process_output()
- `src/gpu/render_grid.rs` -- build_grid_instances(), per-cell rendering loop, abs_row computation
- `src/gpu/renderer.rs` -- GpuRenderer, FrameParams, PreparedFrame, InstanceWriter, prepare_frame()
- `src/selection.rs` -- SelectionPoint, Selection, word_boundaries, extract_text
- `src/search.rs` -- SearchState, SearchMatch, find_matches, cell_match_type
- `src/url_detect.rs` -- DetectedUrl, UrlDetectCache
- `~/projects/reference_repos/console_repos/alacritty` -- Ring buffer storage, occ tracking
- `~/projects/reference_repos/console_repos/ghostty` -- Page-level dirty flags, style interning
- `~/projects/reference_repos/console_repos/wezterm` -- StableRowIndex, CellCluster, compression
