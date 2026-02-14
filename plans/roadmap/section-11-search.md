---
section: 11
title: Search
status: not-started
tier: 3
goal: Plain text and regex search across terminal grid with search UI overlay and match navigation
sections:
  - id: "11.1"
    title: Search State
    status: not-started
  - id: "11.2"
    title: Search Algorithm
    status: not-started
  - id: "11.3"
    title: Row Text Extraction
    status: not-started
  - id: "11.4"
    title: Search UI
    status: not-started
  - id: "11.5"
    title: Section Completion
    status: not-started
---

# Section 11: Search

**Status:** Not Started
**Goal:** Plain text and regex search across the terminal grid (viewport + scrollback) with a search bar overlay, match highlighting, and keyboard-driven navigation.

**Crate:** `oriterm_core` (search state, algorithm, text extraction), `oriterm` (search UI overlay)
**Dependencies:** `regex` (in `oriterm_core`)
**Reference:** `_old/src/search/` (mod.rs, find.rs, text.rs, tests.rs), `_old/src/app/search_ui.rs`

**Prerequisite:** Section 01 (Grid), Section 02 (VTE/Term — for StableRowIndex)

---

## 11.1 Search State

Core search state: query, matches, focused index, navigation. Lives in `oriterm_core` so the library owns search logic independent of rendering.

**File:** `oriterm_core/src/search/mod.rs`

**Reference:** `_old/src/search/mod.rs`

- [ ] `MatchType` enum — `None`, `Match`, `FocusedMatch`
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
  - [ ] Used by rendering to decide per-cell highlight color
- [ ] `SearchMatch` struct — a single match span in stable grid coordinates
  - [ ] Fields:
    - `start_row: StableRowIndex` — stable row of match start
    - `start_col: usize` — column of match start
    - `end_row: StableRowIndex` — stable row of match end (same row for single-line matches)
    - `end_col: usize` — column of match end (inclusive)
  - [ ] Derive: `Debug`, `Clone`
- [ ] `SearchState` struct
  - [ ] Fields:
    - `query: String` — current search query text
    - `matches: Vec<SearchMatch>` — all matches, sorted by position (earliest first)
    - `focused: usize` — index of currently focused match
    - `case_sensitive: bool` — case sensitivity toggle
    - `use_regex: bool` — regex mode toggle
  - [ ] `SearchState::new() -> Self` — empty defaults
  - [ ] `SearchState::next_match(&mut self)` — advance to next match, wrapping around
  - [ ] `SearchState::prev_match(&mut self)` — go to previous match, wrapping around
  - [ ] `SearchState::update_query(&mut self, grid: &Grid)` — re-run search with current query settings
    - [ ] Empty query: clear matches, reset focused to 0
    - [ ] Non-empty: call `find::find_matches()`, clamp focused index
  - [ ] `SearchState::focused_match(&self) -> Option<&SearchMatch>` — currently focused match
  - [ ] `SearchState::cell_match_type(&self, stable_row: StableRowIndex, col: usize) -> MatchType`
    - [ ] Binary search via `partition_point` for O(log n) lookup
    - [ ] Check small window around found index (handles edge cases)
    - [ ] Returns `FocusedMatch` for the focused match, `Match` for others, `None` outside matches
- [ ] `cell_in_match(m: &SearchMatch, stable_row: StableRowIndex, col: usize) -> bool`
  - [ ] Private helper: checks if (stable_row, col) falls within a match span
  - [ ] Handles single-row matches, multi-row matches, and middle rows
- [ ] Re-export `SearchState`, `SearchMatch`, `MatchType` from `lib.rs`

---

## 11.2 Search Algorithm

Find all matches in the grid for a given query, supporting both plain text and regex.

**File:** `oriterm_core/src/search/find.rs`

**Reference:** `_old/src/search/find.rs`

- [ ] `find_matches(grid: &Grid, query: &str, case_sensitive: bool, use_regex: bool) -> Vec<SearchMatch>`
  - [ ] Returns matches sorted by position (earliest first)
- [ ] Regex path:
  - [ ] Build `regex::RegexBuilder` with case_insensitive flag
  - [ ] Invalid regex: return empty vec (no crash)
  - [ ] Search row by row (multi-row regex deferred)
  - [ ] For each row: extract text, get `StableRowIndex`, run `find_iter`, map byte spans to columns
- [ ] Plain text path:
  - [ ] Case-insensitive: lowercase both query and haystack
  - [ ] Sliding search: `haystack[start..].find(query)`, advance past each match
  - [ ] Map byte spans to column spans via `byte_span_to_cols`
- [ ] Iteration scope: all rows = `scrollback.len() + grid.lines` (full history + viewport)
- [ ] Each match: `SearchMatch { start_row, start_col, end_row, end_col }`
- [ ] **Tests** (`oriterm_core/src/search/tests.rs`):
  - [ ] Plain text: "hello" found at correct columns in two rows
  - [ ] Case insensitive: "Hello", "HELLO", "hello" all found
  - [ ] Case sensitive: only exact case match found
  - [ ] Regex `\d+`: digits found at correct positions
  - [ ] Invalid regex: empty result, no panic
  - [ ] Empty query: empty result

---

## 11.3 Row Text Extraction

Extract text from grid rows for search and URL detection, mapping between byte positions and column indices.

**File:** `oriterm_core/src/search/text.rs`

**Reference:** `_old/src/search/text.rs`

- [ ] `extract_row_text(row: &Row) -> (String, Vec<usize>)`
  - [ ] Iterate cells, skip `WIDE_CHAR_SPACER` and `LEADING_WIDE_CHAR_SPACER` cells
  - [ ] Replace `'\0'` with `' '` (null cells render as space)
  - [ ] Push base char to text, record column in col_map
  - [ ] Append zero-width characters (combining marks) from cell
  - [ ] Returns: extracted text + column map (char index -> grid column)
- [ ] `byte_span_to_cols(text: &str, col_map: &[usize], byte_start: usize, byte_end: usize) -> Option<(usize, usize)>`
  - [ ] Convert byte span in extracted text to `(start_col, end_col)` inclusive
  - [ ] Returns `None` if span is empty or indices out of range
  - [ ] Uses `char_index_at_byte` and `char_index_containing_byte` helpers
- [ ] `char_index_at_byte(text: &str, byte_offset: usize) -> usize` — private
  - [ ] First character starting at or after byte_offset
- [ ] `char_index_containing_byte(text: &str, byte_offset: usize) -> usize` — private
  - [ ] Character whose encoding contains byte_offset
- [ ] `pub(crate)` visibility for `extract_row_text` — shared with URL detection
- [ ] **Tests**:
  - [ ] ASCII row: text matches, col_map is identity
  - [ ] Wide char row: spacer cells skipped, col_map jumps by 2
  - [ ] Null cells: replaced with spaces
  - [ ] Byte span to cols: correct mapping for multi-byte UTF-8

---

## 11.4 Search UI

Search bar overlay rendered on top of the terminal grid, with text input, match count, and keyboard navigation.

**File:** `oriterm/src/app/search_ui.rs`

**Reference:** `_old/src/app/search_ui.rs`

- [ ] `App::open_search(&mut self, window_id: WindowId)`
  - [ ] Activate search for the active tab in the given window
  - [ ] Set `search_active = Some(window_id)`
  - [ ] Request redraw
- [ ] `App::close_search(&mut self, window_id: WindowId)`
  - [ ] Close search for the active tab, clear search state
  - [ ] Set `search_active = None`
  - [ ] Request redraw
- [ ] `App::handle_search_key(&mut self, window_id: WindowId, event: &KeyEvent)`
  - [ ] `Escape` — close search
  - [ ] `Enter` — next match (Shift+Enter = prev match)
  - [ ] `Backspace` — delete last character from query
  - [ ] `Character(c)` — append to query, re-run search
  - [ ] Each change calls `update_search` then requests redraw
- [ ] `App::update_search(&mut self, tab_id: TabId)`
  - [ ] Re-run `SearchState::update_query` with current grid
  - [ ] Call `scroll_to_search_match` to center viewport on focused match
- [ ] `App::scroll_to_search_match(&self, tab_id: TabId)`
  - [ ] Convert focused match's StableRowIndex to absolute row
  - [ ] If match outside current viewport: scroll display_offset to center it
- [ ] Search bar rendering:
  - [ ] Position: top-right of grid area (configurable in future)
  - [ ] Content: query text, match count ("N of M"), up/down navigation indicators
  - [ ] Rendered as GPU instances on foreground layer
  - [ ] Match highlight: all match cells get distinct background color
  - [ ] Focused match: different (brighter) background from non-focused matches
- [ ] Key flow: `Ctrl+Shift+F` -> open, type -> update+highlight, `Enter`/`Shift+Enter` -> next/prev, `Escape` -> close+clear
- [ ] Search state is per-tab (stored in Tab struct or binary-side wrapper)

---

## 11.5 Section Completion

- [ ] All 11.1-11.4 items complete
- [ ] `cargo test -p oriterm_core` — search tests pass
- [ ] `cargo clippy -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] Search finds plain text across viewport and scrollback
- [ ] Regex search works (including edge cases like `\d+`, `[a-z]+`)
- [ ] Invalid regex handled gracefully (no crash, empty results)
- [ ] Match cycling wraps around (next from last -> first, prev from first -> last)
- [ ] `cell_match_type` is O(log n) via binary search
- [ ] Search UI opens/closes cleanly, keyboard input captured during search
- [ ] Viewport scrolls to center focused match when outside view
- [ ] Wide characters and combining marks handled correctly in text extraction

**Exit Criteria:** Ctrl+Shift+F opens search, typing highlights matches in the grid, Enter/Shift+Enter navigates between matches, Escape closes. Plain text and regex modes both functional.
