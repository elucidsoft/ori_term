---
section: 1
title: Cell + Grid
status: not-started
tier: 0
goal: Build the core data structures — Cell, Row, Grid — in oriterm_core with full test coverage
sections:
  - id: "1.1"
    title: Workspace Setup
    status: not-started
  - id: "1.2"
    title: Index Newtypes
    status: not-started
  - id: "1.3"
    title: Cell Types
    status: not-started
  - id: "1.4"
    title: Row
    status: not-started
  - id: "1.5"
    title: Grid Foundation
    status: not-started
  - id: "1.6"
    title: Cursor
    status: not-started
  - id: "1.7"
    title: Grid Editing
    status: not-started
  - id: "1.8"
    title: Grid Navigation
    status: not-started
  - id: "1.9"
    title: Grid Scrolling
    status: not-started
  - id: "1.10"
    title: Scrollback Ring Buffer
    status: not-started
  - id: "1.11"
    title: Dirty Tracking
    status: not-started
  - id: "1.12"
    title: Section Completion
    status: not-started
---

# Section 01: Cell + Grid

**Status:** 📋 Planned
**Goal:** Build the foundational data structures for terminal emulation in `oriterm_core`. Every terminal operation ultimately reads or writes cells in a grid. This layer must be rock-solid before anything else is built on top.

**Crate:** `oriterm_core`
**Dependencies:** `bitflags`, `vte` (Color types only), `unicode-width`, `log`

---

## 1.1 Workspace Setup

Convert the single-crate project into a Cargo workspace with `oriterm_core` as the first library crate.

- [ ] Create `oriterm_core/` directory with `Cargo.toml` and `src/lib.rs`
  - [ ] `Cargo.toml`: name = `oriterm_core`, edition = 2024, same lint config as root
  - [ ] Dependencies: `bitflags = "2"`, `vte = { version = "0.15.0", features = ["ansi"] }`, `unicode-width = "0.2"`, `log = "0.4"`
  - [ ] `src/lib.rs`: module declarations, `//!` doc comment, `#![deny(unsafe_code)]`
- [ ] Convert root `Cargo.toml` to workspace
  - [ ] Add `[workspace]` section with `members = ["oriterm_core", "oriterm"]`
  - [ ] Move binary crate to `oriterm/` directory
  - [ ] `oriterm/Cargo.toml`: depends on `oriterm_core = { path = "../oriterm_core" }`
  - [ ] Binary at `oriterm/src/main.rs` (move current `src/main.rs`)
- [ ] Verify: `cargo build --target x86_64-pc-windows-gnu` succeeds for workspace
- [ ] Verify: `cargo test -p oriterm_core` runs (even if no tests yet)

---

## 1.2 Index Newtypes

Type-safe indices prevent mixing up row/column/line values. These are used everywhere.

**File:** `oriterm_core/src/index.rs`

- [ ] `Line(i32)` — Signed line index (negative = scrollback history)
  - [ ] `impl From<i32> for Line`, `impl From<Line> for i32`
  - [ ] `impl Add`, `Sub`, `AddAssign`, `SubAssign` for `Line`
  - [ ] `impl Display` for `Line` — shows inner value
- [ ] `Column(usize)` — Unsigned column index (0-based)
  - [ ] `impl From<usize> for Column`, `impl From<Column> for usize`
  - [ ] `impl Add`, `Sub`, `AddAssign`, `SubAssign` for `Column`
  - [ ] `impl Display` for `Column`
- [ ] `Point<L = Line>` — Generic grid coordinate
  - [ ] Fields: `line: L`, `column: Column`
  - [ ] `impl Point<Line>`: `fn new(line: Line, column: Column) -> Self`
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`
- [ ] `Side` enum — `Left`, `Right` (which half of a cell the cursor is on, for selection)
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [ ] `Direction` enum — `Left`, `Right` (for search, movement)
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [ ] `Boundary` enum — `Grid`, `Cursor`, `Wrap` (semantic boundaries for selection)
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [ ] Re-export all types from `lib.rs`
- [ ] **Tests** (`oriterm_core/src/index.rs` `#[cfg(test)]`):
  - [ ] Line arithmetic: `Line(5) + Line(3) == Line(8)`, negative lines
  - [ ] Column arithmetic: `Column(5) - Column(3) == Column(2)`
  - [ ] Point ordering: `Point { line: Line(0), column: Column(5) } < Point { line: Line(1), column: Column(0) }`

---

## 1.3 Cell Types

A Cell represents one character position in the terminal grid. Must be compact (target: 24 bytes) because there are `rows × cols × scrollback` of them.

**File:** `oriterm_core/src/cell.rs`

**Reference:** `_old/src/cell.rs` — carry forward the proven 24-byte layout.

- [ ] `CellFlags` — `bitflags! { struct CellFlags: u16 { ... } }`
  - [ ] `BOLD`, `DIM`, `ITALIC`, `UNDERLINE`, `BLINK`, `INVERSE`, `HIDDEN`, `STRIKETHROUGH`
  - [ ] `WIDE_CHAR` — This cell is a wide character (width 2)
  - [ ] `WIDE_CHAR_SPACER` — This cell is the trailing spacer of a wide character
  - [ ] `WRAP` — Line wrapped at this cell (soft wrap)
  - [ ] `CURLY_UNDERLINE`, `DOTTED_UNDERLINE`, `DASHED_UNDERLINE`, `DOUBLE_UNDERLINE`
  - [ ] Tests: set/clear/query individual flags, combine flags with `|`
- [ ] `CellExtra` — Heap-allocated optional data (only for cells that need it)
  - [ ] Fields:
    - `underline_color: Option<vte::ansi::Color>` — colored underline (SGR 58)
    - `hyperlink: Option<Hyperlink>` — OSC 8 hyperlink
    - `zerowidth: Vec<char>` — combining marks / zero-width characters appended to this cell
  - [ ] Wrapped in `Option<Box<CellExtra>>` in Cell — None for normal cells (zero overhead)
  - [ ] `CellExtra::new() -> Self` — all fields None/empty
- [ ] `Hyperlink` — URL hyperlink data
  - [ ] Fields: `id: Option<String>`, `uri: String`
  - [ ] Derive: `Debug`, `Clone`, `PartialEq`, `Eq`
- [ ] `Cell` — The core cell struct
  - [ ] Fields:
    - `ch: char` (4 bytes) — the character
    - `fg: vte::ansi::Color` (enum, for deferred palette resolution)
    - `bg: vte::ansi::Color`
    - `flags: CellFlags` (2 bytes)
    - `extra: Option<Box<CellExtra>>` (8 bytes pointer, None = 0)
  - [ ] `Cell::default()` — space character, default fg/bg, no flags, no extra
  - [ ] `Cell::reset(&mut self, template: &Cell)` — reset to template (for erase operations)
  - [ ] `Cell::is_empty(&self) -> bool` — space char, default colors, no flags
  - [ ] `Cell::width(&self) -> usize` — returns `unicode_width::UnicodeWidthChar::width(self.ch).unwrap_or(1)`, respecting `WIDE_CHAR` flag
  - [ ] Derive: `Debug`, `Clone`, `PartialEq`
- [ ] Verify `std::mem::size_of::<Cell>()` ≤ 24 bytes
  - [ ] Add compile-time assert: `const _: () = assert!(std::mem::size_of::<Cell>() <= 24);`
- [ ] Re-export `Cell`, `CellFlags`, `CellExtra`, `Hyperlink` from `lib.rs`
- [ ] **Tests** (`oriterm_core/src/cell.rs` `#[cfg(test)]`):
  - [ ] Default cell is space with default colors
  - [ ] Reset clears to template
  - [ ] `is_empty` returns true for default, false after setting char
  - [ ] Wide char cell has `WIDE_CHAR` flag, width returns 2
  - [ ] CellExtra is None for normal cells, Some for underline color/hyperlink/zerowidth
  - [ ] Appending a combining mark to a cell creates CellExtra with zerowidth vec
  - [ ] Size assertion: `size_of::<Cell>() <= 24`

---

## 1.4 Row

A Row is a contiguous array of Cells representing one terminal line.

**File:** `oriterm_core/src/grid/row.rs`

- [ ] `Row` struct
  - [ ] Fields:
    - `inner: Vec<Cell>` — the cells
    - `occ: usize` — occupancy: index of last non-empty cell + 1 (optimization for sparse rows)
  - [ ] `Row::new(cols: usize) -> Self` — creates row of `cols` default cells, `occ = 0`
  - [ ] `Row::reset(&mut self, cols: usize, template: &Cell)` — reset all cells to template, resize if needed, `occ = 0`
  - [ ] `Row::cols(&self) -> usize` — returns `inner.len()`
  - [ ] `Row::occ(&self) -> usize` — returns occupancy
  - [ ] `impl Index<Column> for Row` — returns `&Cell` at column
  - [ ] `impl IndexMut<Column> for Row` — returns `&mut Cell` at column, updates `occ` if needed
  - [ ] `Row::clear_range(&mut self, range: Range<Column>, template: &Cell)` — clear cells in range
  - [ ] `Row::truncate(&mut self, col: Column)` — clear from col to end, update occ
  - [ ] `Row::append(&mut self, col: Column, cell: &Cell)` — write cell at col, update occ
- [ ] **Tests** (`oriterm_core/src/grid/row.rs` `#[cfg(test)]`):
  - [ ] New row has correct length, all default cells, occ = 0
  - [ ] Writing a cell at column 5 sets occ = 6
  - [ ] Reset clears all cells and resets occ
  - [ ] Index/IndexMut return correct cells
  - [ ] clear_range resets specified columns
  - [ ] truncate clears from column to end

---

## 1.5 Grid Foundation

The Grid is the 2D cell storage. At this stage: a simple Vec of Rows with dimensions. No scrollback yet (added in 1.10).

**File:** `oriterm_core/src/grid/mod.rs`

- [ ] Module declarations: `mod row; mod cursor; mod scroll; mod editing; mod navigation; mod ring; mod dirty;`
- [ ] Re-export key types
- [ ] `Grid` struct (initial, no scrollback)
  - [ ] Fields:
    - `rows: Vec<Row>` — visible rows (indexed 0 = top, N-1 = bottom)
    - `cols: usize` — number of columns
    - `lines: usize` — number of visible lines
    - `cursor: Cursor` — current cursor position + template
    - `saved_cursor: Option<Cursor>` — DECSC/DECRC saved cursor
    - `tab_stops: Vec<bool>` — tab stop at each column (default every 8)
  - [ ] `Grid::new(lines: usize, cols: usize) -> Self`
    - [ ] Allocate `lines` rows of `cols` cells each
    - [ ] Initialize tab stops every 8 columns
    - [ ] Cursor at (0, 0) with default template
  - [ ] `Grid::lines(&self) -> usize`
  - [ ] `Grid::cols(&self) -> usize`
  - [ ] `Grid::cursor(&self) -> &Cursor`
  - [ ] `Grid::cursor_mut(&mut self) -> &mut Cursor`
  - [ ] `impl Index<Line> for Grid` — returns `&Row` (Line(0) = first visible row)
  - [ ] `impl IndexMut<Line> for Grid` — returns `&mut Row`
- [ ] **Tests** (`oriterm_core/src/grid/mod.rs` `#[cfg(test)]`):
  - [ ] New grid has correct dimensions
  - [ ] Tab stops initialized at every 8 columns
  - [ ] Index by Line returns correct row
  - [ ] Cursor starts at (0, 0)

---

## 1.6 Cursor

The cursor tracks the current write position and the "template cell" used for newly written characters.

**File:** `oriterm_core/src/grid/cursor.rs`

- [ ] `Cursor` struct
  - [ ] Fields:
    - `point: Point<usize>` — line (usize index into visible rows), column
    - `template: Cell` — cell template: fg, bg, flags applied to new characters
    - `shape: CursorShape` — block, underline, bar (for rendering)
  - [ ] `Cursor::new() -> Self` — point at (0, 0), default template, block shape
  - [ ] `Cursor::line(&self) -> usize`
  - [ ] `Cursor::col(&self) -> Column`
  - [ ] `Cursor::set_line(&mut self, line: usize)`
  - [ ] `Cursor::set_col(&mut self, col: Column)`
- [ ] `CursorShape` enum — `Block`, `Underline`, `Bar`, `HollowBlock`
  - [ ] `Default` impl returns `Block`
- [ ] **Tests**:
  - [ ] Default cursor at (0, 0) with block shape
  - [ ] Setting line/col updates point

---

## 1.7 Grid Editing

Character insertion, deletion, and erase operations. These are the primitives the VTE handler will call.

**File:** `oriterm_core/src/grid/editing.rs`

Methods on `Grid`:

- [ ] `put_char(&mut self, ch: char)`
  - [ ] Write `ch` into cell at cursor position, using cursor template for colors/flags
  - [ ] Handle wide chars: write cell with `WIDE_CHAR` flag, write spacer in next column with `WIDE_CHAR_SPACER`
  - [ ] If cursor is at last column, set `WRAP` flag but don't advance (next char triggers scroll + wrap)
  - [ ] Otherwise, advance cursor column by character width
  - [ ] If overwriting a wide char spacer, clear the preceding wide char cell
  - [ ] If overwriting a wide char, clear its spacer
  - [ ] Mark row dirty
- [ ] `insert_blank(&mut self, count: usize)`
  - [ ] Insert `count` blank cells at cursor, shifting existing cells right
  - [ ] Cells that shift past the right edge are lost
  - [ ] Mark row dirty
- [ ] `delete_chars(&mut self, count: usize)`
  - [ ] Delete `count` cells at cursor, shifting remaining cells left
  - [ ] New cells at right edge are blank (cursor template)
  - [ ] Mark row dirty
- [ ] `erase_display(&mut self, mode: EraseMode)`
  - [ ] `EraseMode::Below` — erase from cursor to end of display
  - [ ] `EraseMode::Above` — erase from start of display to cursor
  - [ ] `EraseMode::All` — erase entire display
  - [ ] `EraseMode::Scrollback` — erase scrollback buffer only
  - [ ] Mark affected rows dirty
- [ ] `erase_line(&mut self, mode: EraseMode)`
  - [ ] `Below` — erase from cursor to end of line
  - [ ] `Above` — erase from start of line to cursor
  - [ ] `All` — erase entire line
  - [ ] Mark row dirty
- [ ] `erase_chars(&mut self, count: usize)`
  - [ ] Erase `count` cells starting at cursor (replace with template, don't shift)
  - [ ] Mark row dirty
- [ ] `EraseMode` enum — `Below`, `Above`, `All`, `Scrollback`
- [ ] **Tests** (`oriterm_core/src/grid/editing.rs` `#[cfg(test)]`):
  - [ ] `put_char('A')` at (0,0) writes 'A', cursor advances to col 1
  - [ ] `put_char('好')` (wide) writes 好 + spacer, cursor advances by 2
  - [ ] Wide char at last column: wraps correctly
  - [ ] Overwriting spacer clears preceding wide char
  - [ ] Overwriting wide char clears its spacer
  - [ ] `insert_blank(3)` shifts cells right by 3
  - [ ] `delete_chars(2)` shifts cells left by 2, blanks at right
  - [ ] `erase_display(Below)` clears from cursor to end
  - [ ] `erase_display(Above)` clears from start to cursor
  - [ ] `erase_display(All)` clears everything
  - [ ] `erase_line(Below)` clears from cursor to end of line
  - [ ] `erase_line(All)` clears entire line
  - [ ] `erase_chars(5)` erases 5 cells without shifting

---

## 1.8 Grid Navigation

Cursor movement operations. The VTE handler calls these for CUU/CUD/CUF/CUB/CUP/CR/LF/etc.

**File:** `oriterm_core/src/grid/navigation.rs`

Methods on `Grid`:

- [ ] `move_up(&mut self, count: usize)` — CUU: move cursor up, clamped to top of screen (or scroll region)
- [ ] `move_down(&mut self, count: usize)` — CUD: move cursor down, clamped to bottom of screen (or scroll region)
- [ ] `move_forward(&mut self, count: usize)` — CUF: move cursor right, clamped to last column
- [ ] `move_backward(&mut self, count: usize)` — CUB: move cursor left, clamped to column 0
- [ ] `move_to(&mut self, line: usize, col: Column)` — CUP: absolute position, clamped to grid bounds
- [ ] `move_to_column(&mut self, col: Column)` — CHA: absolute column, clamped
- [ ] `move_to_line(&mut self, line: usize)` — VPA: absolute line, clamped
- [ ] `carriage_return(&mut self)` — CR: cursor to column 0
- [ ] `linefeed(&mut self)` — LF: move down one line; if at bottom of scroll region, scroll up
- [ ] `reverse_index(&mut self)` — RI: move up one line; if at top of scroll region, scroll down
- [ ] `next_line(&mut self)` — NEL: carriage return + linefeed
- [ ] `tab(&mut self)` — HT: advance to next tab stop (or end of line)
  - [ ] Respects `self.tab_stops` vector
- [ ] `tab_backward(&mut self)` — CBT: move to previous tab stop (or start of line)
- [ ] `set_tab_stop(&mut self)` — HTS: set tab stop at current column
- [ ] `clear_tab_stop(&mut self, mode: TabClearMode)` — TBC: clear current or all tab stops
- [ ] `TabClearMode` enum — `Current`, `All`
- [ ] `save_cursor(&mut self)` — DECSC: save cursor position + template to `saved_cursor`
- [ ] `restore_cursor(&mut self)` — DECRC: restore from `saved_cursor` (or reset if none)
- [ ] **Tests** (`oriterm_core/src/grid/navigation.rs` `#[cfg(test)]`):
  - [ ] `move_up(3)` from line 5 → line 2
  - [ ] `move_up(100)` from line 5 → line 0 (clamped)
  - [ ] `move_down(3)` from line 0 → line 3
  - [ ] `move_down(100)` clamps to bottom
  - [ ] `move_forward(5)` from col 0 → col 5
  - [ ] `move_forward(100)` clamps to last column
  - [ ] `move_backward(3)` from col 5 → col 2
  - [ ] `move_to(5, 10)` sets cursor to (5, 10)
  - [ ] `carriage_return` sets col to 0
  - [ ] `linefeed` at bottom of screen triggers scroll
  - [ ] `linefeed` in middle of screen moves cursor down
  - [ ] `reverse_index` at top triggers scroll_down
  - [ ] `tab` advances to next tab stop
  - [ ] `tab` at last tab stop goes to end of line
  - [ ] `tab_backward` moves to previous tab stop
  - [ ] `set_tab_stop` / `clear_tab_stop` work correctly
  - [ ] `save_cursor` / `restore_cursor` round-trip

---

## 1.9 Grid Scrolling

Scroll operations within scroll regions. A scroll region is a range of lines (set by DECSTBM).

**File:** `oriterm_core/src/grid/scroll.rs`

- [ ] Add to `Grid`:
  - [ ] Field: `scroll_region: Range<usize>` — top..bottom (default: 0..lines)
  - [ ] `set_scroll_region(&mut self, top: usize, bottom: usize)` — DECSTBM
    - [ ] Validate: top < bottom, both within grid bounds
    - [ ] Store as `top..bottom`
- [ ] `scroll_up(&mut self, count: usize)`
  - [ ] Move rows in scroll region up by `count`
  - [ ] Top rows go to scrollback (if scroll region is full screen) or are lost (if sub-region)
  - [ ] New blank rows appear at bottom of region
  - [ ] Mark affected rows dirty
- [ ] `scroll_down(&mut self, count: usize)`
  - [ ] Move rows in scroll region down by `count`
  - [ ] Bottom rows are lost
  - [ ] New blank rows appear at top of region
  - [ ] Mark affected rows dirty
- [ ] `insert_lines(&mut self, count: usize)` — IL: insert blank lines at cursor, pushing down
  - [ ] Only operates within scroll region
  - [ ] Cursor must be within scroll region
- [ ] `delete_lines(&mut self, count: usize)` — DL: delete lines at cursor, pulling up
  - [ ] Only operates within scroll region
  - [ ] New blank lines at bottom of region
- [ ] **Tests** (`oriterm_core/src/grid/scroll.rs` `#[cfg(test)]`):
  - [ ] `scroll_up(1)` with full-screen region: top row evicted, blank at bottom
  - [ ] `scroll_up(3)` with sub-region: only region rows move
  - [ ] `scroll_down(1)`: bottom row lost, blank at top of region
  - [ ] `insert_lines(2)` at cursor line: 2 blank lines inserted, bottom rows lost
  - [ ] `delete_lines(2)` at cursor line: 2 lines removed, blanks at bottom
  - [ ] Scroll region boundaries respected (rows outside region untouched)
  - [ ] `set_scroll_region` with invalid values is clamped

---

## 1.10 Scrollback Ring Buffer

Efficient storage for scrollback history. Rows that scroll off the top go into a ring buffer. Users can scroll up to view history.

**File:** `oriterm_core/src/grid/ring.rs`

- [ ] `ScrollbackBuffer` struct
  - [ ] Fields:
    - `buf: Vec<Row>` — pre-allocated ring buffer
    - `max_scrollback: usize` — maximum history lines (configurable, default 10000)
    - `len: usize` — current number of rows in buffer
    - `start: usize` — ring buffer start index
  - [ ] `ScrollbackBuffer::new(max_scrollback: usize) -> Self`
  - [ ] `push(&mut self, row: Row)` — add row to scrollback (evicts oldest if full)
  - [ ] `len(&self) -> usize` — number of rows stored
  - [ ] `get(&self, index: usize) -> Option<&Row>` — index 0 = most recent, len-1 = oldest
  - [ ] `iter(&self) -> impl Iterator<Item = &Row>` — iterate newest to oldest
  - [ ] `clear(&mut self)` — clear all scrollback
- [ ] Integrate with `Grid`:
  - [ ] Add field: `scrollback: ScrollbackBuffer`
  - [ ] Add field: `display_offset: usize` — how many lines scrolled back (0 = live)
  - [ ] `Grid::scroll_up` pushes evicted rows to scrollback (when scroll region is full screen)
  - [ ] `Grid::total_lines(&self) -> usize` — `self.lines + self.scrollback.len()`
  - [ ] `Grid::display_offset(&self) -> usize`
  - [ ] `Grid::scroll_display(&mut self, delta: isize)` — adjust display_offset, clamped
- [ ] **Tests** (`oriterm_core/src/grid/ring.rs` `#[cfg(test)]`):
  - [ ] Push rows into scrollback, verify retrieval order (newest first)
  - [ ] Ring buffer wraps: push max+10 rows, only max retained
  - [ ] Clear empties the buffer
  - [ ] Integration: scroll_up pushes to scrollback
  - [ ] display_offset scrolls through history
  - [ ] display_offset clamped to scrollback length

---

## 1.11 Dirty Tracking

Track which rows have changed since last read. Enables damage-based rendering.

**File:** `oriterm_core/src/grid/dirty.rs`

- [ ] `DirtyTracker` struct
  - [ ] Fields:
    - `dirty: Vec<bool>` — one bool per visible row
    - `all_dirty: bool` — shortcut: everything changed (resize, scroll, alt screen swap)
  - [ ] `DirtyTracker::new(lines: usize) -> Self` — all clean
  - [ ] `mark(&mut self, line: usize)` — mark single line dirty
  - [ ] `mark_all(&mut self)` — mark everything dirty
  - [ ] `is_dirty(&self, line: usize) -> bool`
  - [ ] `is_any_dirty(&self) -> bool`
  - [ ] `drain(&mut self) -> DirtyIterator` — returns iterator of dirty line indices, resets all to clean
  - [ ] `resize(&mut self, lines: usize)` — resize tracker, mark all dirty
- [ ] Integrate with `Grid`:
  - [ ] Add field: `dirty: DirtyTracker`
  - [ ] All editing/scroll/navigation methods that change cells call `self.dirty.mark(line)`
  - [ ] `scroll_up`/`scroll_down` call `self.dirty.mark_all()` (conservative, can optimize later)
- [ ] **Tests** (`oriterm_core/src/grid/dirty.rs` `#[cfg(test)]`):
  - [ ] New tracker: nothing dirty
  - [ ] Mark line 5: only line 5 is dirty
  - [ ] Mark all: everything dirty
  - [ ] Drain: returns dirty lines, resets to clean
  - [ ] After drain, nothing is dirty
  - [ ] Resize marks all dirty

---

## 1.12 Section Completion

- [ ] All 1.1–1.11 items complete
- [ ] `cargo test -p oriterm_core` — all tests pass
- [ ] `cargo clippy -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo doc -p oriterm_core --no-deps` — generates clean docs
- [ ] Grid can: create, write chars (including wide), move cursor, scroll, erase, tab stops, scrollback, dirty tracking
- [ ] No VTE, no events, no palette, no selection, no rendering — just data structures + operations

**Exit Criteria:** `oriterm_core` compiles, all grid operations are tested, `cargo test -p oriterm_core` passes with zero failures.
