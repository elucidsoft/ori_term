---
section: "19"
title: Font Ligatures
status: not-started
goal: Support OpenType ligature rendering for programming fonts
sections:
  - id: "19.1"
    title: Text Shaping with HarfBuzz
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

**Current state:** fontdue rasterizes individual glyphs. No text shaping — each
cell is rendered independently. Ligature-capable fonts render individual characters
instead of combined glyphs.

---

## 19.1 Text Shaping with HarfBuzz

Integrate a text shaping engine.

- [ ] Add `harfbuzz` or `rustybuzz` dependency (rustybuzz is pure Rust, no C deps)
- [ ] Create shaper module:
  - [ ] `FontShaper` struct wrapping the shaping engine
  - [ ] `shape_line(cells: &[Cell], font: &Font) -> Vec<ShapedGlyph>`
  - [ ] `ShapedGlyph { glyph_id, x_advance, x_offset, y_offset, cluster }`
- [ ] Shape text in runs:
  - [ ] Group consecutive cells with same font style into shaping runs
  - [ ] Break runs at: style changes, explicit cell boundaries, wide chars
  - [ ] Feed each run through the shaper
- [ ] Map shaped glyphs back to cell positions:
  - [ ] A ligature produces one glyph spanning multiple cells
  - [ ] First cell gets the ligature glyph, subsequent cells render nothing
  - [ ] Track cluster mapping for cursor positioning within ligatures

---

## 19.2 Ligature Detection & Rendering

Handle multi-cell ligatures in the rendering pipeline.

- [ ] Glyph atlas changes:
  - [ ] Ligature glyphs may be wider than one cell
  - [ ] Atlas entries need actual glyph dimensions, not just cell_width
  - [ ] Render ligature glyph with full width, position at first cell
- [ ] Cursor rendering within ligatures:
  - [ ] Cursor on any cell of a ligature highlights that cell position
  - [ ] Don't break the ligature visually — draw cursor overlay
- [ ] Selection within ligatures:
  - [ ] Selection is still per-cell (characters, not glyphs)
  - [ ] May need to re-shape when selection splits a ligature
- [ ] Line invalidation:
  - [ ] When any cell in a ligature run changes, re-shape the entire run
  - [ ] Cache shaped results per row, invalidate on row dirty

---

## 19.3 Configuration

User control over ligatures.

- [ ] Config: `font_ligatures = true | false` (default: true)
- [ ] Config: `font_features = "calt", "liga", "dlig"` — OpenType features to enable
  - [ ] Default: `calt` + `liga` (standard ligatures)
  - [ ] `dlig` = discretionary ligatures (optional)
- [ ] Allow disabling specific ligatures (e.g., keep `=>` but not `==`)
  - [ ] Via OpenType feature variation axis if font supports it
- [ ] Fallback: if shaping engine unavailable, render as individual glyphs (current behavior)

---

## 19.4 Completion Checklist

- [ ] Fira Code ligatures render correctly (==, =>, ->, !=, >=, <=, etc.)
- [ ] JetBrains Mono ligatures render correctly
- [ ] Cascadia Code ligatures render correctly
- [ ] Cursor navigates correctly through ligature cells
- [ ] Selection works correctly across ligature boundaries
- [ ] Ligatures can be disabled via config
- [ ] No performance regression on non-ligature text
- [ ] Ligatures break correctly at style boundaries (e.g., colored `=` + `>`)

**Exit Criteria:** Programming fonts with ligatures render their combined glyphs
correctly, and ligatures can be toggled on/off in config.
