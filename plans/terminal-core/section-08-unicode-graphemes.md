---
section: "08"
title: Unicode & Graphemes
status: complete
goal: Correct Unicode handling including grapheme cluster rendering, combining marks, ZWJ sequences, width fixes, and grapheme-aware selection
sections:
  - id: "08.1"
    title: Render Combining Marks
    status: complete
  - id: "08.2"
    title: Grapheme Cluster Input Buffering
    status: complete
  - id: "08.3"
    title: Width Fixes & Variation Selectors
    status: complete
  - id: "08.4"
    title: Grapheme-Aware Selection & Extraction
    status: complete
  - id: "08.5"
    title: Tab Title Width Fix
    status: complete
  - id: "08.6"
    title: Tests
    status: complete
  - id: "08.7"
    title: Completion Checklist
    status: not-started
---

# Section 08: Unicode & Graphemes

**Status:** Not Started
**Goal:** Correct Unicode rendering and input handling: render combining marks that
are already stored, buffer incoming codepoints into grapheme clusters, fix width
calculation bugs, handle ZWJ/variation selectors, and make selection grapheme-aware.

**Inspired by:**
- Ghostty's grapheme handling with UAX #29 state machine and out-of-band grapheme map
- Alacritty's `unicode-width` + `CellExtra.zerowidth` approach
- WezTerm's extensive grapheme and emoji support

**Current state (what already works):**
- `unicode-width` used in `term_handler.rs:93` to route chars by width (0/1/2)
- Wide chars (width 2) handled correctly: `WIDE_CHAR` + `WIDE_CHAR_SPACER` flags,
  `LEADING_WIDE_CHAR_SPACER` at line boundary, reflow preserves wide chars
- Zero-width chars stored in `CellExtra.zerowidth: Vec<char>` via `push_zerowidth()`
- `selection.rs:extract_text()` includes zerowidth chars in clipboard output
- `unicode-segmentation` imported in Cargo.toml but **unused**

**Current gaps:**
- **Combining marks stored but NOT RENDERED** (`render.rs:612-617` only looks at `cell.c`,
  ignores `cell.zerowidth()` — accents, diacritics are invisible)
- **No grapheme cluster input buffering** — each codepoint dispatched individually, so
  ZWJ sequences and multi-codepoint emoji are split across cells or attached ad-hoc
- **Tab title truncation uses `title.len()` (byte length)** instead of display width
  (`tab_bar.rs:219`) — CJK/emoji titles overflow
- **No variation selector handling** — VS15 (text) and VS16 (emoji presentation) not
  recognized, width may be wrong for emoji with selectors
- **Selection not grapheme-aware** — word boundaries use per-char classification, not
  grapheme clusters; backspace doesn't erase complete grapheme
- **Ambiguous-width characters** not configurable (always width 1)

**Approach:** Follow Alacritty's simpler model (keep `CellExtra.zerowidth` storage,
render combined glyphs) rather than Ghostty's more complex out-of-band grapheme map.
Use `unicode-segmentation` crate (already in deps) for grapheme break detection.

---

## 08.1 Render Combining Marks

**Priority: HIGH** — This is the most impactful fix. Combining marks are already stored
in cells, they just need to be drawn.

Currently `render.rs:612-617`:
```rust
if cell.c == ' ' || cell.c == '\0' {
    continue;
}
let style = FontStyle::from_cell_flags(cell.flags);
if let Some((metrics, bitmap)) = glyphs.get(cell.c, style) {
    // render single char only
```

### Tasks:

- [ ] Build the full grapheme string from cell data before rasterizing:
  ```rust
  // After the space/null check:
  let mut grapheme = String::with_capacity(8);
  grapheme.push(cell.c);
  for &zw in cell.zerowidth() {
      grapheme.push(zw);
  }
  ```

- [ ] Add `FontSet::get_grapheme(grapheme: &str, style: FontStyle)` method:
  - Cache key: `(String, FontStyle)` or hash of the grapheme + style
  - Rasterize with fontdue using the full grapheme string if font supports it
  - fontdue's `rasterize()` takes a single `char` — for multi-codepoint graphemes,
    two approaches:
    1. **Simple (do first):** Render base char, then overlay combining marks at same
       position (fontdue renders each char independently, blend alpha bitmaps)
    2. **Proper (later, with HarfBuzz):** Use a shaper that handles the full sequence
       and returns positioned glyphs

- [ ] **Simple overlay approach for combining marks:**
  ```rust
  fn render_grapheme(glyphs: &mut FontSet, grapheme: &str, style: FontStyle,
                     buffer: &mut [u32], buf_w: usize, buf_h: usize,
                     gx: i32, gy: i32, fg_r: u32, fg_g: u32, fg_b: u32,
                     fg_u32: u32, synthetic: bool) {
      // 1. Render base character
      let base = grapheme.chars().next().unwrap();
      if let Some((metrics, bitmap)) = glyphs.get(base, style) {
          render_glyph(buffer, buf_w, buf_h, metrics, bitmap,
                       gx, gy, fg_r, fg_g, fg_b, fg_u32, synthetic);
      }
      // 2. Overlay each combining mark at same position
      for combining in grapheme.chars().skip(1) {
          if let Some((m, bm)) = glyphs.get(combining, style) {
              let cx = gx + m.xmin;
              let cy = gy; // Same baseline
              render_glyph(buffer, buf_w, buf_h, m, bm,
                           cx, cy, fg_r, fg_g, fg_b, fg_u32, false);
          }
      }
  }
  ```

- [ ] Update `render_grid()` to use `render_grapheme()` when `cell.zerowidth()` is
  non-empty, fall back to existing single-char path otherwise (hot path stays fast)

- [ ] Handle the common case: precomposed characters (NFC) like `é` (U+00E9) already
  work fine — they're single codepoints. The fix is for decomposed forms like
  `e` + `\u{0301}` (combining acute) which the VTE parser delivers as two codepoints.

**Ref:** fontdue doesn't do shaping — it renders individual glyphs. For proper
combining mark positioning, overlay at the same cell origin is a reasonable
approximation. HarfBuzz integration (Section 06.9 / future) would give perfect results.

---

## 08.2 Grapheme Cluster Input Buffering

**Priority: MEDIUM** — Needed for correct ZWJ emoji and complex scripts. Without this,
multi-codepoint sequences like `👩‍👩‍👧‍👦` get split across cells.

The VTE parser calls `input(c: char)` for each codepoint individually. We need to
detect grapheme cluster boundaries and buffer codepoints until the cluster is complete.

### How VTE delivers input:

For `👩‍👩‍👧‍👦` (family emoji), VTE calls:
1. `input('👩')` — U+1F469
2. `input('\u{200D}')` — ZWJ (width 0, stored as zerowidth)
3. `input('👩')` — U+1F469 (new cell! wrong — should attach to previous)
4. `input('\u{200D}')` — ZWJ
5. `input('👧')` — U+1F467 (new cell! wrong)
6. `input('\u{200D}')` — ZWJ
7. `input('👦')` — U+1F466 (new cell! wrong)

Result: 4 cells used instead of 1 (width 2) cluster.

### Tasks:

- [ ] Add grapheme buffering state to `TermHandler`:
  ```rust
  struct GraphemeBuffer {
      codepoints: Vec<char>,
      pending: bool,
  }
  ```

- [ ] In `input(c)`, check if `c` continues the current grapheme cluster:
  ```rust
  use unicode_segmentation::UnicodeSegmentation;

  fn input(&mut self, c: char) {
      let c = self.charset.map(c);

      if self.grapheme_buf.pending {
          // Check if this codepoint extends the current cluster
          if is_grapheme_extend(c) || is_zwj_continuation(c, &self.grapheme_buf) {
              self.grapheme_buf.codepoints.push(c);
              return; // Buffer more
          } else {
              // Previous cluster is complete — flush it
              self.flush_grapheme();
              // Fall through to start new cluster
          }
      }

      // Start new potential cluster
      let width = UnicodeWidthChar::width(c);
      match width {
          Some(0) => {
              // Zero-width char with no pending cluster: attach to previous cell
              // (existing behavior for lone combining marks)
              self.attach_zerowidth(c);
              return;
          }
          _ => {
              self.grapheme_buf.codepoints.clear();
              self.grapheme_buf.codepoints.push(c);
              self.grapheme_buf.pending = true;
              // Don't commit yet — next char might extend
          }
      }
  }
  ```

- [ ] `flush_grapheme()` commits the buffered cluster to the grid:
  ```rust
  fn flush_grapheme(&mut self) {
      let cps = &self.grapheme_buf.codepoints;
      if cps.is_empty() { return; }

      let base = cps[0];
      let base_width = UnicodeWidthChar::width(base).unwrap_or(1);

      let grid = self.active_grid_mut();
      if base_width == 2 {
          grid.put_wide_char(base);
      } else {
          grid.put_char(base);
      }

      // Attach remaining codepoints as zerowidth to the base cell
      if cps.len() > 1 {
          let col = grid.cursor.col.saturating_sub(if base_width == 2 { 2 } else { 1 });
          let row = grid.cursor.row;
          for &cp in &cps[1..] {
              grid.row_mut(row)[col].push_zerowidth(cp);
          }
      }

      self.grapheme_buf.pending = false;
      self.grapheme_buf.codepoints.clear();
  }
  ```

- [ ] Detect grapheme extension using `unicode-segmentation`:
  ```rust
  fn is_grapheme_extend(c: char) -> bool {
      // Zero-width chars: combining marks, ZWJ, variation selectors
      matches!(UnicodeWidthChar::width(c), Some(0) | None)
  }

  fn is_zwj_continuation(c: char, buf: &GraphemeBuffer) -> bool {
      // After ZWJ (U+200D), the next non-zero-width char is part of the cluster
      buf.codepoints.last() == Some(&'\u{200D}') && UnicodeWidthChar::width(c).is_some()
  }
  ```

- [ ] **Flushing edge cases:**
  - Flush on `linefeed()`, `carriage_return()`, `backspace()`, `goto()`, any cursor
    movement — any non-character event means the cluster is done
  - Flush on erase/delete operations
  - Flush before reading cursor position for DSR
  - Add `self.flush_grapheme()` call at the start of all cursor-movement Handler methods

- [ ] **Safety timeout:** If grapheme buffer has been pending for more than ~10
  codepoints, flush anyway (malformed input protection)

**Ref:** Ghostty `terminal/grapheme.zig` — state machine approach.
Simpler alternative: just use `unicode-segmentation::grapheme_indices` but that
requires the full string up front, which we don't have (streaming input).

---

## 08.3 Width Fixes & Variation Selectors

**Priority: MEDIUM** — Correct width is needed for proper cursor positioning.

### Tasks:

- [ ] **Variation Selector 15 (VS15, U+FE0E) — text presentation:**
  - When VS15 follows an emoji, force width to 1 (text-style rendering)
  - Store VS15 as zerowidth attached to previous cell
  - If base char was already placed as width-2, this is complex — may need to
    retroactively change the cell (remove spacer, adjust cursor)
  - **Simpler approach:** Handle in grapheme buffer — when VS15 is in the cluster,
    use width 1 for `flush_grapheme()` output

- [ ] **Variation Selector 16 (VS16, U+FE0F) — emoji presentation:**
  - When VS16 follows a text char, force width to 2 (emoji-style rendering)
  - Store VS16 as zerowidth
  - Handle via grapheme buffer: when VS16 is in cluster, use width 2

- [ ] **Emoji modifier sequences** (skin tone modifiers U+1F3FB-1F3FF):
  - Modifier follows emoji base, becomes part of grapheme cluster
  - Combined sequence is width 2
  - Handle in grapheme buffer (modifier is zero-width, attaches to base)

- [ ] **Ambiguous-width characters:**
  - East Asian Ambiguous Width characters (UAX #11) can be width 1 or 2
  - `unicode-width` defaults to width 1 (Western convention)
  - Add `ambiguous_width: usize` (1 or 2) to `Grid` or app config
  - Override width for ambiguous characters when set to 2
  - Characters affected: Greek letters, Cyrillic, box-drawing, some symbols
  - **Defer to Section 13 (Configuration)** — for now, keep width 1 default

- [ ] **Width edge case: Regional Indicator symbols** (flags):
  - Two Regional Indicator letters (U+1F1E6-U+1F1FF) form a flag emoji
  - Each indicator is width 1 standalone, but pair is width 2
  - Handle in grapheme buffer: detect RI pairs, output as width-2 cluster

---

## 08.4 Grapheme-Aware Selection & Extraction

**Priority: LOW** — Selection already works for most cases. This is polish.

### Tasks:

- [ ] **Word boundary detection** should respect grapheme clusters:
  - `word_boundaries()` in `selection.rs:187-195` classifies by individual `char`
  - Should classify by grapheme cluster (entire emoji is one "word unit")
  - When double-clicking an emoji, select the whole grapheme (base + modifiers)

- [ ] **Selection hit-testing** with wide chars:
  - `contains(row, col)` should account for wide char spacers
  - Clicking on a `WIDE_CHAR_SPACER` cell should select the wide char's first cell
  - Already partially handled by `Side::Left/Right` but verify edge cases

- [ ] **Backspace over grapheme clusters:**
  - Currently backspace in grid moves cursor back one column
  - For a ZWJ sequence occupying 2 cells (width 2), backspace should erase the
    entire cluster, not just one cell
  - This is primarily a PTY-level concern (the shell handles backspace semantics)
  - Terminal side: when overwriting a cell that has zerowidth chars, clear them

- [ ] **Text extraction preserves grapheme order:**
  - `extract_text()` already appends zerowidth chars after base char ✅
  - Verify the order matches the original input order
  - Verify ZWJ sequences round-trip correctly (write to PTY → read back → select → copy)

---

## 08.5 Tab Title Width Fix

**Priority: HIGH** — Known bug, simple fix.

`tab_bar.rs:219` uses `title.len()` (byte count) instead of display width:

```rust
// CURRENT (buggy):
let display_title: String = if title.len() > max_text_chars {

// FIXED:
use unicode_width::UnicodeWidthStr;
let title_width = UnicodeWidthStr::width(title.as_str());
let display_title: String = if title_width > max_text_chars {
```

### Tasks:

- [ ] Replace `title.len()` with `UnicodeWidthStr::width()` in `tab_bar.rs`
- [ ] Fix truncation to be width-aware:
  ```rust
  fn truncate_to_width(s: &str, max_width: usize) -> String {
      let mut width = 0;
      let mut result = String::new();
      for c in s.chars() {
          let cw = UnicodeWidthChar::width(c).unwrap_or(0);
          if width + cw + 1 > max_width {  // +1 for ellipsis
              result.push('\u{2026}');       // …
              return result;
          }
          width += cw;
          result.push(c);
      }
      result
  }
  ```
- [ ] Handle edge case: wide char at truncation boundary (don't split a CJK char)
- [ ] `render_text()` in `render.rs:651` also advances cursor by `cell_width` per
  char — should advance by display width for wide chars (currently only used for
  tab titles, which are usually ASCII, but fix for correctness)

---

## 08.6 Tests

### Unit tests:

- [ ] **Combining mark storage:**
  ```rust
  #[test]
  fn combining_mark_stored() {
      // e + combining acute
      grid.put_char('e');
      let col = grid.cursor.col - 1;
      grid.row_mut(0)[col].push_zerowidth('\u{0301}');
      assert_eq!(grid.rows[0][col].c, 'e');
      assert_eq!(grid.rows[0][col].zerowidth(), &['\u{0301}']);
  }
  ```

- [ ] **Wide char at line boundary:**
  ```rust
  #[test]
  fn wide_char_at_end_of_line() {
      let mut grid = Grid::new(5, 2);  // 5 cols
      grid.cursor.col = 4;             // Last column
      grid.put_wide_char('漢');
      // Should wrap: col 4 gets LEADING_WIDE_CHAR_SPACER, char goes to next line
      assert!(grid.rows[0][4].flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER));
      assert_eq!(grid.rows[1][0].c, '漢');
      assert!(grid.rows[1][0].flags.contains(CellFlags::WIDE_CHAR));
      assert!(grid.rows[1][1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
  }
  ```

- [ ] **Wide char overwrite clears spacer:**
  ```rust
  #[test]
  fn overwrite_wide_char() {
      let mut grid = Grid::new(10, 2);
      grid.put_wide_char('漢');
      grid.cursor.col = 0;
      grid.put_char('a');
      assert_eq!(grid.rows[0][0].c, 'a');
      assert!(!grid.rows[0][1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
  }
  ```

- [ ] **Tab title truncation with CJK:**
  ```rust
  #[test]
  fn tab_title_truncate_cjk() {
      let title = "漢字テスト";  // 5 CJK chars, display width 10
      let truncated = truncate_to_width(title, 7);
      // Should fit 3 CJK chars (width 6) + ellipsis (width 1) = 7
      assert_eq!(truncated, "漢字テ…");
  }
  ```

- [ ] **Grapheme buffer ZWJ sequence** (after 08.2):
  ```rust
  #[test]
  fn zwj_emoji_single_cell() {
      // Family emoji: woman ZWJ woman ZWJ girl ZWJ boy
      // Should occupy one cell (width 2) with zerowidth chars
      feed_input(&mut handler, "👩\u{200D}👩\u{200D}👧\u{200D}👦");
      flush_grapheme(&mut handler);
      let cell = &grid.rows[0][0];
      assert_eq!(cell.c, '👩');
      assert!(cell.flags.contains(CellFlags::WIDE_CHAR));
      assert!(cell.zerowidth().len() >= 6);  // ZWJ + char * 3
  }
  ```

- [ ] **Selection extracts grapheme clusters correctly:**
  ```rust
  #[test]
  fn selection_extracts_combining_marks() {
      grid.put_char('e');
      grid.row_mut(0)[0].push_zerowidth('\u{0301}');
      let text = extract_text(&grid, /* full line selection */);
      assert_eq!(text, "é");  // NFC or decomposed, contains the accent
  }
  ```

---

## 08.7 Completion Checklist

- [ ] Combining marks (accents, diacritics) render visibly on base characters
- [ ] CJK characters display at double width correctly (already working, verified)
- [ ] Tab title truncation uses display width, not byte length
- [ ] Wide char at line boundary wraps correctly (already working, add tests)
- [ ] ZWJ emoji sequences render as single grapheme (08.2, width 2)
- [ ] Variation selectors (VS15/VS16) affect rendering width (08.3)
- [ ] Emoji with skin tone modifiers render correctly (08.2 + 08.3)
- [ ] Selection of wide characters selects the full character (not half)
- [ ] Selected text round-trips correctly (copy includes combining marks)
- [ ] Backspace over grapheme cluster erases the whole cluster (08.4)
- [ ] Box-drawing characters align properly at cell boundaries (already working)
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no new warnings
- [ ] `cargo test` — all tests pass including new Unicode tests
- [ ] No performance regression for ASCII-heavy workloads (hot path unchanged)

**Exit Criteria:** Combining marks render visibly. ZWJ emoji render as single
glyphs. Tab titles truncate correctly for CJK/emoji. Selection is grapheme-aware.
All width calculations use display width, not byte length.

---

## Implementation Order

1. **08.5** Tab title width fix (quick win, standalone)
2. **08.1** Combining mark rendering (high impact, already stored)
3. **08.6** Tests for existing wide char behavior (verify before changing)
4. **08.2** Grapheme cluster input buffering (most complex, enables ZWJ)
5. **08.3** Variation selectors & width fixes (builds on 08.2)
6. **08.4** Grapheme-aware selection (polish, after 08.2)

**Dependencies:**
- 08.1 and 08.5 are independent, can be done in any order
- 08.2 must come before 08.3 (VS handling uses grapheme buffer)
- 08.4 benefits from 08.2 (grapheme clusters defined)
- 08.6 tests should be added incrementally with each subsection
