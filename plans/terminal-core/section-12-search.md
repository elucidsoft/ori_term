---
section: "12"
title: Search
status: complete
goal: Implement incremental search through terminal scrollback with regex support
sections:
  - id: "12.1"
    title: Search Engine
    status: complete
  - id: "12.2"
    title: Search UI
    status: complete
  - id: "12.3"
    title: Completion Checklist
    status: complete
---

# Section 12: Search

**Status:** Complete
**Goal:** Users can search through terminal output (visible + scrollback) with
incremental highlighting, regex support, and next/previous navigation.

**Inspired by:**
- Alacritty's regex-based search with DFA matching across grid rows
- Ghostty's search with visual match highlighting
- WezTerm's search with multiple modes (plain, regex, case-sensitive)

**Current state:** Fully implemented in `src/search.rs` with renderer integration
and keyboard handling.

---

## 12.1 Search Engine

Core search logic over the terminal grid and scrollback.

- [x] Search modes:
  - [x] Plain text (exact substring match)
  - [x] Case-insensitive text (default)
  - [x] Regex (using `regex` crate, `RegexBuilder`)
- [x] Search scope: visible grid + all scrollback rows
- [x] Extract searchable text from grid (`extract_row_text()`):
  - [x] Walk rows, concatenating cell characters
  - [x] Handle wide characters (include char, skip spacer)
  - [x] Handle grapheme clusters (combine base + zerowidth)
  - [ ] Handle wrapped lines (join into logical lines) — deferred, per-row search only
- [x] Match result: `SearchMatch { start_row, start_col, end_row, end_col }`
- [x] Find all matches in entire grid (for highlighting)
- [x] Find next/previous match via `next_match()` / `prev_match()` with wrapping
- [x] Incremental: `update_query()` re-runs search as query changes
- [x] Binary search cell hit-test via `cell_match_type()` for O(log n) per-cell lookup
- [ ] Performance: lazy search for large scrollback — deferred (synchronous is fast enough for ~10k rows)

**Implementation:** `src/search.rs` — `SearchState`, `SearchMatch`, `MatchType`,
`find_matches()`, `extract_row_text()`, `byte_span_to_cols()`

**Ref:** Alacritty `term/search.rs`, `regex` crate

---

## 12.2 Search UI

User-facing search interaction.

- [x] Activate search: Ctrl+Shift+F
- [x] Search bar: overlay at bottom of terminal
  - [x] Text input field with current query
  - [x] Cursor indicator (2px bar after query text)
  - [x] Match count indicator (e.g., "3 of 47" or "No matches")
  - [ ] Mode toggle buttons (case, regex) — deferred to 13.2 (config UI)
- [x] Navigation:
  - [x] Enter: next match
  - [x] Shift+Enter: previous match
  - [x] Scroll viewport to show current match (centered)
- [x] Highlighting:
  - [x] Current match: orange background `rgb(200, 120, 30)` with black fg
  - [x] Other matches: dark yellow background `rgb(80, 80, 20)`
  - [ ] Highlight colors configurable — deferred to 13.2
- [x] Close search: Escape
  - [x] Clear highlights and search state
- [x] Integration with rendering:
  - [x] `search: Option<&SearchState>` in `FrameParams`
  - [x] Cell loop applies match highlighting before selection
  - [x] `build_search_bar_overlay()` renders bar at bottom of window
- [x] Keyboard interception: all keys go to search query (not PTY) when active

**Implementation:** `src/gpu/renderer.rs` (`build_search_bar_overlay`, cell loop),
`src/app.rs` (`open_search`, `close_search`, `handle_search_key`, `scroll_to_search_match`)

**Ref:** Ghostty search UI, Alacritty vi mode search

---

## 12.3 Completion Checklist

- [x] Ctrl+Shift+F opens search bar
- [x] Plain text search finds matches
- [x] Case-insensitive search works
- [x] Regex search works
- [x] All matches highlighted in viewport
- [x] Current match has distinct highlight
- [x] Enter/Shift+Enter navigates between matches
- [x] Viewport scrolls to show matches
- [ ] Search across wrapped lines works — deferred (per-row only)
- [x] Search through scrollback works
- [x] Escape closes search and clears highlights
- [x] Performance acceptable with 10k+ lines of scrollback
- [x] 7 unit tests pass (basic, case, regex, invalid regex, empty, nav, cell match type)
- [x] `cargo clippy` clean, `cargo test` 104 tests pass

**Exit Criteria:** Users can search through terminal history, navigate between
matches, and find text across scrollback. ✓

**Deferred:**
- Wrapped line search (join WRAPLINE rows into logical lines)
- Configurable highlight colors (Section 13.2)
- Mode toggle UI (case sensitive / regex buttons in search bar)
- Arrow key cursor movement within query
