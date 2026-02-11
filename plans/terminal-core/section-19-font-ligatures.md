---
section: "19"
title: Font Ligatures
status: not-started
goal: Support OpenType ligature rendering for programming fonts
sections:
  - id: "19.1"
    title: Text Shaping with rustybuzz
    status: not-started
  - id: "19.2"
    title: Ligature Detection & Rendering
    status: not-started
  - id: "19.3"
    title: Configuration
    status: not-started
  - id: "19.4"
    title: Completion Checklist
    status: not-started
---

# Section 19: Font Ligatures

**Status:** Not Started
**Goal:** Render font ligatures (==, =>, ->, !=, etc.) for programming fonts
like Fira Code, JetBrains Mono, Cascadia Code using proper text shaping.

**Why this matters:** Ligature support is the #1 reason people leave Alacritty.
Alacritty refused to implement it for 8+ years. Every other modern terminal
(Ghostty, Kitty, WezTerm, iTerm2) supports ligatures. Programming font ligatures
are a must-have for developer-focused terminals.

**Inspired by:**
- Ghostty: HarfBuzz on Linux, CoreText on macOS
- Kitty: HarfBuzz-based text shaping
- WezTerm: HarfBuzz with font fallback chain

**Current state:** fontdue rasterizes individual glyphs in `src/render.rs`.
The glyph atlas (`src/gpu/atlas.rs`) caches by `(char, FontStyle)` key. Each
cell is rendered independently — no text shaping. Ligature-capable fonts
(JetBrains Mono, Cascadia Code) render individual characters instead of
combined glyphs. No HarfBuzz, rustybuzz, or any shaping engine in the dependency
tree.

**Dependency choice:** `rustybuzz` (pure Rust port of HarfBuzz) is preferred
over `harfbuzz-sys` (C FFI bindings) because:
- No C toolchain dependency (important for cross-compilation from WSL)
- Same shaping quality as HarfBuzz (same algorithm, same tables)
- Well-maintained, used by cosmic-text and other Rust text stacks

---

## 19.1 Text Shaping with rustybuzz

Integrate a text shaping engine.

- [ ] Add `rustybuzz` dependency to Cargo.toml
- [ ] Create `src/shaping.rs` module:
  - [ ] `FontShaper` struct wrapping `rustybuzz::Face` instances
  - [ ] Load font face data from the same font files `FontSet` uses
  - [ ] `shape_run(text: &str, font_face: &Face, features: &[Feature]) -> Vec<ShapedGlyph>`
  - [ ] `ShapedGlyph { glyph_id: u16, cluster: u32, x_advance: i32, x_offset: i32, y_offset: i32 }`
- [ ] Text run segmentation:
  - [ ] Group consecutive cells with same font style into shaping runs
  - [ ] Break runs at:
    - [ ] Font style change (bold → regular)
    - [ ] Wide character (CJK) — always isolated
    - [ ] Explicit cell boundaries (cursor position must be correct)
    - [ ] Color changes (optional — could shape across color boundaries
      if only fg changes, but simpler to break)
  - [ ] Each run: extract text string from cell chars, shape, map back
- [ ] Glyph ID mapping:
  - [ ] fontdue rasterizes by `char`; rustybuzz produces glyph IDs
  - [ ] Need `fontdue::Font::rasterize_indexed(glyph_id, size)` (fontdue supports this)
  - [ ] Atlas key changes from `(char, FontStyle)` to `(GlyphId, FontStyle)`
  - [ ] Or: add a parallel atlas for shaped glyphs
- [ ] Cluster mapping (glyph ↔ cells):
  - [ ] A ligature like `=>` produces one glyph spanning cluster 0 and 1
  - [ ] Map: glyph 0 covers cells [0, 1], glyph 1 covers cells [2, 3], etc.
  - [ ] First cell in a ligature cluster gets the ligature glyph
  - [ ] Subsequent cells in the cluster render nothing (skip in instance buffer)

---

## 19.2 Ligature Detection & Rendering

Handle multi-cell ligatures in the rendering pipeline.

- [ ] Atlas changes for wide glyphs:
  - [ ] Ligature glyphs may be 2x, 3x, or more cell widths
  - [ ] Atlas entry needs actual glyph width (not cell_width)
  - [ ] Shelf packing must handle variable-width entries
  - [ ] Position ligature glyph at the x-offset of the first cell in the cluster
- [ ] Renderer changes (`src/gpu/renderer.rs`):
  - [ ] `build_grid_instances()` currently iterates col-by-col
  - [ ] With shaping: iterate shaped runs, emit one instance per glyph
  - [ ] Ligature glyph instance: positioned at first cell, width = N * cell_width
  - [ ] Subsequent cells in cluster: emit background only, no foreground glyph
- [ ] Cursor rendering within ligatures:
  - [ ] Cursor on any cell of a ligature highlights that cell's column position
  - [ ] Don't break the ligature visually — draw cursor overlay on top
  - [ ] Block cursor: filled rectangle at cursor column, ligature glyph still visible
  - [ ] Bar cursor: thin bar at cursor column (unambiguous)
- [ ] Selection within ligatures:
  - [ ] Selection is still per-cell (characters, not glyphs)
  - [ ] Selection highlight drawn per-cell, independent of ligature rendering
  - [ ] When selection boundary falls within a ligature, render ligature
    with partial selection highlight (some cells highlighted, others not)
- [ ] Line invalidation:
  - [ ] When any cell in a shaping run changes, re-shape the entire run
  - [ ] Cache shaped results per row: `HashMap<row_idx, Vec<ShapedRun>>`
  - [ ] Invalidate row's shaped cache when row is marked dirty
  - [ ] Shaped cache cleared on: font change, resize, full redraw
- [ ] Performance:
  - [ ] Only shape rows that are dirty (combine with damage tracking 15.1)
  - [ ] Shaping is ~10x faster than rasterization — not the bottleneck
  - [ ] ASCII-only rows can skip shaping (no ligatures possible without
    multi-char sequences)

---

## 19.3 Configuration

User control over ligatures.

- [ ] Config: `font.ligatures = true | false` (default: true)
  - [ ] When false: skip shaping entirely, render char-by-char (current behavior)
  - [ ] Hot-reload: toggling ligatures triggers full re-shape + redraw
- [ ] Config: `font.features` — list of OpenType features to enable
  ```toml
  [font]
  features = ["calt", "liga"]  # default
  # features = ["calt", "liga", "dlig"]  # include discretionary
  # features = ["-calt"]  # disable contextual alternates
  ```
  - [ ] Map to `rustybuzz::Feature` with tag and value
  - [ ] Default: `calt` (contextual alternates) + `liga` (standard ligatures)
  - [ ] `dlig` = discretionary ligatures (optional, some fonts have extra)
- [ ] Per-feature disable:
  - [ ] Prefix `-` to disable: `"-liga"` disables standard ligatures
  - [ ] This lets users keep `calt` but not `liga`, or vice versa
- [ ] Fallback: if rustybuzz not available or fails, render as individual
  glyphs (graceful degradation to current behavior)

---

## 19.4 Completion Checklist

- [ ] rustybuzz integrated and shaping runs on each dirty row
- [ ] Fira Code ligatures render correctly (==, =>, ->, !=, >=, <=, etc.)
- [ ] JetBrains Mono ligatures render correctly
- [ ] Cascadia Code ligatures render correctly
- [ ] Cursor navigates correctly through ligature cells
- [ ] Selection works correctly across ligature boundaries
- [ ] Ligatures can be disabled via config (`font.ligatures = false`)
- [ ] OpenType features configurable (`font.features`)
- [ ] No performance regression on non-ligature text
- [ ] Ligatures break correctly at style boundaries (e.g., colored `=` + `>`)
- [ ] Shaped glyph cache invalidated on font change, resize, cell edit
- [ ] Atlas handles multi-cell-width glyph entries

**Exit Criteria:** Programming fonts with ligatures render their combined glyphs
correctly, and ligatures can be toggled on/off in config.
