---
section: "12"
title: Search
status: not-started
goal: Implement incremental search through terminal scrollback with regex support
sections:
  - id: "12.1"
    title: Search Engine
    status: not-started
  - id: "12.2"
    title: Search UI
    status: not-started
  - id: "12.3"
    title: Completion Checklist
    status: not-started
---

# Section 12: Search

**Status:** Not Started
**Goal:** Users can search through terminal output (visible + scrollback) with
incremental highlighting, regex support, and next/previous navigation.

**Inspired by:**
- Alacritty's regex-based search with DFA matching across grid rows
- Ghostty's search with visual match highlighting
- WezTerm's search with multiple modes (plain, regex, case-sensitive)

**Current state:** No search support.

---

## 12.1 Search Engine

Core search logic over the terminal grid and scrollback.

- [ ] Search modes:
  - [ ] Plain text (exact substring match)
  - [ ] Case-insensitive text
  - [ ] Regex (using `regex` or `regex-automata` crate)
- [ ] Search scope: visible grid + all scrollback rows
- [ ] Extract searchable text from grid:
  - [ ] Walk rows, concatenating cell characters
  - [ ] Handle wide characters (include char, skip spacer)
  - [ ] Handle wrapped lines (join into logical lines)
  - [ ] Handle grapheme clusters (combine base + zerowidth)
- [ ] Match result: `Match { start_row, start_col, end_row, end_col }`
- [ ] Find all matches in viewport (for highlighting)
- [ ] Find next/previous match from current position
- [ ] Incremental: update matches as search query changes
- [ ] Performance: for large scrollback (10k+ lines), search lazily
  - [ ] Search visible viewport first, then expand outward
  - [ ] Use `regex-automata` DFA for fast matching if using regex

**Ref:** Alacritty `term/search.rs`, `regex-automata` crate

---

## 12.2 Search UI

User-facing search interaction.

- [ ] Activate search: Ctrl+Shift+F (configurable)
- [ ] Search bar: overlay at top or bottom of terminal
  - [ ] Text input field with current query
  - [ ] Match count indicator (e.g., "3 of 47")
  - [ ] Mode toggle buttons (case, regex)
- [ ] Navigation:
  - [ ] Enter / Ctrl+G: next match
  - [ ] Shift+Enter / Ctrl+Shift+G: previous match
  - [ ] Scroll viewport to show current match
- [ ] Highlighting:
  - [ ] Current match: bright highlight (e.g., orange background)
  - [ ] Other matches: dim highlight (e.g., yellow background)
  - [ ] Highlight colors configurable
- [ ] Close search: Escape
  - [ ] Clear highlights
  - [ ] Return to previous scroll position (or stay at current match)
- [ ] Integration with rendering:
  - [ ] Add `search_matches: Vec<Match>` to render state
  - [ ] Render pass applies match highlighting to affected cells

**Ref:** Ghostty search UI, Alacritty vi mode search

---

## 12.3 Completion Checklist

- [ ] Ctrl+Shift+F opens search bar
- [ ] Plain text search finds matches
- [ ] Case-insensitive search works
- [ ] Regex search works
- [ ] All matches highlighted in viewport
- [ ] Current match has distinct highlight
- [ ] Enter/Shift+Enter navigates between matches
- [ ] Viewport scrolls to show matches
- [ ] Search across wrapped lines works
- [ ] Search through scrollback works
- [ ] Escape closes search and clears highlights
- [ ] Performance acceptable with 10k+ lines of scrollback

**Exit Criteria:** Users can search through terminal history, navigate between
matches, and find text across wrapped lines and scrollback.
