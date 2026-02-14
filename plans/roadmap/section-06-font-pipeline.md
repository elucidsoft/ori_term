---
section: 6
title: Font Pipeline + Advanced Glyph Rendering
status: not-started
tier: 2
goal: "Full font pipeline: multi-face loading, rustybuzz shaping, ligatures, fallback chain, built-in glyphs, color emoji, advanced atlas"
sections:
  - id: "6.1"
    title: Multi-Face Font Loading
    status: not-started
  - id: "6.2"
    title: Fallback Chain + Cap-Height Normalization
    status: not-started
  - id: "6.3"
    title: Run Segmentation
    status: not-started
  - id: "6.4"
    title: Rustybuzz Text Shaping
    status: not-started
  - id: "6.5"
    title: Ligature + Multi-Cell Glyph Handling
    status: not-started
  - id: "6.6"
    title: Combining Marks + Zero-Width Characters
    status: not-started
  - id: "6.7"
    title: OpenType Feature Control
    status: not-started
  - id: "6.8"
    title: Advanced Atlas (Guillotine + LRU + Multi-Page)
    status: not-started
  - id: "6.9"
    title: Built-in Geometric Glyphs
    status: not-started
  - id: "6.10"
    title: Color Emoji
    status: not-started
  - id: "6.11"
    title: Synthetic Bold + Variable Weight
    status: not-started
  - id: "6.12"
    title: Text Decorations
    status: not-started
  - id: "6.13"
    title: UI Text Shaping
    status: not-started
  - id: "6.14"
    title: Pre-Caching + Performance
    status: not-started
  - id: "6.15"
    title: Section Completion
    status: not-started
---

# Section 06: Font Pipeline + Advanced Glyph Rendering

**Status:** 📋 Planned
**Goal:** Replace Section 04's basic per-character rasterization with the full font pipeline from the old prototype: rustybuzz shaping, ligature support, multi-face fallback with cap-height normalization, built-in geometric glyphs, color emoji, advanced atlas packing, and all text decorations.

**Crate:** `oriterm` (binary)
**Dependencies:** `swash`, `rustybuzz`, `dwrote` (Windows)
**Reference:** `_old/src/font/`, `_old/src/gpu/atlas.rs`, `_old/src/gpu/builtin_glyphs.rs`, `_old/src/gpu/render_grid.rs`

**Prerequisite:** Section 04 complete (basic terminal rendering working with simple per-char glyphs). This section replaces the basic font path with the full pipeline.

---

## 6.1 Multi-Face Font Loading

Load all 4 style variants (Regular, Bold, Italic, BoldItalic) from the primary font family.

**File:** `oriterm/src/font/collection.rs`

**Reference:** `_old/src/font/collection.rs`

- [ ] `FaceData` struct
  - [ ] Fields:
    - `bytes: Arc<Vec<u8>>` — raw font file bytes (shared across variants from same file)
    - `face_index: u32` — index within .ttc collection
    - `offset: u32` — byte offset to font table directory
    - `cache_key: swash::CacheKey` — swash cache identifier
- [ ] `FaceIdx` newtype — `pub struct FaceIdx(pub u16)`
  - [ ] 0–3: primary styles (Regular=0, Bold=1, Italic=2, BoldItalic=3)
  - [ ] 4+: fallback fonts in priority order
- [ ] `FontCollection` expanded fields:
  - [ ] `primary: [Option<FaceData>; 4]` — Regular, Bold, Italic, BoldItalic
  - [ ] `has_variant: [bool; 4]` — true = real font file, false = fallback to Regular
  - [ ] `font_paths: [Option<PathBuf>; 4]`
  - [ ] `weight: u16` — CSS weight (100–900, default 400)
- [ ] Loading pipeline:
  - [ ] Load Regular (required — fail if missing)
  - [ ] Try loading Bold, Italic, BoldItalic from same family
  - [ ] If variant not found: `has_variant[i] = false` (will use Regular + synthetic styling)
  - [ ] Compute cell metrics from Regular face (cell_width from 'M' advance, cell_height from ascent + descent)
- [ ] Platform discovery (`font/discovery.rs`):
  - [ ] Windows (dwrote): enumerate via DirectWrite API by family name
  - [ ] Linux: scan `~/.local/share/fonts/`, `/usr/share/fonts/`, `/usr/local/share/fonts/`
  - [ ] Family search order: user-configured > JetBrains Mono > Cascadia Code > Consolas > Courier New
- [ ] `find_face_for_char(&self, ch: char, preferred_style: GlyphStyle) -> Option<FaceIdx>`
  - [ ] Try preferred style in primary
  - [ ] Fall back to Regular in primary
  - [ ] Fall back through fallback chain
  - [ ] Return None only if .notdef everywhere
- [ ] **Tests**:
  - [ ] Load a system font, all 4 variants attempted
  - [ ] `find_face_for_char('A', Bold)` returns Bold face if available
  - [ ] `find_face_for_char('A', Bold)` returns Regular if no Bold face
  - [ ] Unknown char falls to fallback chain

---

## 6.2 Fallback Chain + Cap-Height Normalization

Fallback fonts for characters missing from the primary (CJK, symbols, emoji). Visual consistency via cap-height normalization.

**File:** `oriterm/src/font/collection.rs` (continued)

**Reference:** `_old/src/font/collection.rs` (cap_height_px, FallbackMeta)

- [ ] `FallbackMeta` struct
  - [ ] Fields:
    - `features: Vec<rustybuzz::Feature>` — per-fallback OpenType features (override collection defaults)
    - `scale_factor: f32` — cap-height normalization ratio
    - `size_offset: f32` — user-configured size offset in points
- [ ] Fallback loading:
  - [ ] `fallbacks: Vec<FaceData>` — priority-ordered fallback fonts
  - [ ] `fallback_meta: Vec<FallbackMeta>` — per-fallback metadata (1:1 with fallbacks)
  - [ ] User-configured fallbacks loaded first (from config TOML)
  - [ ] System-discovered fallbacks loaded after
  - [ ] Lazy loading: `ensure_fallbacks_loaded()` called once on first use
- [ ] Cap-height normalization:
  - [ ] `cap_height_px(bytes, face_index, size) -> f32`
    - [ ] Read OS/2 table `sCapHeight` field via rustybuzz Face
    - [ ] If missing: estimate as `ascender * 0.75`
    - [ ] Convert from font units: `cap_units / upem * size`
  - [ ] `primary_cap_height_px: f32` — computed from Regular at load time
  - [ ] Per-fallback: `scale_factor = primary_cap_height / fallback_cap_height`
  - [ ] Effective size: `base_size * scale_factor + size_offset`
  - [ ] **Why:** Noto Sans CJK looks tiny next to JetBrains Mono at same pt size. Normalizing by cap-height makes glyphs visually consistent.
- [ ] `effective_size(&self, face_idx: FaceIdx) -> f32`
  - [ ] Primary faces: base size
  - [ ] Fallback faces: `base_size * meta.scale_factor + meta.size_offset`
- [ ] User-configurable per-fallback:
  ```toml
  [[font.fallback]]
  family = "Noto Sans CJK"
  features = ["-liga"]
  size_offset = -2.0
  ```
- [ ] **Tests**:
  - [ ] Fallback chain resolves CJK char to CJK font
  - [ ] Cap-height scale factor computed correctly (known font pair)
  - [ ] Effective size for fallback differs from primary
  - [ ] User size_offset applied

---

## 6.3 Run Segmentation

Break a terminal row into shaping runs. Each run is a contiguous sequence of characters that can be shaped together (same font face, no breaks).

**File:** `oriterm/src/font/shaper.rs`

**Reference:** `_old/src/font/shaper.rs` (prepare_line)

- [ ] `ShapingRun` struct
  - [ ] Fields:
    - `text: String` — base characters + combining marks for this run
    - `face_idx: FaceIdx` — which font face to shape with
    - `col_start: usize` — grid column where run starts
    - `byte_to_col: Vec<usize>` — maps byte offset in `text` → grid column
  - [ ] byte_to_col is critical for mapping rustybuzz cluster indices back to grid positions
- [ ] `prepare_line(row: &[Cell], cols: usize, collection: &FontCollection, runs: &mut Vec<ShapingRun>)`
  - [ ] Iterate cells left to right
  - [ ] Skip `WIDE_CHAR_SPACER` cells (they're part of the preceding wide char)
  - [ ] For each cell:
    - [ ] Determine face via `find_face_for_char(cell.ch, style_from_flags(cell.flags))`
    - [ ] If face differs from current run, or cell is space/null/builtin: start new run
    - [ ] Append `cell.ch` to current run's text
    - [ ] Record byte offset → column mapping
    - [ ] Append zero-width characters (combining marks) from cell at same column mapping
  - [ ] Run breaks on:
    - [ ] Space (' ') or null ('\0')
    - [ ] Font face change (different glyph found in different face)
    - [ ] Built-in glyph character (box drawing, blocks, braille, powerline)
    - [ ] Wide char spacer
  - [ ] Runs reuse a scratch `Vec<ShapingRun>` (cleared + refilled each frame, not reallocated)
- [ ] **Tests**:
  - [ ] `"hello world"` → two runs: "hello" (face 0), "world" (face 0) — space breaks runs
  - [ ] `"hello你好"` → two runs if CJK resolves to different face
  - [ ] `"a\u{0301}"` (a + combining accent) → single run with "á" text, byte_to_col maps both to same column
  - [ ] `"━"` (box drawing) → no run (handled by builtin glyph system)

---

## 6.4 Rustybuzz Text Shaping

Shape each run through rustybuzz to produce positioned glyphs with correct ligature substitution.

**File:** `oriterm/src/font/shaper.rs` (continued)

**Reference:** `_old/src/font/shaper.rs` (shape_prepared_runs)

- [ ] Two-phase API:
  - [ ] Phase 1: `prepare_line()` — segment into runs (immutable, reuses scratch buffers)
  - [ ] Phase 2: `shape_prepared_runs()` — shape each run (needs rustybuzz Face references)
  - [ ] **Why two phases?** Create rustybuzz `Face` objects once per frame, reuse across all rows. Faces borrow font bytes, so they must outlive shaping calls.
- [ ] `shape_prepared_runs(runs: &[ShapingRun], faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<ShapedGlyph>)`
  - [ ] For each run:
    - [ ] Create `rustybuzz::UnicodeBuffer`, push run's text
    - [ ] Set direction: `LeftToRight` (terminal is always LTR)
    - [ ] Get features for this face: `collection.features_for_face(run.face_idx)`
    - [ ] Call `rustybuzz::shape(face, &features, buffer)`
    - [ ] Extract `glyph_infos()` and `glyph_positions()`
    - [ ] Scale: `effective_size / upem`
    - [ ] For each (info, position) pair:
      - [ ] Map `info.cluster` (byte offset) → grid column via `run.byte_to_col`
      - [ ] Compute `col_span` from advance: `(x_advance * scale / cell_width).round().max(1)`
      - [ ] Emit `ShapedGlyph`
- [ ] `ShapedGlyph` struct
  - [ ] Fields:
    - `glyph_id: u16` — rustybuzz glyph ID (NOT codepoint — this is the shaped result)
    - `face_idx: FaceIdx` — which face this was shaped from
    - `col_start: usize` — first grid column this glyph occupies
    - `col_span: usize` — how many columns (1 = normal, 2+ = ligature or wide char)
    - `x_offset: f32` — shaper x positioning offset (pixels)
    - `y_offset: f32` — shaper y positioning offset (pixels)
- [ ] Output reuses scratch `Vec<ShapedGlyph>` (cleared + refilled each row)
- [ ] **Tests**:
  - [ ] `"hello"` → 5 glyphs, each col_span=1
  - [ ] `"=>"` with ligature-supporting font → 1 glyph, col_span=2
  - [ ] `"fi"` with liga feature → 1 glyph (fi ligature), col_span=2
  - [ ] `"好"` (wide char) → 1 glyph, col_span=2
  - [ ] CJK char → shaped from fallback face, correct face_idx

---

## 6.5 Ligature + Multi-Cell Glyph Handling

Map shaped glyphs back to grid columns. Ligatures span multiple columns — only the first column renders the glyph.

**File:** `oriterm/src/gpu/render_grid.rs` (rendering integration)

- [ ] Column → glyph mapping:
  - [ ] `col_glyph_map: Vec<Option<usize>>` — maps column index → index in shaped glyphs vec
  - [ ] For each shaped glyph: `col_glyph_map[glyph.col_start] = Some(glyph_index)`
  - [ ] Subsequent columns of a ligature (col_start+1, col_start+2, ...) remain `None`
  - [ ] During rendering: if `col_glyph_map[col]` is `Some(i)` → render glyph; if `None` → skip (continuation of ligature)
- [ ] Ligature background:
  - [ ] Background color for each column still rendered independently (cell-by-cell)
  - [ ] Only the foreground glyph spans multiple columns
- [ ] Ligature + selection interaction:
  - [ ] If selection covers part of a ligature, still render the full glyph
  - [ ] Selection highlighting applies to individual cells (not whole ligature)
- [ ] Ligature + cursor interaction:
  - [ ] Cursor on a ligature column renders on top of the glyph
  - [ ] Cursor rendering is per-cell, unaffected by glyph span
- [ ] **Tests**:
  - [ ] `"=>"` ligature: col 0 gets glyph, col 1 is None
  - [ ] Selection of col 1 of a ligature doesn't duplicate glyph
  - [ ] Mixed ligature + non-ligature on same line renders correctly

---

## 6.6 Combining Marks + Zero-Width Characters

Handle combining diacritics, ZWJ sequences, and other zero-width characters.

**Files:** `oriterm_core/src/cell.rs` (storage), `oriterm/src/font/shaper.rs` (shaping)

- [ ] Cell storage for zero-width characters:
  - [ ] Add to `CellExtra`: `zerowidth: Option<Vec<char>>` — combining marks attached to this cell
  - [ ] `Cell::push_zerowidth(&mut self, ch: char)` — add combining mark
  - [ ] `Cell::zerowidth(&self) -> &[char]` — get combining marks (empty slice if none)
  - [ ] Zero-width chars don't advance the cursor — they attach to the preceding cell
- [ ] VTE handler integration:
  - [ ] When `input(ch)` receives a character with `unicode_width == 0`:
    - [ ] Don't advance cursor
    - [ ] Push to previous cell's zerowidth list
- [ ] Shaping integration:
  - [ ] In `prepare_line()`: after appending base char, also append `cell.zerowidth()` chars to run text
  - [ ] All zero-width chars get same column mapping as their base char
  - [ ] Rustybuzz handles combining: base + accent → single positioned cluster
- [ ] Rendering:
  - [ ] Shaper produces multiple glyphs at same col_start (base + marks)
  - [ ] Each glyph rendered with its own x_offset/y_offset from shaper
  - [ ] Multiple glyphs at same column are all rendered (not just first)
- [ ] **Tests**:
  - [ ] `'e'` + `'\u{0301}'` (combining acute) → single shaping cluster at same column
  - [ ] `'a'` + `'\u{0308}'` (combining diaeresis) → 'ä' appearance
  - [ ] ZWJ sequence (e.g., family emoji): stored as base + zerowidth sequence
  - [ ] Width: combining marks don't advance cursor (width 0)

---

## 6.7 OpenType Feature Control

Collection-wide and per-fallback OpenType feature settings.

**File:** `oriterm/src/font/collection.rs` (continued)

- [ ] Collection-wide features:
  - [ ] `features: Vec<rustybuzz::Feature>` — applied to all primary faces
  - [ ] Default: `["liga", "calt"]` (standard ligatures + contextual alternates)
  - [ ] Parsed from config: `"liga"` → enable, `"-liga"` → disable
- [ ] Per-fallback features:
  - [ ] `FallbackMeta.features` — overrides collection defaults for specific fallback
  - [ ] Use case: disable ligatures for CJK fonts (`["-liga"]`)
- [ ] `features_for_face(&self, face_idx: FaceIdx) -> &[rustybuzz::Feature]`
  - [ ] Primary (0–3): return collection-wide features
  - [ ] Fallback (4+): return fallback-specific features
- [ ] Feature parsing:
  - [ ] `parse_features(input: &[&str]) -> Vec<rustybuzz::Feature>`
  - [ ] `"liga"` → `Feature { tag: tag!("liga"), value: 1, start: 0, end: u32::MAX }`
  - [ ] `"-dlig"` → `Feature { tag: tag!("dlig"), value: 0, start: 0, end: u32::MAX }`
- [ ] Config integration:
  ```toml
  [font]
  features = ["liga", "calt", "dlig"]
  ligatures = true  # Shorthand for liga + calt
  ```
- [ ] **Tests**:
  - [ ] Features parsed correctly: "liga" → value 1, "-liga" → value 0
  - [ ] Collection features applied during shaping
  - [ ] Fallback override: CJK font uses different features than primary

---

## 6.8 Advanced Atlas (Guillotine + LRU + Multi-Page)

Replace Section 04's simple shelf packing with the production atlas: guillotine packing, 2D texture array, LRU eviction.

**File:** `oriterm/src/gpu/atlas.rs`

**Reference:** `_old/src/gpu/atlas.rs`

- [ ] Guillotine rectangle packing:
  - [ ] `RectPacker` struct
    - [ ] `free_rects: Vec<Rect>` — available rectangles
    - [ ] `pack(w: u32, h: u32) -> Option<(u32, u32)>` — find best-short-side-fit
    - [ ] Split: remove chosen rect, create up to 2 children (horizontal or vertical split based on leftover shape)
    - [ ] Reset: clear to single full-page rect
  - [ ] **Why guillotine over shelf?** Better packing density for mixed glyph sizes (CJK large + Latin small + accent tiny)
- [ ] Multi-page texture array:
  - [ ] `GlyphAtlas.texture: wgpu::Texture` — `Texture2DArray` format
  - [ ] Page size: 2048×2048 (configurable, old app used 2048)
  - [ ] Max pages: 4 (= 16MB VRAM at R8Unorm)
  - [ ] Start with 1 page, grow on demand up to max
  - [ ] `pages: Vec<AtlasPage>` — per-page packing state + LRU frame counter
- [ ] LRU eviction:
  - [ ] Each page tracks `last_used_frame: u64`
  - [ ] When all pages full and new glyph needs space:
    - [ ] Find page with oldest `last_used_frame`
    - [ ] Reset that page's packer
    - [ ] Remove all cache entries pointing to that page
    - [ ] Re-insert the new glyph on the now-empty page
- [ ] Cache key: `(glyph_id: u16, face_idx: FaceIdx, size_q6: u32, collection_id: u8)`
  - [ ] `size_q6 = (size * 64.0).round() as u32` — 26.6 fixed-point for precise DPI-aware keying
  - [ ] `collection_id` discriminates grid font (0) vs UI font (1)
  - [ ] **Why Q6?** Prevents rounding collisions at fractional DPI: 13.95pt vs 14.05pt get distinct keys
- [ ] `get_or_insert_shaped(glyph_id, face_idx, size_q6, collection_id, rasterize_fn, queue) -> &AtlasEntry`
  - [ ] Check `shaped_entries` HashMap
  - [ ] If miss: call `rasterize_fn()` to get bitmap, upload to atlas, cache entry
  - [ ] Update page's `last_used_frame`
  - [ ] Return atlas entry (UV coordinates, metrics, page index)
- [ ] `AtlasEntry` struct (same as old):
  - [ ] `uv_pos: [f32; 2]`, `uv_size: [f32; 2]`, `metrics: GlyphMetrics`, `page: u32`
- [ ] `begin_frame()` — increment frame counter
- [ ] `clear()` — full atlas reset (called on font size change)
- [ ] **Tests**:
  - [ ] Guillotine packing: insert 100 varied-size rects, all find positions
  - [ ] Multi-page: fill page 0, overflow to page 1
  - [ ] LRU eviction: fill all 4 pages, insert new glyph → oldest page evicted
  - [ ] Cache hit: same key returns same entry
  - [ ] Q6 keying: slightly different sizes produce different keys

---

## 6.9 Built-in Geometric Glyphs

Pixel-perfect rendering for box drawing, block elements, braille, and powerline glyphs. Bypasses the font pipeline entirely — generated as GPU rectangles.

**File:** `oriterm/src/gpu/builtin_glyphs.rs`

**Reference:** `_old/src/gpu/builtin_glyphs.rs`

- [ ] `is_builtin(ch: char) -> bool` — fast check if character is handled by builtin system
- [ ] `draw_builtin_glyph(ch: char, x: f32, y: f32, w: f32, h: f32, fg: [f32; 4], instances: &mut InstanceWriter) -> bool`
  - [ ] Returns true if handled, false to fall through to font pipeline
- [ ] **Box Drawing** (U+2500–U+257F):
  - [ ] 128 characters, lookup table: `[left, right, up, down]` per char
  - [ ] Values: 0=none, 1=light (thin), 2=heavy (thick), 3=double
  - [ ] Render from cell center: horizontal segments left/right, vertical segments up/down
  - [ ] Line thickness: thin = `max(1.0, round(cell_width / 8.0))`, heavy = `thin * 3.0`
  - [ ] Double lines: two parallel lines with gap = `max(2.0, thin * 2.0)`
  - [ ] Segments connect cleanly at cell boundaries (critical for box drawing to look right)
- [ ] **Block Elements** (U+2580–U+259F):
  - [ ] Full block `█` (U+2588): entire cell filled
  - [ ] Upper half `▀` (U+2580): top half filled
  - [ ] Lower N/8 blocks (U+2581–U+2587): fractional heights from bottom
  - [ ] Left N/8 blocks (U+2589–U+258F): fractional widths from left
  - [ ] Shade blocks: light `░` (25% alpha), medium `▒` (50%), dark `▓` (75%)
  - [ ] Quadrant blocks (U+2596–U+259F): bitmask → fill selected quadrants
- [ ] **Braille** (U+2800–U+28FF):
  - [ ] 8-dot pattern in 2×4 grid
  - [ ] Character value encodes which dots are filled (8-bit bitmask)
  - [ ] Dot positions: 2 columns × 4 rows within cell
  - [ ] Render as small filled circles or rectangles at fractional cell positions
- [ ] **Powerline** (U+E0A0–U+E0D4):
  - [ ] Right-pointing solid triangle (U+E0B0): filled triangle, scanline rendered
  - [ ] Left-pointing solid triangle (U+E0B2): mirrored
  - [ ] Right-pointing thin arrow (U+E0B1): outline only
  - [ ] Left-pointing thin arrow (U+E0B3): mirrored outline
  - [ ] Branch symbol (U+E0A0): git branch icon
  - [ ] Rounded separators, flame shapes, etc.
- [ ] Integration with rendering loop:
  - [ ] Before font glyph lookup: `if builtin_glyphs::draw_builtin_glyph(...) { continue; }`
  - [ ] Built-in glyphs emit background-layer instances (opaque rectangles, not atlas-textured)
- [ ] **Tests**:
  - [ ] Box drawing: `'─'` (U+2500) produces horizontal line
  - [ ] Box drawing: `'┼'` (U+253C) produces cross
  - [ ] Block: `'█'` fills entire cell
  - [ ] Block: `'▄'` fills lower half
  - [ ] Braille: `'⠿'` (U+283F) fills all 6 main dots
  - [ ] Powerline: `''` (U+E0B0) produces triangle

---

## 6.10 Color Emoji

Support for color emoji rendering (CBDT/CBLC bitmap emoji or COLR/CPAL outline emoji).

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/gpu/atlas.rs`

- [ ] Rasterization with color source:
  - [ ] Swash `Render::new(&[Source::ColorOutline, Source::ColorBitmap, Source::Outline])`
  - [ ] `Format::Rgba` for color glyphs (4 bytes per pixel)
  - [ ] `Format::Alpha` for non-color fallback (1 byte per pixel)
  - [ ] Check render result: if RGBA → color glyph, if Alpha → normal glyph
- [ ] Atlas support for color glyphs:
  - [ ] Option A: Separate RGBA atlas (Rgba8Unorm texture) for color glyphs
  - [ ] Option B: Single atlas with mixed formats (more complex shader)
  - [ ] **Recommended: Option A** — separate atlas, separate pipeline pass
  - [ ] Color atlas bind group separate from grayscale atlas
- [ ] Rendering color glyphs:
  - [ ] Color glyphs render with their own colors (not tinted by fg_color)
  - [ ] Fragment shader: sample RGBA directly, blend with background
  - [ ] No foreground color multiplication (unlike grayscale glyphs)
- [ ] Emoji presentation:
  - [ ] Characters like U+2764 (❤) can be text or emoji presentation
  - [ ] VS15 (U+FE0E) forces text presentation
  - [ ] VS16 (U+FE0F) forces emoji presentation
  - [ ] Store variation selectors in cell's zerowidth list
  - [ ] During face resolution: check for VS16 → prefer color emoji font
- [ ] Fallback for emoji:
  - [ ] Windows: Segoe UI Emoji
  - [ ] Linux: Noto Color Emoji
  - [ ] These should be high-priority in fallback chain for emoji codepoints
- [ ] **Tests**:
  - [ ] Emoji character rasterizes as RGBA bitmap
  - [ ] Color glyph renders without fg tinting
  - [ ] VS16 forces emoji presentation
  - [ ] VS15 forces text presentation
  - [ ] Emoji fallback resolves to color emoji font

---

## 6.11 Synthetic Bold + Variable Weight

When a font doesn't have a Bold variant, simulate it. When a font has a weight axis, use it.

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/gpu/render_grid.rs`

**Reference:** `_old/src/font/collection.rs` (weight_variation_for)

- [ ] Variable font weight:
  - [ ] If font has `wght` axis: use font variations instead of separate Bold file
  - [ ] `weight_variation_for(face_idx: FaceIdx, weight: u16) -> Option<f32>`
    - [ ] Regular/Italic: use base weight (e.g., 400)
    - [ ] Bold/BoldItalic: `min(weight + 300, 900)` — CSS "bolder" algorithm
    - [ ] Fallbacks: `None` (use font's default weight)
  - [ ] Pass to swash: `scale_ctx.builder(face).variations(&[("wght", value)])`
- [ ] Synthetic bold (when `has_variant[Bold] == false` and no wght axis):
  - [ ] Render glyph at `(x, y)` AND `(x + 1.0, y)` — double-strike
  - [ ] Detectable during rendering: check `has_variant[1]` for bold
  - [ ] Only apply to primary face (fallbacks use their own weight)
- [ ] Integration with rendering loop:
  - [ ] If cell has BOLD flag and current face lacks real bold and lacks wght:
    - [ ] Push foreground glyph instance twice (at x and x+1)
- [ ] **Tests**:
  - [ ] Variable font: weight variation applied (visually thicker at wght=700)
  - [ ] Synthetic bold: two instances emitted for bold cell without bold face
  - [ ] Regular cells: single instance only

---

## 6.12 Text Decorations

All underline styles, strikethrough, hyperlink underline, URL hover underline.

**File:** `oriterm/src/gpu/render_grid.rs`

**Reference:** `_old/src/gpu/render_grid.rs` (underline/strikethrough sections)

- [ ] **Single underline** (CellFlags::UNDERLINE):
  - [ ] Solid line at `y = cell_bottom - 2px`, thickness = 1px
  - [ ] Spans cell width
- [ ] **Double underline** (CellFlags::DOUBLE_UNDERLINE):
  - [ ] Two solid lines: `y = cell_bottom - 2px` and `y = cell_bottom - 4px`
- [ ] **Curly underline** (CellFlags::CURLY_UNDERLINE):
  - [ ] Sine wave: `y = base_y + amplitude * sin(x * freq)`
  - [ ] Rendered as a sequence of short horizontal rectangles (1px tall) at computed y positions
  - [ ] Amplitude: ~2px, frequency: ~2π per cell_width
- [ ] **Dotted underline** (CellFlags::DOTTED_UNDERLINE):
  - [ ] Alternating 1px on, 1px off pattern
  - [ ] Phase reset at start of each cell
- [ ] **Dashed underline** (CellFlags::DASHED_UNDERLINE):
  - [ ] 3px on, 2px off pattern
- [ ] **Underline color** (SGR 58):
  - [ ] `cell.extra().underline_color` — resolved via palette
  - [ ] If present: use this color for underline
  - [ ] If absent: use foreground color
- [ ] **Strikethrough** (CellFlags::STRIKETHROUGH):
  - [ ] Solid line at `y = cell_top + cell_height / 2`, thickness = 1px
  - [ ] Color: foreground color
- [ ] **Hyperlink underline** (cell has hyperlink via OSC 8):
  - [ ] Dotted underline when not hovered
  - [ ] Solid underline when hovered (cursor over cell)
  - [ ] Color: foreground color (or a distinct link color)
- [ ] **URL hover underline** (implicitly detected URL):
  - [ ] Solid underline on hover
  - [ ] Only visible when Ctrl held + mouse over URL range
- [ ] All decorations emit background-layer instances (opaque rectangles)
- [ ] **Tests**:
  - [ ] Single underline: 1px line at correct y
  - [ ] Curly underline: wave shape (visual test)
  - [ ] Dotted: alternating pattern
  - [ ] Underline color: uses SGR 58 color when set
  - [ ] Strikethrough: centered horizontally

---

## 6.13 UI Text Shaping

Shape non-grid text (tab bar titles, search bar, status text) through rustybuzz without grid-column mapping.

**File:** `oriterm/src/font/shaper.rs` (additional function)

**Reference:** `_old/src/font/shaper.rs` (shape_text_string)

- [ ] `UiShapedGlyph` struct
  - [ ] Fields:
    - `glyph_id: u16`
    - `face_idx: FaceIdx`
    - `x_advance: f32` — absolute pixel advance (for cursor positioning)
    - `x_offset: f32`
    - `y_offset: f32`
  - [ ] No `col_start` / `col_span` — UI text is free-positioned, not grid-locked
- [ ] `shape_text_string(text: &str, faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<UiShapedGlyph>)`
  - [ ] Segment text into runs by font face (same as grid shaping)
  - [ ] Shape through rustybuzz
  - [ ] Emit glyphs with absolute x_advance (sum of advances = total text width)
  - [ ] Spaces: emit as advance-only (glyph_id = 0, advance = space width)
- [ ] `measure_text(text: &str, collection: &FontCollection) -> f32`
  - [ ] Sum x_advances for all glyphs → total pixel width
  - [ ] Used for tab bar layout, text truncation, centering
- [ ] Text truncation with ellipsis:
  - [ ] If text width > available width: truncate and append `…` (U+2026)
  - [ ] Binary search for truncation point
- [ ] Integration with tab bar and search bar rendering:
  - [ ] Tab title → `shape_text_string` → glyph instances
  - [ ] Search query → `shape_text_string` → glyph instances
- [ ] **Tests**:
  - [ ] "Hello" → 5 glyphs with sequential advances
  - [ ] Measure text returns correct total width
  - [ ] Truncation: long text gets ellipsis at correct position

---

## 6.14 Pre-Caching + Performance

Eliminate first-frame stalls and optimize per-frame costs.

- [ ] Pre-cache ASCII (0x20–0x7E) at font load time:
  - [ ] Rasterize all printable ASCII for Regular style
  - [ ] Insert into atlas immediately
  - [ ] First frame renders without any rasterization stalls
- [ ] Pre-cache bold ASCII if bold face available
- [ ] Scratch buffer reuse:
  - [ ] `runs_scratch: Vec<ShapingRun>` — cleared + reused per row (not reallocated)
  - [ ] `shaped_scratch: Vec<ShapedGlyph>` — same pattern
  - [ ] `col_glyph_map: Vec<Option<usize>>` — same pattern
  - [ ] Allocated once at max expected size, never shrink
- [ ] Face creation once per frame:
  - [ ] `create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face>>` — creates Face references from FaceData
  - [ ] Called once at start of frame, reused for all rows
  - [ ] Faces borrow from `Arc<Vec<u8>>` in FaceData (zero-copy)
- [ ] Font size change:
  - [ ] Clear entire atlas
  - [ ] Recompute cell metrics
  - [ ] Re-pre-cache ASCII
  - [ ] Invalidate all cached frame data
- [ ] **Performance targets**:
  - [ ] Shaping: < 2ms per frame for 80×24 terminal
  - [ ] Atlas miss (new glyph): < 0.5ms per glyph (rasterize + upload)
  - [ ] Atlas hit: HashMap lookup only (< 1μs)
  - [ ] No allocation in per-cell rendering loop

---

## 6.15 Section Completion

- [ ] All 6.1–6.14 items complete
- [ ] Full font pipeline: multi-face, fallback chain, cap-height normalization
- [ ] Rustybuzz shaping: ligatures, combining marks, OpenType features
- [ ] Advanced atlas: guillotine packing, multi-page, LRU eviction, Q6 keying
- [ ] Built-in glyphs: box drawing, blocks, braille, powerline — pixel-perfect
- [ ] Color emoji: RGBA atlas, correct rendering without fg tinting
- [ ] Synthetic bold: double-strike when no real bold face
- [ ] All text decorations: single, double, curly, dotted, dashed underline + strikethrough
- [ ] UI text shaping: tab bar titles, search bar, measure + truncate
- [ ] Pre-caching: no first-frame stall for ASCII
- [ ] **Visual tests**:
  - [ ] Ligatures: `=>`, `->`, `!=` render as single glyphs (with ligature font)
  - [ ] Box drawing: `htop`, `top`, `tmux` borders look pixel-perfect
  - [ ] Braille: `braille-art` renders correctly
  - [ ] Powerline: oh-my-zsh/starship prompt renders correctly
  - [ ] CJK: Chinese/Japanese/Korean characters render at correct size
  - [ ] Emoji: 🎉 🔥 ❤️ render in color
  - [ ] Combining marks: `é`, `ñ`, `ü` render with correct accents
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings

**Exit Criteria:** Font pipeline is production-quality. Every character type renders correctly: ASCII, ligatures, CJK, emoji, box drawing, braille, powerline, combining marks. Visual parity with the old prototype's font rendering.
