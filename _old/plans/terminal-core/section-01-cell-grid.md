---
section: "01"
title: Cell & Grid Model
status: complete
goal: Replace the current minimal Cell/Grid with a rich, compact representation inspired by Alacritty and Ghostty
sections:
  - id: "01.1"
    title: Rich Cell Struct
    status: complete
  - id: "01.2"
    title: Cell Flags
    status: complete
  - id: "01.3"
    title: Cell Extras
    status: complete
  - id: "01.4"
    title: Grid Rewrite
    status: complete
  - id: "01.5"
    title: Cursor Model
    status: complete
  - id: "01.6"
    title: Completion Checklist
    status: complete
---

# Section 01: Cell & Grid Model

**Status:** Complete
**Goal:** Build a compact, rich cell representation that supports all terminal attributes while keeping memory usage low. The cell is the atom of the terminal -- everything else builds on it.

**Inspired by:**
- Alacritty's 24-byte Cell with bitflags + `Arc<CellExtra>` for rare data
- Ghostty's packed Cell struct with clamped width (0-2) and grapheme break properties

**Implemented in:** `src/cell.rs`, `src/grid/mod.rs`, `src/grid/row.rs`, `src/grid/cursor.rs`

**What was built:**
- Rich `Cell` struct using `vte::ansi::Color` directly (Named/Indexed/Spec)
- `CellFlags` bitflags u16 with all SGR attributes + WIDE_CHAR + WRAPLINE
- `CellExtra` with `Arc` wrapping (zerowidth, underline_color, hyperlink)
- `Row` wrapper with `occ` occupancy tracking
- `Grid` with cursor, saved_cursor, scroll regions, scrollback, tab stops
- `Cursor` with template cell and `input_needs_wrap` deferred-wrap
- All grid operations: put_char, put_wide_char, newline, erase, scroll, insert/delete, resize
- 13+ unit tests

---

## 01.1 Rich Cell Struct

Design target: ~24 bytes per cell (matches Alacritty's proven compact layout).

- [ ] Define `Color` enum supporting Named(u8), Indexed(u8), Rgb(u8,u8,u8)
  - [ ] Named: 16 ANSI colors (0-15)
  - [ ] Indexed: 256-color palette (0-255)
  - [ ] Rgb: 24-bit truecolor
  - [ ] Keep it small -- fit in 4 bytes via packed repr

- [ ] Define new `Cell` struct
  ```rust
  pub struct Cell {
      pub c: char,                         // 4 bytes - the character
      pub fg: Color,                       // 4 bytes - foreground
      pub bg: Color,                       // 4 bytes - background
      pub flags: CellFlags,                // 2 bytes - bitflags
      pub extra: Option<Arc<CellExtra>>,   // 8 bytes - rare data (Option<Arc> is 8 bytes)
  }
  // Total: 22 bytes, padded to 24
  ```

- [ ] Implement `Cell::default()` returning a space with default fg/bg
- [ ] Implement `Cell::blank()` and `Cell::blank_with_attrs(fg, bg)`
- [ ] Add `#[cfg(test)]` size assertion: `assert_eq!(std::mem::size_of::<Cell>(), 24)`

**Ref:** Alacritty `alacritty_terminal/src/term/cell.rs`

---

## 01.2 Cell Flags

Bitflags for per-cell rendering attributes. Must cover everything SGR can set.

- [ ] Define `CellFlags` as `bitflags! { struct CellFlags: u16 { ... } }`
  - [ ] `BOLD`           = 0x0001
  - [ ] `DIM`            = 0x0002
  - [ ] `ITALIC`         = 0x0004
  - [ ] `UNDERLINE`      = 0x0008
  - [ ] `DOUBLE_UNDERLINE` = 0x0010
  - [ ] `UNDERCURL`      = 0x0020
  - [ ] `DOTTED_UNDERLINE` = 0x0040
  - [ ] `DASHED_UNDERLINE` = 0x0080
  - [ ] `BLINK`          = 0x0100
  - [ ] `INVERSE`        = 0x0200
  - [ ] `HIDDEN`         = 0x0400
  - [ ] `STRIKEOUT`      = 0x0800
  - [ ] `WIDE_CHAR`      = 0x1000  (character occupies 2 columns)
  - [ ] `WIDE_CHAR_SPACER` = 0x2000  (placeholder cell after wide char)
  - [ ] `WRAPLINE`       = 0x4000  (this cell is at end of a wrapped line)
  - [ ] `LEADING_WIDE_CHAR_SPACER` = 0x8000 (spacer before wide char at line break)

- [ ] Implement helper methods: `is_wide()`, `is_spacer()`, `is_wrapped()`

**Ref:** Alacritty `cell.rs` Flags bitflags, Ghostty `page.zig` Cell packed struct

---

## 01.3 Cell Extras

Rarely-used cell data goes in a heap-allocated `Arc<CellExtra>` to keep the
common cell compact.

- [ ] Define `CellExtra` struct
  ```rust
  pub struct CellExtra {
      pub zerowidth: Vec<char>,            // combining marks
      pub underline_color: Option<Color>,  // colored underlines (SGR 58)
      pub hyperlink: Option<Hyperlink>,    // OSC 8 hyperlinks
  }
  ```

- [ ] Define `Hyperlink` struct
  ```rust
  pub struct Hyperlink {
      pub id: Option<String>,
      pub uri: String,
  }
  ```

- [ ] Add methods to `Cell`:
  - [ ] `push_zerowidth(c: char)` -- lazily creates `CellExtra` on first combining mark
  - [ ] `zerowidth_chars(&self) -> &[char]` -- returns empty slice if no extra
  - [ ] `set_hyperlink(link: Hyperlink)` -- attaches hyperlink
  - [ ] `hyperlink(&self) -> Option<&Hyperlink>`

**Ref:** Alacritty `CellExtra` struct, WezTerm `Cell::new_grapheme()` for combining marks

---

## 01.4 Grid Rewrite

Replace the flat `Vec<Cell>` with a proper grid that supports dynamic sizing.

- [ ] Define `Row` struct wrapping `Vec<Cell>` with occupied-cell tracking
  ```rust
  pub struct Row {
      cells: Vec<Cell>,
      occ: usize,  // rightmost occupied cell index (for efficient reset)
  }
  ```
  - [ ] `Row::new(cols)` -- creates blank row
  - [ ] `Row::reset(&mut self, template: &Cell)` -- resets only occupied range
  - [ ] `Row::get(col) -> &Cell` / `Row::get_mut(col) -> &mut Cell`

- [ ] Define new `Grid` struct
  ```rust
  pub struct Grid {
      rows: Vec<Row>,           // visible rows
      cols: usize,
      lines: usize,             // visible line count
      cursor: Cursor,
      saved_cursor: Cursor,     // for DECSC/DECRC
  }
  ```

- [ ] Implement grid operations:
  - [ ] `put_char(c: char, fg: Color, bg: Color, flags: CellFlags)` -- write at cursor, advance
  - [ ] `put_wide_char(c: char, ...)` -- write wide char + spacer, handle line boundary
  - [ ] `newline()` -- move cursor down, scroll if at bottom
  - [ ] `carriage_return()` -- cursor to column 0
  - [ ] `backspace()` -- cursor left one
  - [ ] `scroll_up(region_top, region_bottom, count)` -- scroll within region
  - [ ] `scroll_down(region_top, region_bottom, count)` -- scroll within region
  - [ ] `clear()` -- reset entire grid
  - [ ] `erase_display(mode)` -- ED 0/1/2/3
  - [ ] `erase_line(mode)` -- EL 0/1/2
  - [ ] `insert_blank_chars(count)` -- shift cells right
  - [ ] `delete_chars(count)` -- shift cells left
  - [ ] `insert_lines(count)` -- insert blank lines in scroll region
  - [ ] `delete_lines(count)` -- delete lines in scroll region
  - [ ] `resize(new_cols, new_lines)` -- resize without reflow (reflow in Section 04)

**Ref:** Alacritty `grid/mod.rs`, Ghostty `Terminal.zig`

---

## 01.5 Cursor Model

Proper cursor with saved state and template cell.

- [ ] Define `Cursor` struct
  ```rust
  pub struct Cursor {
      pub col: usize,
      pub row: usize,
      pub template: Cell,           // current attributes for new characters
      pub input_needs_wrap: bool,    // cursor past last column, next char wraps
  }
  ```

- [ ] Implement `DECSC` (save cursor) -- copies cursor to `saved_cursor`
- [ ] Implement `DECRC` (restore cursor) -- copies `saved_cursor` to `cursor`
- [ ] `input_needs_wrap` flag: when cursor is at last column and a char is printed,
  the cursor stays at the last column with this flag set. The *next* character
  triggers the wrap. This matches xterm/Ghostty/Alacritty behavior.

**Ref:** Alacritty `Cursor<T>` with template, Ghostty `Terminal.zig` cursor handling

---

## 01.6 Completion Checklist

- [x] Cell struct is 24 bytes (verified by test)
- [x] All CellFlags map to SGR attributes
- [x] Wide characters produce char + spacer cells
- [x] Zero-width characters stored via CellExtra
- [x] Grid operations cover ED, EL, IL, DL, ICH, DCH
- [x] Cursor save/restore works
- [x] `input_needs_wrap` deferred-wrap behavior correct
- [x] Old `grid.rs` replaced; old `Cell` removed
- [x] `render.rs` updated to read new Cell struct
- [x] `vte_performer.rs` replaced by `term_handler.rs` using vte ansi Handler trait
- [x] All existing functionality preserved (cmd.exe still works)
- [x] Unit tests for cell creation, flags, wide chars, grid operations

**Exit Criteria:** Complete. Terminal renders cmd.exe and WSL output with the new
Cell/Grid model. Wide characters display correctly. Bold/color attributes stored
and rendered.
