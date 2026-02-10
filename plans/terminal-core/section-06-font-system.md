---
section: "06"
title: Font System
status: not-started
goal: Replace fontdue with a proper font stack supporting fallback chains, bold/italic variants, and emoji
sections:
  - id: "06.1"
    title: Font Discovery & Loading
    status: not-started
  - id: "06.2"
    title: Font Fallback Chain
    status: not-started
  - id: "06.3"
    title: Bold/Italic Variants
    status: not-started
  - id: "06.4"
    title: Glyph Caching
    status: not-started
  - id: "06.5"
    title: Emoji & Color Fonts
    status: not-started
  - id: "06.6"
    title: Completion Checklist
    status: not-started
---

# Section 06: Font System

**Status:** Not Started
**Goal:** Build a font system with discovery, fallback chains, bold/italic variants,
and emoji support. Replace the current single-font fontdue approach with something
that handles real-world text.

**Inspired by:**
- Ghostty's font system (`src/font/`) with shaper abstraction, fallback discovery, and sprite rendering
- Alacritty's `crossfont` crate with platform-specific font discovery
- WezTerm's extensive font configuration and fallback system

**Current state:** Single hardcoded font loaded via `load_font()` trying Windows paths
(CascadiaMono, Consola, Courier). fontdue does rasterization. No fallback, no
bold/italic, no emoji. `GlyphCache` is a simple `HashMap<char, (Metrics, Vec<u8>)>`.

---

## 06.1 Font Discovery & Loading

Find and load monospace fonts from the system.

- [ ] Platform-specific font discovery:
  - [ ] Windows: scan `C:\Windows\Fonts\` for known monospace fonts, or use DirectWrite font enumeration
  - [ ] Linux: use fontconfig (`fc-match "monospace"`) or scan `/usr/share/fonts/`
  - [ ] macOS: use CoreText font enumeration
- [ ] Font configuration: user specifies font family name, system resolves to path
  - [ ] Default: "Cascadia Mono", "Consolas", "Courier New" (Windows), "monospace" (Linux)
- [ ] Font size configuration: default 16px, user-adjustable
  - [ ] Ctrl+= / Ctrl+- for dynamic font size changes
  - [ ] Recompute cell dimensions on size change
- [ ] Replace hardcoded path list with proper discovery

**Ref:** Ghostty `font/discovery.zig`, Alacritty `crossfont` crate

---

## 06.2 Font Fallback Chain

When the primary font doesn't have a glyph, try fallback fonts.

- [ ] Define font fallback chain (ordered list of fonts to try)
  - [ ] Primary: user-configured font
  - [ ] Fallback 1: system monospace
  - [ ] Fallback 2: CJK font (for wide characters)
  - [ ] Fallback 3: emoji font (Segoe UI Emoji / Noto Color Emoji)
  - [ ] Fallback 4: symbol font (for box-drawing, braille, etc.)
- [ ] Glyph lookup walks the chain: try primary, then each fallback
- [ ] Cache which font provides which glyph (avoid re-searching)
- [ ] Handle missing glyphs: render a replacement character (U+FFFD) or tofu box

**Ref:** Ghostty `font/SharedGrid.zig` fallback logic, WezTerm font-fallback

---

## 06.3 Bold/Italic Variants

Support styled text rendering.

- [ ] For each font family, discover bold/italic/bold-italic variants
  - [ ] Windows: look for "-Bold", "-Italic", "-BoldItalic" variants
  - [ ] Or use OS font APIs to find style variants
- [ ] Map CellFlags to font variant:
  - [ ] `BOLD` -> bold font face
  - [ ] `ITALIC` -> italic font face
  - [ ] `BOLD | ITALIC` -> bold-italic font face
- [ ] Fallback: if bold variant not found, use synthetic bold (render with slight offset)
- [ ] Fallback: if italic variant not found, use synthetic italic (shear transform)
- [ ] GlyphCache keys need to include style: `(char, FontStyle)` -> glyph data

**Ref:** Ghostty font style handling, Alacritty `crossfont` style resolution

---

## 06.4 Glyph Caching

Efficient glyph rasterization and caching.

- [ ] Replace `HashMap<char, ...>` with `HashMap<GlyphKey, RasterizedGlyph>`
  - [ ] `GlyphKey { char, font_index, style, size }` — identifies unique glyphs
  - [ ] `RasterizedGlyph { metrics, bitmap, atlas_position }` — cached rasterization
- [ ] Lazy rasterization: only rasterize glyphs when first needed
- [ ] Pre-cache ASCII range (0x20-0x7E) at startup for performance
- [ ] LRU eviction for large glyph caches (if memory constrained)
- [ ] Invalidate cache on font size change or font change

**Ref:** Ghostty `font/SharedGrid.zig` atlas, Alacritty `GlyphCache`

---

## 06.5 Emoji & Color Fonts

Support emoji and color bitmap fonts.

- [ ] Detect color font formats: COLR/CPAL, CBDT/CBLC, sbix
- [ ] Render color glyphs as RGBA bitmaps (not alpha-only)
- [ ] Handle emoji presentation selectors (VS15/VS16)
- [ ] Handle ZWJ sequences (render as single glyph if font supports it)
- [ ] Emoji width: most emoji are width 2 (fullwidth)
- [ ] Box-drawing and powerline glyphs: consider custom rendering for pixel-perfect results

**Ref:** Ghostty sprite rendering for box-drawing, WezTerm emoji handling

---

## 06.6 Completion Checklist

- [ ] Font loads from system by family name, not hardcoded path
- [ ] Missing glyphs fall back to secondary fonts
- [ ] Bold text uses bold font variant (or synthetic bold)
- [ ] Italic text uses italic font variant (or synthetic italic)
- [ ] CJK characters render correctly at double width
- [ ] Emoji render (at least basic single-codepoint emoji)
- [ ] Font size can be changed at runtime
- [ ] Glyph cache performs well (no visible stutter on new characters)
- [ ] Box-drawing characters align properly

**Exit Criteria:** Terminal displays multi-language text correctly with proper
bold/italic rendering and basic emoji support. Font is configurable by name.
