---
section: "06"
title: Font System
status: complete
goal: Replace single-font GlyphCache with FontSet supporting 4 style variants, fallback chains, underline/strikethrough rendering, dynamic font size, and cross-platform font discovery
sections:
  - id: "06.1"
    title: FontStyle enum & FontSet struct
    status: complete
  - id: "06.2"
    title: Cross-platform font discovery & loading
    status: complete
  - id: "06.3"
    title: Font fallback chain
    status: complete
  - id: "06.4"
    title: Style-aware glyph lookup & synthetic bold
    status: complete
  - id: "06.5"
    title: Render underlines & strikethrough
    status: complete
  - id: "06.6"
    title: Dynamic font size (Ctrl+=/Ctrl+-)
    status: complete
  - id: "06.7"
    title: Update app.rs & tab_bar.rs
    status: complete
  - id: "06.8"
    title: Completion checklist
    status: complete
  - id: "06.9"
    title: "Color Emoji (after Section 07: GPU Rendering)"
    status: not-started
---

# Section 06: Font System

**Status:** Complete (06.1–06.8 implemented; 06.9 Color Emoji deferred to post-GPU)
**Goal:** Build a multi-font system with style variants, fallback chains, text decorations,
dynamic sizing, and cross-platform font discovery. Replace the current single-font
`GlyphCache` with `FontSet`.

**Inspired by:**
- Ghostty's font system (`src/font/`) with shaper abstraction, fallback discovery
- Alacritty's `crossfont` crate with platform-specific font discovery and synthetic bold
- WezTerm's font configuration, fallback system, and underline rendering

**Implemented in:** `src/render.rs` (FontSet struct, FontStyle enum, glyph cache,
fallback chain, render_grid with style-aware glyphs, underline decorations, strikethrough,
synthetic bold), `src/app.rs` (Ctrl+=/Ctrl+-/Ctrl+0 zoom, change_font_size/reset_font_size)

**What was built:**
- FontSet with Regular/Bold/Italic/BoldItalic font variants (fontdue)
- Cross-platform font discovery: Windows (CascadiaMonoNF > CascadiaMono > Consolas > Courier)
  and Linux (JetBrainsMono > UbuntuMono > DejaVuSansMono > LiberationMono)
- Fallback font chain: Windows (Segoe UI Symbol, MS Gothic, Segoe UI), Linux (NotoSansMono,
  NotoSansSymbols2, NotoSansCJK, DejaVuSans)
- Style-aware glyph rasterization with (char, FontStyle) cache key
- Synthetic bold (double-strike at +1px) when real bold font unavailable
- Five underline styles: single, double, dotted, dashed, undercurl
- Strikethrough rendering
- Dynamic font zoom: Ctrl+= (+1px), Ctrl+- (-1px), Ctrl+0 (reset to 16px), clamped 8-32px
- ASCII pre-caching at startup for Regular style

---

## 06.1 FontStyle enum & FontSet struct

Replace `GlyphCache` with `FontSet` in `src/render.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

pub struct FontSet {
    fonts: [fontdue::Font; 4],      // Regular, Bold, Italic, BoldItalic
    has_variant: [bool; 4],          // true if loaded from actual variant file
    fallback_fonts: Vec<fontdue::Font>, // additional fonts for missing glyphs
    pub size: f32,
    pub cell_width: usize,
    pub cell_height: usize,
    pub baseline: usize,
    cache: HashMap<(char, FontStyle), (fontdue::Metrics, Vec<u8>)>,
}
```

- [ ] Define `FontStyle` enum with 4 variants
- [ ] Define `FontSet` struct with `fonts[4]`, `has_variant[4]`, `fallback_fonts`, metrics, cache
- [ ] `has_variant[i]` tracks whether real variant file was loaded or fell back to regular
- [ ] Cell dimensions always come from Regular font (all variants share grid)
- [ ] Keep `GlyphCache` as a type alias during transition if needed, then remove

---

## 06.2 Cross-platform font discovery & loading

Replace `load_font() -> Vec<u8>` with `FontSet::load(size: f32) -> FontSet`.

```rust
struct FontFamily {
    name: &'static str,
    regular: &'static [&'static str],  // candidate paths
    bold: &'static [&'static str],
    italic: &'static [&'static str],
    bold_italic: &'static [&'static str],
}
```

### Windows font families (in priority order):

| Family | Regular | Bold | Italic | BoldItalic |
|--------|---------|------|--------|------------|
| Cascadia Mono NF | CascadiaMonoNF.ttf | CascadiaMonoNF-Bold.ttf | CascadiaMonoNF-Italic.ttf | CascadiaMonoNF-BoldItalic.ttf |
| Cascadia Mono | CascadiaMono.ttf | CascadiaMono-Bold.ttf | CascadiaMono-Italic.ttf | CascadiaMono-BoldItalic.ttf |
| Consolas | consola.ttf | consolab.ttf | consolai.ttf | consolaz.ttf |
| Courier New | cour.ttf | courbd.ttf | couri.ttf | courbi.ttf |

### Linux font paths:

Scan these directories for font files:
- `~/.local/share/fonts/`
- `/usr/share/fonts/`
- `/usr/local/share/fonts/`

Known Linux font families:
| Family | Regular | Bold | Italic | BoldItalic |
|--------|---------|------|--------|------------|
| JetBrains Mono | JetBrainsMono-Regular.ttf | JetBrainsMono-Bold.ttf | JetBrainsMono-Italic.ttf | JetBrainsMono-BoldItalic.ttf |
| Ubuntu Mono | UbuntuMono-Regular.ttf | UbuntuMono-Bold.ttf | UbuntuMono-Italic.ttf | UbuntuMono-BoldItalic.ttf |
| DejaVu Sans Mono | DejaVuSansMono.ttf | DejaVuSansMono-Bold.ttf | DejaVuSansMono-Oblique.ttf | DejaVuSansMono-BoldOblique.ttf |
| Liberation Mono | LiberationMono-Regular.ttf | LiberationMono-Bold.ttf | LiberationMono-Italic.ttf | LiberationMono-BoldItalic.ttf |

### Loading logic:

- [ ] Try each family in order. Family succeeds if at least regular font loads.
- [ ] For each variant, try to load from file. If missing, clone regular font and set `has_variant[i] = false`.
- [ ] Compute cell_width/cell_height/baseline from regular font (same as today).
- [ ] On Windows, paths are `C:\Windows\Fonts\{filename}`
- [ ] On Linux, scan font directories for matching filenames
- [ ] Return `FontSet` with all 4 fonts populated.

---

## 06.3 Font fallback chain

When primary font doesn't have a glyph, try fallback fonts before giving up.

- [ ] After loading primary family, load additional fallback fonts:
  - Windows: Segoe UI Symbol, MS Gothic (CJK), Segoe UI Emoji
  - Linux: Noto Sans Mono, Noto Sans CJK, Noto Color Emoji, Symbola
- [ ] `FontSet::ensure()` glyph lookup walks the chain:
  1. Try primary font for the requested style
  2. Try primary font Regular style (style fallback)
  3. Try each fallback font in order
  4. If all miss, rasterize U+FFFD from primary font
- [ ] Cache stores which font provided each glyph (avoid re-searching)
- [ ] `fontdue::Font::has_glyph(char)` or check if rasterization returns empty bitmap

```rust
impl FontSet {
    fn rasterize_with_fallback(&mut self, ch: char, style: FontStyle) -> (Metrics, Vec<u8>) {
        // 1. Try requested style
        let idx = style as usize;
        if self.fonts[idx].has_glyph(ch) {
            return self.fonts[idx].rasterize(ch, self.size);
        }
        // 2. Try Regular
        if style != FontStyle::Regular && self.fonts[0].has_glyph(ch) {
            return self.fonts[0].rasterize(ch, self.size);
        }
        // 3. Try fallback fonts
        for fb in &self.fallback_fonts {
            if fb.has_glyph(ch) {
                return fb.rasterize(ch, self.size);
            }
        }
        // 4. Replacement character
        self.fonts[0].rasterize('\u{FFFD}', self.size)
    }
}
```

---

## 06.4 Style-aware glyph lookup & synthetic bold

- [ ] Map `CellFlags` to `FontStyle`:
  ```rust
  fn style_for_flags(flags: CellFlags) -> FontStyle {
      match (flags.contains(CellFlags::BOLD), flags.contains(CellFlags::ITALIC)) {
          (true, true) => FontStyle::BoldItalic,
          (true, false) => FontStyle::Bold,
          (false, true) => FontStyle::Italic,
          (false, false) => FontStyle::Regular,
      }
  }
  ```
- [ ] `ensure(ch, style)` — rasterize with fallback chain, cache result
- [ ] `get(ch, style) -> Option<&(Metrics, Vec<u8>)>` — retrieve from cache
- [ ] **Synthetic bold**: when `has_variant[Bold] == false`, draw glyph at (gx, gy) AND (gx+1, gy) for double-strike effect. Applied during rendering in `render_grid()`, not in cache.
- [ ] Pre-cache ASCII (0x20-0x7E) at startup for Regular style

---

## 06.5 Render underlines & strikethrough

Add text decoration rendering to `render_grid()` in `src/render.rs`.

### Underline rendering (after glyph alpha-blending, per cell):

```rust
let underline_y = y0 + cell_height - 2;  // 2px from bottom of cell

if cell.flags.contains(CellFlags::UNDERLINE) {
    draw_hline(buffer, buf_w, x0, underline_y, cell_w, ul_color);
}
if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
    draw_hline(buffer, buf_w, x0, underline_y, cell_w, ul_color);
    draw_hline(buffer, buf_w, x0, underline_y - 2, cell_w, ul_color);
}
if cell.flags.contains(CellFlags::DOTTED_UNDERLINE) {
    draw_dotted_hline(buffer, buf_w, x0, underline_y, cell_w, ul_color);
}
if cell.flags.contains(CellFlags::DASHED_UNDERLINE) {
    draw_dashed_hline(buffer, buf_w, x0, underline_y, cell_w, ul_color);
}
if cell.flags.contains(CellFlags::UNDERCURL) {
    draw_undercurl(buffer, buf_w, x0, underline_y, cell_w, ul_color);
}
```

### Underline color resolution:
- Check `cell.underline_color()` for SGR 58 override color
- Resolve via `palette.resolve()` if it's a named/indexed color
- Fall back to `fg_rgb` if no override

### Strikethrough:
```rust
if cell.flags.contains(CellFlags::STRIKEOUT) {
    let strike_y = y0 + cell_height / 2;
    draw_hline(buffer, buf_w, x0, strike_y, cell_w, fg_u32);
}
```

### Drawing helper functions:

- [ ] `draw_dotted_hline()` — 1px on, 1px off pattern
- [ ] `draw_dashed_hline()` — 3px on, 2px off pattern
- [ ] `draw_undercurl()` — sine wave, 2px amplitude, period = cell_width
- [ ] Reuse existing `draw_hline()` from `tab_bar.rs` (move to render.rs or make shared)

Note: `draw_hline` already exists in `tab_bar.rs`. Since render.rs needs it too, add
versions to `render.rs` (they operate on the same buffer format). The tab_bar versions
stay in tab_bar.rs since that module is self-contained.

---

## 06.6 Dynamic font size (Ctrl+=/Ctrl+-)

Allow runtime font size changes.

- [ ] Add `Ctrl+=` (or `Ctrl+Shift+=`) keybinding to increase font size by 1.0
- [ ] Add `Ctrl+-` keybinding to decrease font size by 1.0
- [ ] Add `Ctrl+0` keybinding to reset font size to default (16.0)
- [ ] Minimum font size: 8.0, maximum: 32.0
- [ ] On size change:
  1. Create new `FontSet` with new size (re-rasterize all cached glyphs)
  2. Recompute cell_width/cell_height/baseline from new size
  3. Recompute grid cols/rows from window size and new cell dimensions
  4. Resize all tabs in the window to new grid dimensions
  5. Request redraw
- [ ] Store default font size as a constant, current size in `App` or `FontSet`

```rust
// In App::window_event KeyboardInput handler:
// Ctrl+= or Ctrl++ — zoom in
if ctrl && matches!(&event.logical_key, Key::Character(c) if c.as_str() == "=" || c.as_str() == "+") {
    self.change_font_size(window_id, 1.0);
    return;
}
// Ctrl+- — zoom out
if ctrl && matches!(&event.logical_key, Key::Character(c) if c.as_str() == "-") {
    self.change_font_size(window_id, -1.0);
    return;
}
// Ctrl+0 — reset zoom
if ctrl && matches!(&event.logical_key, Key::Character(c) if c.as_str() == "0") {
    self.reset_font_size(window_id);
    return;
}
```

---

## 06.7 Update app.rs & tab_bar.rs

### app.rs changes:
- [ ] Replace `glyphs: GlyphCache` with `glyphs: FontSet` in `App` struct
- [ ] Replace `load_font()` + `GlyphCache::new()` with `FontSet::load(FONT_SIZE)`
- [ ] Update `render::GlyphCache` import to `render::FontSet`
- [ ] Add `change_font_size()` and `reset_font_size()` methods
- [ ] Add keyboard bindings for Ctrl+=/Ctrl+-/Ctrl+0

### tab_bar.rs changes:
- [ ] Replace `&mut GlyphCache` with `&mut FontSet` in `render_tab_bar()` and `render_text()`
- [ ] Always use `FontStyle::Regular` for tab bar text
- [ ] Update all call sites

---

## 06.8 Completion Checklist

- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no new warnings
- [ ] `cargo build --target x86_64-pc-windows-gnu --release` — compiles
- [ ] `cargo test` — all tests pass
- [ ] Visual: `ls --color` shows bold filenames
- [ ] Visual: `echo -e "\e[1mbold\e[0m \e[3mitalic\e[0m \e[4munderline\e[0m \e[9mstrike\e[0m"` — each style distinct
- [ ] Visual: `echo -e "\e[1;3mbold italic\e[0m"` — combined style
- [ ] Visual: `echo -e "\e[4:3mcurly\e[0m \e[4:4mdotted\e[0m \e[4:5mdashed\e[0m"` — underline variants
- [ ] Visual: Ctrl+= zooms in, Ctrl+- zooms out, Ctrl+0 resets
- [ ] Visual: Font size change reflows grid correctly
- [ ] Visual: Tab bar text unchanged (regular weight)
- [ ] Visual: Wide characters (CJK) still render correctly at double width
- [ ] Visual: Cursor renders correctly over styled text
- [ ] Visual: Missing glyphs show replacement character, not blank
- [ ] Synthetic bold works when bold font file is missing (double-strike)
- [ ] Font loads on Linux (when font files are present)

**Exit Criteria (06.1–06.8):** Terminal renders bold, italic, bold-italic text with real font variants.
Underlines (5 styles) and strikethrough are visible. Font size is dynamically adjustable.
Missing glyphs fall back through a chain before showing U+FFFD.

---

## 06.9 Color Emoji (after Section 07: GPU Rendering)

**Prerequisite:** Section 07 (GPU Rendering) should be completed first. Color emoji
requires RGBA compositing which is much more natural with a GPU pipeline. With
softbuffer CPU rendering, we'd need a separate RGBA blending path; with GPU we can
just upload RGBA textures to the atlas.

**Problem:** fontdue is an alpha-only rasterizer — it produces grayscale bitmaps.
Color emoji (COLR/CPAL, CBDT/CBLC, sbix) need RGBA output. Currently emoji render
as monochrome shapes tinted with the foreground color.

**How WezTerm does it:** WezTerm uses FreeType + HarfBuzz. FreeType handles
COLR/CPAL and CBDT/CBLC natively, returning BGRA bitmaps for color glyphs.

### Approach: `swash` crate for color glyph rasterization

`swash` is a pure-Rust font introspection and rendering library that supports:
- COLR/CPAL (vector color glyphs)
- CBDT/CBLC (bitmap color glyphs, used by Noto Color Emoji)
- sbix (Apple bitmap color glyphs)
- Returns RGBA pixel data for color glyphs

### Implementation steps:

- [ ] Add `swash` and `zeno` (its rasterization companion) to dependencies
- [ ] Detect color glyphs: use swash to check if a glyph has color layers
- [ ] Dual rasterization path in `FontSet`:
  - Non-color glyphs: continue using fontdue (fast, optimized for text)
  - Color glyphs: use swash to get RGBA bitmap
- [ ] New cache entry type to distinguish alpha-only vs RGBA:
  ```rust
  enum RasterizedGlyph {
      Alpha { metrics: Metrics, bitmap: Vec<u8> },       // fontdue grayscale
      Color { metrics: Metrics, bitmap: Vec<u8> },       // swash RGBA
  }
  ```
- [ ] RGBA compositing in render pipeline:
  - Alpha glyphs: existing fg-tinted alpha blend
  - Color glyphs: direct RGBA-over-background composite (no fg tinting)
- [ ] With GPU (post Section 07): upload color glyphs as RGBA textures to atlas
- [ ] Handle emoji presentation selectors (VS15 text, VS16 emoji)
- [ ] Handle ZWJ sequences (would need HarfBuzz for proper shaping — deferred)
- [ ] Emoji width: most emoji are width 2 (fullwidth cell)

### Color fonts to load:

| Platform | Font | Format |
|----------|------|--------|
| Windows | Segoe UI Emoji | COLR/CPAL |
| Linux | Noto Color Emoji | CBDT/CBLC |
| macOS | Apple Color Emoji | sbix |

### Verification:
- [ ] `echo "😀🎉🚀❤️🌍"` renders in full color
- [ ] Color emoji render at correct size (double-width cell)
- [ ] Non-emoji text still uses fontdue (no performance regression)
- [ ] Mixed lines (text + emoji) render correctly
- [ ] Emoji in vim/tmux status lines render correctly

**Exit Criteria (06.9):** Emoji render in full color, not monochrome. Uses swash for
color glyph rasterization alongside fontdue for regular text.
