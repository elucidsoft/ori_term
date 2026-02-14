---
section: 14
title: URL Detection
status: not-started
tier: 3
goal: Detect URLs in terminal output for hover underline and Ctrl+click opening
sections:
  - id: "14.1"
    title: URL Detection Engine
    status: not-started
  - id: "14.2"
    title: URL Cache
    status: not-started
  - id: "14.3"
    title: Hover & Click Handling
    status: not-started
  - id: "14.4"
    title: Section Completion
    status: not-started
---

# Section 14: URL Detection

**Status:** Not Started
**Goal:** Detect URLs in terminal output using regex, provide visual hover feedback (underline + pointer cursor), and open URLs in the system browser on Ctrl+click. Handles soft-wrapped lines, balanced parentheses (Wikipedia-style URLs), and coexists with explicit OSC 8 hyperlinks.

**Crate:** `oriterm` (binary)
**Dependencies:** `regex`, `std::sync::LazyLock`
**Reference:** `_old/src/url_detect.rs`, `_old/src/app/cursor_hover.rs`

**Prerequisite:** Section 01 (Grid), Section 09 (search text extraction — shared `extract_row_text`)

---

## 14.1 URL Detection Engine

Regex-based URL detection across logical lines (sequences of soft-wrapped rows).

**File:** `oriterm/src/url_detect.rs`

**Reference:** `_old/src/url_detect.rs`

- [ ] `UrlSegment` type alias — `(usize, usize, usize)` = `(abs_row, start_col, end_col)` inclusive
- [ ] `DetectedUrl` struct
  - [ ] Fields:
    - `segments: Vec<UrlSegment>` — per-row segments (handles URLs wrapped across rows)
    - `url: String` — the extracted URL string
  - [ ] `DetectedUrl::contains(&self, abs_row: usize, col: usize) -> bool`
    - [ ] Check if any segment covers the given position
  - [ ] Derive: `Debug`, `Clone`
- [ ] URL regex pattern (static `LazyLock<Regex>`):
  - [ ] `(?:https?|ftp|file)://[^\s<>\[\]'"]+`
  - [ ] Covers: http, https, ftp, file schemes
  - [ ] Stops at whitespace, angle brackets, square brackets, quotes
- [ ] `trim_url_trailing(url: &str) -> &str`
  - [ ] Strip trailing punctuation: `.`, `,`, `;`, `:`, `!`, `?`
  - [ ] Handle balanced parentheses: only strip trailing `)` if unbalanced
    - [ ] Count `(` and `)` in URL
    - [ ] If `close > open`: strip one trailing `)`
    - [ ] Repeat until stable
  - [ ] Preserves Wikipedia-style URLs: `https://en.wikipedia.org/wiki/Rust_(language)`
- [ ] `detect_urls_in_logical_line(grid: &Grid, line_start: usize, line_end: usize) -> Vec<DetectedUrl>`
  - [ ] Concatenate text from all rows in logical line using `extract_row_text`
  - [ ] Build `char_to_pos: Vec<(usize, usize)>` mapping char index to `(abs_row, col)`
  - [ ] Run regex on concatenated text
  - [ ] For each match:
    - [ ] Trim trailing punctuation
    - [ ] Skip URLs shorter than scheme prefix (e.g., bare "https://")
    - [ ] Convert byte offsets to char offsets
    - [ ] Skip if any cell in span has an OSC 8 hyperlink (explicit hyperlinks take precedence)
    - [ ] Build per-row segments from `char_to_pos` mapping
    - [ ] Emit `DetectedUrl` with segments and URL string
- [ ] `logical_line_start(grid: &Grid, abs_row: usize) -> usize` — walk backwards to find first row of logical line
- [ ] `logical_line_end(grid: &Grid, abs_row: usize) -> usize` — walk forwards to find last row of logical line

---

## 14.2 URL Cache

Lazy per-logical-line URL detection cache. Avoids redundant regex matching on every mouse move.

**File:** `oriterm/src/url_detect.rs` (continued)

**Reference:** `_old/src/url_detect.rs` (UrlDetectCache)

- [ ] `UrlDetectCache` struct
  - [ ] Fields:
    - `lines: HashMap<usize, Vec<DetectedUrl>>` — logical line start row -> detected URLs
    - `row_to_line: HashMap<usize, usize>` — any row -> its logical line start (fast lookup)
  - [ ] `Default` derive for empty initialization
- [ ] `UrlDetectCache::url_at(&mut self, grid: &Grid, abs_row: usize, col: usize) -> Option<DetectedUrl>`
  - [ ] Ensure logical line is computed (lazy)
  - [ ] Search cached URLs for one containing (abs_row, col)
  - [ ] Return cloned `DetectedUrl` if found
- [ ] `UrlDetectCache::ensure_logical_line(&mut self, grid: &Grid, abs_row: usize) -> usize`
  - [ ] If already cached (via `row_to_line`): return cached line start
  - [ ] Otherwise: compute logical line bounds, detect URLs, cache results
  - [ ] Register all rows in the logical line in `row_to_line`
- [ ] `UrlDetectCache::invalidate(&mut self)`
  - [ ] Clear both HashMaps
  - [ ] Called after: PTY output, scroll, resize, font change (anything that changes grid content or layout)
- [ ] Cache is per-tab (stored in Tab or binary-side wrapper)

---

## 14.3 Hover & Click Handling

Visual feedback on URL hover and opening URLs on Ctrl+click.

**File:** `oriterm/src/app/cursor_hover.rs`

**Reference:** `_old/src/app/cursor_hover.rs`, `_old/src/app/hover_url.rs`

- [ ] On mouse move (while Ctrl held):
  - [ ] Convert pixel position to grid cell (abs_row, col)
  - [ ] Query `url_cache.url_at(grid, abs_row, col)`
  - [ ] If URL found:
    - [ ] Store `hovered_url: Option<DetectedUrl>` in app/tab state
    - [ ] Set cursor icon to `CursorIcon::Pointer` (hand cursor)
    - [ ] Underline all cells in the URL's segments (solid underline on hover)
    - [ ] Request redraw
  - [ ] If no URL (or Ctrl not held):
    - [ ] Clear `hovered_url`
    - [ ] Restore cursor icon to default
    - [ ] Remove hover underline
    - [ ] Request redraw if state changed
- [ ] On Ctrl+click (left button):
  - [ ] If `hovered_url` is Some:
    - [ ] Validate URL scheme: only `http`, `https`, `ftp`, `file` allowed
    - [ ] Open URL in system browser:
      - [ ] Windows: `std::process::Command::new("cmd").args(["/C", "start", &url])`
      - [ ] Linux: `xdg-open`
      - [ ] macOS: `open`
    - [ ] Consume the click event (don't pass to terminal/selection)
- [ ] URL hover rendering integration:
  - [ ] During `draw_frame`: check if cell is in `hovered_url` segments
  - [ ] If yes: draw solid underline decoration at cell position
  - [ ] Color: foreground color (matches text above)
- [ ] Interaction with OSC 8 hyperlinks:
  - [ ] Implicit URL detection skips cells that already have explicit OSC 8 hyperlinks
  - [ ] OSC 8 hyperlinks have their own hover/click behavior (section 20)
- [ ] Interaction with mouse reporting:
  - [ ] When terminal has mouse reporting enabled: Ctrl+click still opens URL (Ctrl is override)
  - [ ] Shift+click bypasses mouse reporting per xterm convention

---

## 14.4 Section Completion

- [ ] All 14.1-14.3 items complete
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] Simple URLs detected at correct column ranges (http, https, ftp, file)
- [ ] Multiple URLs on same line detected independently
- [ ] Wikipedia-style parenthesized URLs preserved: `https://en.wikipedia.org/wiki/Rust_(language)`
- [ ] Trailing punctuation stripped: `https://example.com.` detects `https://example.com`
- [ ] Wrapped URLs: URL spanning two rows detected with correct per-row segments
- [ ] OSC 8 hyperlinks not duplicated by implicit detection
- [ ] Ctrl+hover: underline appears, cursor changes to pointer
- [ ] Ctrl+click: URL opens in system browser
- [ ] Cache invalidated on PTY output/scroll/resize (no stale URLs)
- [ ] No URL on plain text: no false positives on words like "https" without "://"
- [ ] **Tests** (`oriterm/src/url_detect.rs` `#[cfg(test)]`):
  - [ ] Detect simple URL at correct columns
  - [ ] Detect multiple URLs on same line
  - [ ] Balanced parentheses preserved
  - [ ] No URLs in plain text
  - [ ] Wrapped URL spans two rows with correct segments
  - [ ] `DetectedUrl::contains` returns correct results for all positions

**Exit Criteria:** Ctrl+hover underlines URLs in terminal output, Ctrl+click opens them in the system browser. Detection handles wrapped lines, parenthesized URLs, and coexists with explicit OSC 8 hyperlinks.
