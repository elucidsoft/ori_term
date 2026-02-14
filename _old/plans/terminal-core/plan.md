# Plan: Terminal Core Improvements

## Completed Work

### VecDeque Scrollback Buffer

**Status:** Complete

**Context:**
The scrollback buffer in `src/grid/mod.rs` used `Vec<Row>`. Every time the buffer
was full and a new line scrolled in, `self.scrollback.remove(0)` shifted all elements
left — O(n) with max_scrollback=10,000. Replacing with `VecDeque` gives O(1)
`pop_front()` and `push_back()`.

**Changes (single file: `src/grid/mod.rs`):**
- Added `use std::collections::VecDeque;`
- Changed field type: `pub scrollback: Vec<Row>` → `pub scrollback: VecDeque<Row>`
- Constructor: `Vec::new()` → `VecDeque::new()`
- `scroll_up_in_region`: `remove(0)` → `pop_front()`, `push()` → `push_back()`
- `resize_rows` shrink: `remove(0)` → `pop_front()`, `push()` → `push_back()`
- `resize_rows` grow: `pop()` → `pop_back()`
- `drain(..).collect()`, `clear()`, indexing, `len()`, iteration — unchanged (VecDeque compatible)

**Verification:**
- `cargo build` — clean
- `cargo clippy -- -D warnings` — clean
- `cargo test` — 152/152 pass

---

### Implicit URL Detection for Clickable Links

**Status:** Complete

**Context:**
Plain-text URLs in terminal output (from git, cargo, etc.) were not underlined or
clickable. OSC 8 hyperlink support existed but most programs don't emit OSC 8
sequences. Automatic URL detection was needed to make visible URLs interactive —
matching iTerm2, WezTerm, Kitty behavior.

**Approach:**
Detect URLs lazily on Ctrl+hover/click (not every frame). Concatenate soft-wrapped
rows into logical lines, run regex, map spans back to per-row segments. Cache results
per logical line. Render solid underline on hovered URL segments across all rows.

**Changes:**

`src/url_detect.rs` (new):
- `DetectedUrl` struct with `segments: Vec<(abs_row, start_col, end_col)>` and `url`
- `UrlDetectCache`: keyed by logical line start row, lazy computation, invalidation
- `detect_urls_in_logical_line()`: concatenates wrapped rows, runs regex, maps byte
  spans back to per-row segments via char-to-position mapping
- `trim_url_trailing()`: strips trailing punctuation preserving balanced parentheses
- URL regex: `(?:https?|ftp|file)://[^\s<>\[\]'"]+` with post-processing
- Skips spans where any cell has an existing OSC 8 hyperlink
- 6 unit tests including wrapped URL detection

`src/search.rs`:
- Made `extract_row_text` and `byte_span_to_cols` `pub(crate)`

`src/lib.rs`:
- Added `pub mod url_detect;`

`src/app.rs`:
- Added `url_cache: UrlDetectCache` and `hover_url_range: Option<Vec<UrlSegment>>`
- `handle_cursor_moved`: falls through from OSC 8 to implicit URL detection via cache
- `handle_grid_press`: Ctrl+click checks implicit URLs after OSC 8
- Cache invalidation after PTY output
- `FrameParams` construction passes `hover_url_range` as slice

`src/gpu/renderer.rs`:
- `FrameParams`: `hover_url_range: Option<&[(usize, usize, usize)]>`
- `build_grid_instances`: renders solid underline for hovered implicit URL segments,
  checking all segments (supports multi-row URLs)

**Verification:**
- Build: `cargo build --target x86_64-pc-windows-gnu --release` — pass
- Clippy: `cargo clippy --target x86_64-pc-windows-gnu` — clean
- Test: `cargo test` — 131/131 pass
- Manual: URLs in terminal output show pointer cursor + solid underline on Ctrl+hover,
  open in browser on Ctrl+click, including URLs that wrap across rows
