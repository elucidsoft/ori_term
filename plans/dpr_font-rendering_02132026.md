---
plan: "dpr_font-rendering_02132026"
title: "Design Pattern Review: Font Rendering"
status: refined
---

# Design Pattern Review: Font Rendering

> **MANDATORY: After completing ANY task from the Implementation Roadmap, you MUST update this plan.**
> - Mark the task as `[x]` (done) in the roadmap checklist.
> - If the implementation deviated from the plan, update the relevant design section to reflect what was actually built.
> - If new tasks were discovered during implementation, add them to the appropriate phase.
> - If a design choice changed during implementation, update the Key Design Choices section with the new decision and rationale.
> - **This document is the single source of truth.** It must always reflect the current state of the font system — not what was originally planned, but what exists now and what remains to be done.

## ori_term Today

ori_term's font rendering is a modular three-tier stack: `src/render/font_discovery.rs` handles platform-specific font file resolution (DirectWrite on Windows via `dwrote`, directory scanning on Linux), `src/render/font_loading.rs` orchestrates lazy variant loading through `FontSet::load()`, and `src/render/mod.rs` defines `FontSet` -- the central type that holds four `fontdue::Font` objects (Regular/Bold/Italic/BoldItalic), a `Vec<fontdue::Font>` fallback chain, and a `HashMap<(char, FontStyle), (Metrics, Vec<u8>)>` glyph cache. The `GlyphAtlas` in `src/gpu/atlas.rs` uses row-based shelf packing on a fixed 1024x1024 R8Unorm wgpu texture, keyed by `(char, FontStyle, u16)` where the size key is `(size * 10.0).round() as u16`. The render loop in `src/gpu/render_grid.rs` calls `atlas.get_or_insert()` per cell, which triggers `FontSet::rasterize_with_fallback()` for cache misses -- a five-step fallback chain: requested style font, Regular font, fallback fonts, U+FFFD, then empty glyph.

Several design choices are sound and worth preserving. Lazy variant loading (Regular loaded eagerly, Bold/Italic/BoldItalic deferred until first use via `ensure_font_loaded()`) saves startup latency. The unified atlas that holds both grid glyphs and UI glyphs at different sizes via the `size_key` discriminator avoids separate texture bind group switching. Synthetic bold (1px offset re-render when `needs_synthetic_bold()` returns true) provides graceful degradation. The `FontConfig { size, family }` in `src/config/mod.rs` supports user-specified font families resolved through DirectWrite or filesystem paths. ASCII pre-caching via `atlas.precache_ascii()` eliminates cold-start latency for the most common codepoints.

The gaps are significant enough to block "best in class" font support. First, **no shaping**: `fontdue` rasterizes individual glyphs with no concept of glyph substitution or positioning. Programming ligatures (`!=` to a single glyph in Fira Code, `=>` arrows in JetBrains Mono) are impossible. Complex scripts (Arabic, Devanagari, Thai) are broken -- combining sequences render as isolated codepoints. Second, **no color emoji**: the atlas is `R8Unorm` (single-channel grayscale), so COLR/CBLC/sbix emoji tables are unrenderable. Third, **atlas overflow is silent**: when the 1024x1024 texture fills, `upload_bitmap()` returns a zero-size `AtlasEntry` with a log message and the glyph simply vanishes. No eviction, no growth, no multi-page fallback. Fourth, **fallback chain is hardcoded**: `FALLBACK_FONT_PATHS` and `DWRITE_FALLBACK_FAMILIES` are compile-time constants with no user configurability, no coverage-based selection, and no async discovery. A user with custom CJK fonts or Nerd Font patches has no way to insert them into the chain. Fifth, **no per-font configuration**: no way to specify OpenType features (`calt`, `liga`, `dlig`), size adjustments for fallback fonts, or per-font style overrides. Sixth, **the size key precision** of `(size * 10.0).round() as u16` means two font sizes differing by 0.05pt share a cache entry, producing incorrect rasterization at fractional DPI scales.

## Prior Art

### Alacritty -- Rasterizer Abstraction with Built-in Box Drawing

Alacritty separates font concerns into three layers: a `Rasterize` trait (with FreeType, CoreText, and DirectWrite backends in `crossfont`), a `GlyphCache` that maps `(FontKey, GlyphKey)` to loaded textures, and built-in rendering for box drawing characters (`U+2500`-`U+259F`) and Powerline glyphs that bypasses the font entirely. The `LoadGlyph` trait decouples rasterization from GPU upload -- the cache calls `load_glyph()` and doesn't know whether it's going to OpenGL or any other backend. Four `FontKey`s (normal/bold/italic/bold-italic) are computed once at startup and reused for all lookups.

The key strength is deterministic simplicity: no async, no background threads, no state machines. Box drawing glyphs are pixel-perfect because they're rendered mathematically, not from font outlines -- this avoids the common problem of box drawing characters from different fonts having inconsistent widths or heights. The tradeoff is that Alacritty has no shaping (no ligatures), limited fallback on Windows (DirectWrite's opaque fallback doesn't expose the chain), and no color emoji. For ori_term, the `LoadGlyph` trait pattern and the built-in box drawing approach are both directly applicable -- ori_term already has `draw_block_char()` in `render_grid.rs` covering `U+2580`-`U+259F`, but could extend this to the full box drawing + Powerline range.

### WezTerm -- Async Fallback Resolution with HarfBuzz Shaping

WezTerm's font system is the most comprehensive of the three reference emulators. It uses HarfBuzz for shaping (programming ligatures, complex scripts, combining marks), a font locator layer that abstracts fontconfig, DirectWrite, and CoreText behind a common interface, and -- crucially -- an async fallback resolution system. When a glyph is missing from all loaded fonts, WezTerm doesn't block; it returns a placeholder and spawns a background thread to search the system for a font that covers the missing codepoint. Once found, the renderer is notified to re-cache. The `LoadedFont` holds an ordered fallback chain, and HarfBuzz consumes it left-to-right during shaping, with a `no_glyphs` set tracking permanently missing characters to avoid repeated searches.

The shaping integration is the critical pattern for ori_term. WezTerm shapes runs of text (not individual codepoints) through HarfBuzz, which handles glyph substitution (`calt`, `liga` features), pair positioning (kerning), and cluster formation (combining marks attached to base characters). This is the only way to support programming ligatures -- the terminal must identify runs of characters that belong to the same font, pass them to HarfBuzz as a cluster, and receive back shaped glyph IDs with x/y offsets. The async fallback is also worth adopting: blocking the render loop to search the filesystem for a CJK font that covers U+4E00 is unacceptable at 60fps. WezTerm's approach of showing a placeholder and resolving in the background is the right latency tradeoff.

### Ghostty -- Deferred Faces with Metric Normalization and 2D Atlas Packing

Ghostty's font `Collection` organizes faces by style (normal/bold/italic/bold-italic) and supports deferred loading: a face can be in "deferred" state where only codepoint coverage information (from OpenType `cmap` tables) is loaded, without full font data. This enables fast coverage queries during fallback selection without the memory cost of loading every fallback font. When adding a fallback face, Ghostty computes a `size_adjustment` factor to normalize cap heights -- if the fallback font's cap height doesn't match the primary font's, all its glyphs are scaled so that text from mixed fonts aligns vertically.

The atlas uses Jukka Jylanki's 2D rectangle bin-packing algorithm instead of simple shelf packing, which achieves better texture utilization (less wasted space between rows of different-height glyphs). For ori_term, the deferred face pattern directly maps to the existing lazy loading in `FontSet` but extends it: instead of storing `Option<fontdue::Font>` and loading the entire font on first use, store coverage metadata eagerly and defer the full load. The metric normalization is essential for professional-quality rendering -- without it, fallback fonts (especially CJK fonts with different design metrics) produce jarring vertical alignment shifts. Ghostty's per-font feature control (`dlig` disable for fonts where it breaks CJK) is also critical.

## Community Pain Points & How We Address Them

The following are the most common and most painful font-related complaints across terminal emulator issue trackers. Each one maps to a specific design choice in this proposal.

| # | Pain Point | Source Issues | Severity | Our Answer |
|---|-----------|---------------|----------|------------|
| 1 | **No automatic font fallback on Windows/macOS.** Users must manually configure every fallback font or emoji/CJK/symbols render as tofu. | Alacritty [#3573](https://github.com/alacritty/alacritty/issues/3573), [#6867](https://github.com/alacritty/alacritty/issues/6867), [#4617](https://github.com/alacritty/alacritty/issues/4617) | Critical | **FontDatabase** with coverage-indexed system scan + **async fallback discovery** (Design Choices #2, #7). On first missing glyph, background thread queries DirectWrite `MapCharacters` / fontconfig to find a covering font automatically. Zero user config required for basic emoji/CJK. |
| 2 | **No programming ligatures.** `!=`, `=>`, `->`, `<=` render as separate characters even with ligature fonts like Fira Code or JetBrains Mono. | Alacritty — deliberate omission; widely requested | High | **ShapingEngine** with rustybuzz (Design Choice #1, #8). Run-based shaping with `calt`/`liga` features enabled by default. Ligature glyphs span multiple cells via cluster-aware cell mapping. |
| 3 | **Ligature glyphs wider than cell width cause stair-stepping.** Shaped glyphs from Nerd Fonts Mono overshoot cell boundaries, producing visual gaps or overlaps. DPI-dependent — worse on external monitors at certain font sizes. Root cause: hinted glyphs snap to pixel grid differently per-glyph, so shaped ligature widths don't align with integer cell grid. | WezTerm [#4874](https://github.com/wez/wezterm/issues/4874), [#2888](https://github.com/wez/wezterm/issues/2888) | High | **Hinting ON with grid clamping** — keep hinting for crisp text but quantize glyph advances to cell-width multiples so gaps can't form. First-cell + continuation rendering (Design Choice #8) means ligatures are one glyph spanning multiple cells — no seam possible. 26.6 fixed-point precision (Design Choice #9) eliminates rounding collisions at fractional DPI. |
| 4 | **CJK rendering breaks with `dlig` enabled.** Discretionary ligatures interfere with Japanese/Chinese glyph substitution, producing garbled output (e.g., "ます" breaking in BIZ UDGothic). | Ghostty [#5372](https://github.com/ghostty-org/ghostty/issues/5372) | High | **Per-font OpenType feature configuration** (Design Choice #6). Each fallback font entry can specify feature overrides (`features = ["-dlig"]`), so CJK fonts never get discretionary ligatures applied. |
| 5 | **Fallback fonts vertically misaligned.** Different fonts have different ascenders/descenders/cap heights, causing CJK or emoji text to sit higher or lower than the primary font on the same line. CJK fonts at `scale=1.2` (needed for correct 2x width) shift baseline without compensation. | WezTerm [#1803](https://github.com/wez/wezterm/issues/1803) | Medium | **Cap-height metric normalization** (Design Choice #3) + **OpenType BASE table reading** when available (per HarfBuzz maintainer recommendation). Every fallback face gets a scale factor: `primary.cap_height / fallback.cap_height`. When the font's BASE table contains per-script baseline offsets (e.g., ideographic baseline for CJK), use those for more precise alignment. Per-font `size_offset` in config allows manual fine-tuning. |
| 6 | **Small font sizes produce gaps between cells.** Antialiased glyphs at small sizes have fractional pixel widths; inconsistent rounding per glyph produces visible seams in the grid. | WezTerm [#6931](https://github.com/wez/wezterm/issues/6931) | Medium | **Sub-pixel precision size key** (Design Choice #9) using 1/64th point (26.6 fixed-point), plus consistent rounding in the rasterization path. Cell dimensions are computed from the primary font's `advance_width` rounded to whole pixels, ensuring uniform grid spacing. |
| 7 | **No color emoji.** Terminals render emoji as monochrome outlines (or tofu) because the glyph atlas is grayscale-only. | Alacritty [#4657](https://github.com/alacritty/alacritty/issues/4657); common across terminals | Medium | **Multi-page RGBA atlas** (Design Choice #4). Separate RGBA texture pages for color emoji. Emoji rasterized at **native resolution** (e.g., 128x128 for Apple emoji) and stored in atlas at full size — GPU scales down in shader for maximum quality. CBDT/CBLC bitmap extraction in Phase 2.5 (covers Noto Color Emoji), COLR vector in Phase 3 (covers Windows Segoe UI Emoji). Single draw call for both grayscale and color via shader branching. |
| 8 | **Nerd Font / Powerline glyphs misaligned or wrong width.** Icon fonts patched into monospace fonts often have incorrect metrics, producing gaps or overlaps at the Powerline separator boundary. | Widespread across all emulators | Medium | **Built-in pixel-perfect rendering** (Design Choice #5), **configurable, default ON** (`builtin_glyphs = true`). Powerline (`U+E0A0`-`U+E0D4`), box drawing (`U+2500`-`U+257F`), block elements (`U+2580`-`U+259F`), and braille (`U+2800`-`U+28FF`) rendered as mathematical geometry, bypassing fonts entirely. Users with custom Powerline glyphs in their font can disable with `builtin_glyphs = false` to use font-based rendering. |
| 9 | **HarfBuzz shaping causes perceptible lag.** Complex shaping on slower machines introduces frame drops, especially with many unique glyph clusters per frame. Root cause in WezTerm: aggressive font unloading heuristic caused repeated file I/O for fallback fonts. | WezTerm [#5280](https://github.com/wez/wezterm/issues/5280) | Low | **No font unloading** — once a deferred face is promoted to Loaded, it stays loaded for the session (no pruning heuristic). **Shaped run caching** by `(text_run, font_id, features)` key. **Only dirty lines re-shape** — integrated with damage tracking in `PreparedFrame`. Shape all visible lines each frame (80-line viewport + run caching makes this fast enough; no frame budget needed). |
| 10 | **Atlas exhaustion with many unique glyphs.** CJK-heavy workloads or large font sizes fill a fixed-size atlas texture, and new glyphs silently disappear. | ori_term (current), common in simpler emulators | Low | **Multi-page atlas with LRU eviction** (Design Choice #4). Atlas grows by allocating new pages on demand (up to configurable max). When all pages are full, the least-recently-used page is evicted and its glyphs re-rasterized on demand. wgpu texture arrays make multi-page cost-free in draw calls. |

### What This Means for Users

A user running ori_term with default settings should get:
- **Automatic emoji and CJK support** without configuring fallback fonts (pain points #1, #7)
- **Programming ligatures** in Fira Code / JetBrains Mono out of the box (pain point #2)
- **Pixel-perfect Powerline prompts** regardless of Nerd Font version (pain point #8)
- **No vertical jitter** when mixing fonts (pain point #5)
- **No atlas exhaustion** on CJK-heavy workloads (pain point #10)

A power user can additionally:
- Configure per-font OpenType features to avoid CJK/ligature interference (pain point #4)
- Fine-tune fallback font sizes with `size_offset` (pain point #5)
- Set variable font axes (`wght`, `wdth`, `ital`) for fonts that support them

## Proposed Best-of-Breed Design

### Core Idea

The new font system replaces `FontSet` + `GlyphAtlas` with a four-layer architecture: **FontDatabase** (system font discovery and coverage indexing, from WezTerm's locator pattern), **FontCollection** (ordered face management with deferred loading and metric normalization, from Ghostty's Collection), **ShapingEngine** (HarfBuzz integration for ligatures and complex scripts, from WezTerm's shaper), and **GlyphAtlasManager** (multi-page RGBA+grayscale atlas with LRU eviction and 2D bin packing, combining Ghostty's packing with a growth strategy none of the reference emulators have). Built-in glyph rendering for box drawing, Powerline, and braille characters (from Alacritty's pattern) bypasses the entire font pipeline for pixel-perfect results.

The key architectural insight is that these four layers have clean data flow boundaries: FontDatabase produces `FaceId`s and coverage bitmaps. FontCollection maps `(FaceId, style)` to loaded face handles with normalized metrics. ShapingEngine consumes text runs and produces `ShapedGlyph` sequences (glyph IDs + positions). AtlasManager consumes `ShapedGlyph`s and produces UV coordinates for the GPU. Each layer is independently testable, and the pipeline flows strictly forward: discovery -> collection -> shaping -> atlas -> GPU. No layer reaches back into a previous layer. This matches ori_term's implementation hygiene rules (one-way data flow, no circular imports).

### Key Design Choices

1. **rustybuzz for shaping, fontdue retained for rasterization** (from WezTerm). rustybuzz (pure-Rust HarfBuzz port) handles glyph substitution (`calt`, `liga`), positioning, and cluster formation. fontdue continues to rasterize individual glyph IDs from the shaped output. This is surgical — adds exactly shaping without replacing the rasterizer that already works. rustybuzz is chosen over `harfbuzz-rs` (C bindings) because it cross-compiles cleanly to `x86_64-pc-windows-gnu` without a C toolchain, has no unsafe FFI, and is actively maintained by the same team behind `ttf-parser`. cosmic-text was considered and rejected — it bundles paragraph layout, bidi reordering, and other rich-text features that are overkill for a terminal grid.

2. **Coverage-indexed deferred faces with eager cmap scan** (from Ghostty). At startup, scan all system fonts and parse just the `cmap` table (codepoint coverage) from each, stored as RoaringBitmaps (~1-2 KB per font). This builds a coverage index: "which fonts on this system can render U+4E00?" Full font data (often 5-20 MB for CJK) is deferred until first shape/rasterize call. Coverage queries (`face.has_codepoint(cp)`) are O(1) on the pre-parsed cmap, enabling fast fallback selection without loading megabytes of font data at startup. The cmap scan cost is ~100ms for hundreds of system fonts (just reading binary tables, no rasterization) — acceptable at startup. This extends ori_term's existing `ensure_font_loaded()` pattern to work at the coverage level rather than the entire-font level.

3. **Cap-height metric normalization across fallback fonts** (from Ghostty). When a fallback font is added to the collection, compute a scale factor: `primary.cap_height / fallback.cap_height`. All glyphs rasterized from the fallback are scaled by this factor, ensuring CJK characters, emoji, and symbol fonts align vertically with the primary font. Cap height was chosen over x-height (better for terminal workloads where you're mixing Latin commands with CJK filenames) and over ascender (which shrinks fallback glyphs too aggressively). A user-configurable `size_offset` per fallback font provides manual fine-tuning for edge cases. This directly addresses the community pain point (WezTerm #1803) of fallback fonts with different descenders causing vertical drift.

4. **Multi-page RGBA atlas with LRU eviction** (novel, inspired by Ghostty's 2D packing). Replace the single 1024x1024 R8Unorm texture with an atlas manager using **2048x2048 pages** (4x capacity per page — modern GPUs handle this trivially, and CJK workloads with thousands of unique characters genuinely need the space). Supports: (a) grayscale pages for regular text glyphs, (b) RGBA pages for color emoji (CBDT/CBLC bitmap in Phase 2, COLR vector in Phase 3), (c) dynamic page allocation (start with one page, grow on demand up to a configurable max, default 4), (d) LRU eviction when all pages are full (evict the least-recently-used page entirely, re-cache its glyphs on demand). Each page uses 2D rectangle bin packing (Jylanki's Guillotine best-short-side-fit algorithm from Ghostty) instead of shelf packing, improving utilization by 15-25% for mixed-size glyph workloads.

5. **Built-in glyph rendering for box drawing, Powerline, and braille** (from Alacritty). Extend the existing `draw_block_char()` in `render_grid.rs` to cover: `U+2500`-`U+257F` (box drawing), `U+2580`-`U+259F` (block elements, already implemented), `U+2800`-`U+28FF` (braille), `U+E0A0`-`U+E0D4` (Powerline + extras). These are rendered as mathematical geometry (rectangles, lines, arcs) directly into the instance buffer, bypassing font lookup entirely. This guarantees pixel-perfect alignment regardless of which font is loaded -- the most common Nerd Font complaint (glyph misalignment, incorrect widths) disappears.

6. **Per-font OpenType feature configuration** (from Ghostty's per-font feature tuning, addressing Ghostty #5372). The config supports per-font feature overrides:
   ```toml
   [font]
   family = "JetBrains Mono"
   size = 16.0
   features = ["calt", "liga"]

   [[font.fallback]]
   family = "Noto Sans CJK"
   features = ["-dlig"]  # disable discretionary ligatures for CJK
   size_offset = -1.0    # slightly smaller to match cap height

   [[font.fallback]]
   family = "Noto Color Emoji"
   ```
   Each fallback entry specifies family, optional feature overrides, and optional size offset. The ShapingEngine applies per-font features during rustybuzz shaping calls. **Defaults:** `calt` + `liga` enabled (programming ligatures work out of the box, matching VS Code). `dlig`, `clig`, `kern` disabled by default — `dlig` is the feature that breaks CJK (Ghostty #5372), and kerning is irrelevant for monospace grids.

7. **Async fallback font discovery** (from WezTerm). When a codepoint is not covered by any loaded face, queue a background search. The search thread queries the `FontDatabase.find_faces_for_codepoints()` with the missing codepoint set. Results are sent back via `mpsc` channel and merged into the FontCollection. **Placeholder:** The render loop shows a **dotted box with hex codepoint** (e.g., a rectangle containing "4E00") for the 1-2 frames while the lookup is in-flight. This is rendered as a built-in glyph — more informative than U+FFFD because users can see the exact codepoint being resolved and know the system is working on it, not that the font is broken. Once the fallback font is discovered, the next frame renders the real glyph. This prevents UI stalls when encountering unexpected Unicode blocks.

8. **Run-based shaping with cluster-aware cell mapping, first-cell rendering** (from WezTerm/Ghostty). The shaping engine doesn't shape individual characters -- it shapes runs. A "run" is a maximal sequence of characters that share the same font face and style. For a terminal line like `if x != y`, the shaper might produce: run1=`[i, f, space, x, space]` (regular), run2=`[!, =]` (ligature replaces two chars with one glyph spanning two cells), run3=`[space, y]` (regular). The shaped output maps each glyph back to a cell range (cluster), so the renderer knows which cells a multi-cell ligature glyph spans. **Multi-cell ligature rendering:** The full ligature glyph is drawn at the first cell's position with its full width. Subsequent cells in the cluster are marked as "continuation" and render nothing (no UV clipping per cell). This is WezTerm and Ghostty's approach — simpler, avoids visual artifacts from splitting ligature glyphs, and the cursor/selection system already understands cell boundaries.

9. **Sub-pixel-precision size key** (novel). Replace `(size * 10.0).round() as u16` with `(size * 64.0).round() as u32` (1/64th point precision, matching FreeType's 26.6 fixed-point convention). This eliminates the rounding collisions at fractional DPI scales where two distinct sizes mapped to the same cache key.

10. **Variable font axis support** (novel, **Phase 3**). For variable fonts (e.g., Recursive, JetBrains Mono Variable), expose axis configuration in the config:
    ```toml
    [font]
    family = "JetBrains Mono Variable"
    axes = { wght = 400, wdth = 100 }
    ```
    The FontCollection applies axis values when loading the face. Bold/Italic variants are derived by adjusting `wght`/`ital` axes rather than loading separate font files, providing smoother weight transitions and reducing memory usage. **Note:** Types are designed to support this from day one (axis fields in config, `VariationAxis` in `LoadedFace`), but actual variable font interpolation is deferred to Phase 3. The foundation (shaping, atlas, fallback) is far more impactful — most terminal users use static fonts.

11. **Hinting on with grid clamping** (novel, addressing WezTerm #4874/#6931). Keep font hinting enabled for crisp text at all sizes. After rasterization, quantize glyph advance widths to the nearest cell-width multiple. This eliminates the sub-pixel gaps that occur when hinted glyphs snap to the pixel grid differently per-glyph, without sacrificing sharpness (WezTerm's `NO_HINTING` fix made text blurrier). The grid clamping is applied post-shaping: `clamped_advance = round(advance / cell_width) * cell_width`.

12. **OpenType BASE table for per-script baselines** (from HarfBuzz maintainer recommendation on WezTerm #1803). When a fallback font's BASE table contains per-script baseline offsets (e.g., ideographic baseline for CJK, hanging baseline for Devanagari), use those for more precise vertical alignment than cap-height ratio alone. `ttf-parser` (used by rustybuzz) exposes BASE table reading. This is a refinement applied on top of cap-height normalization — not a replacement.

13. **Color emoji at native resolution, GPU-scaled** (novel). Store color emoji bitmaps in the RGBA atlas at their native resolution (e.g., 128x128 for Apple emoji, 64x64 for Noto). The GPU scales down to cell size in the fragment shader via bilinear sampling. This preserves emoji detail that would be lost by pre-scaling to cell dimensions. The tradeoff (more atlas space per emoji) is acceptable because emoji usage is typically sparse and the 2048x2048 RGBA pages have ample room.

14. **Configurable built-in glyph rendering, default ON**. The `builtin_glyphs` config option (default `true`) controls whether box drawing, braille, and Powerline glyphs are rendered as mathematical geometry or loaded from fonts. Users with custom Powerline glyphs in their Nerd Font can disable this to use font-based rendering. When ON, these codepoints bypass the entire font pipeline (no fallback search, no shaping, no atlas insertion).

### What Makes ori_term's Approach Unique

ori_term's constraints create three opportunities that no reference emulator exploits:

**wgpu's texture array support enables true multi-page atlases without bind group switching.** OpenGL (Alacritty) requires rebinding textures or using texture atlases. Metal (Ghostty) can use argument buffers but the implementation is complex. wgpu's `TextureViewDimension::D2Array` lets ori_term create a single texture array where each layer is an atlas page. The fragment shader receives a `texture_2d_array` and a layer index per glyph instance. This means multi-page atlas support costs zero additional draw calls -- a glyph on page 3 and a glyph on page 0 render in the same instanced draw call, with the page index packed into the existing instance data. None of the reference emulators achieve this.

**The WGSL shader pipeline can perform color emoji compositing in the same pass as grayscale glyph rendering.** The current foreground shader samples `R8Unorm` and multiplies by `fg_color`. With an RGBA atlas page for color emoji, the shader can branch on a flag bit in the instance data: if the glyph is from a color page, sample all four channels and bypass the fg_color tint; if grayscale, use the existing alpha-multiply path. This eliminates the separate "color emoji pass" that WezTerm requires (it renders color emoji in a distinct pipeline stage).

**The single-process, multi-tab architecture means the FontCollection is shared across all tabs.** WezTerm's multiplexer architecture means each pane potentially has its own font state. Ghostty's per-surface rendering means font data is duplicated per window. ori_term's `App` struct owns a single `FontCollection` and `AtlasManager` shared by all tabs and windows via `&mut` borrows during rendering. This means one copy of each loaded font, one atlas, one fallback chain -- memory-efficient and cache-friendly.

### Concrete Types & Interfaces

```rust
// ---- font/database.rs ----

/// System font database with coverage indexing.
/// Built once at startup, updated when async fallback discovers new fonts.
pub struct FontDatabase {
    /// All known font files on the system.
    faces: Vec<FontFaceInfo>,
    /// Codepoint -> list of face indices that cover it.
    /// Built from cmap tables during initial scan.
    coverage_index: HashMap<u32, SmallVec<[FaceIdx; 4]>>,
}

/// Lightweight metadata about a font face (no font data loaded).
pub struct FontFaceInfo {
    pub path: PathBuf,
    pub face_index: u32,           // for .ttc collections
    pub family: String,
    pub style: FontStyle,          // Regular/Bold/Italic/BoldItalic
    pub weight: u16,               // 100-900 (CSS weight)
    pub is_monospace: bool,
    pub coverage: RoaringBitmap,   // codepoints this face covers (from cmap)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceIdx(u32);

impl FontDatabase {
    /// Build by scanning system font directories.
    /// Windows: enumerate via DirectWrite. Linux: fontconfig + directory scan.
    pub fn build() -> Self { /* ... */ }

    /// Find faces covering a set of codepoints, sorted by coverage intersection size.
    pub fn find_faces_for_codepoints(&self, codepoints: &[u32]) -> Vec<FaceIdx> { /* ... */ }

    /// Find a specific family by name, returning face indices for all 4 styles.
    pub fn find_family(&self, name: &str) -> Option<[Option<FaceIdx>; 4]> { /* ... */ }
}


// ---- font/collection.rs ----

/// Ordered collection of font faces with deferred loading and metric normalization.
pub struct FontCollection {
    /// Primary font (4 style variants). Always fully loaded.
    primary: [LoadedFace; 4],
    /// Fallback chain, in priority order. Deferred until first use.
    fallbacks: Vec<CollectionEntry>,
    /// Metrics from the primary Regular face (used for normalization).
    primary_metrics: PrimaryMetrics,
    /// Current font size in points.
    size: f32,
    /// OpenType features for the primary font.
    primary_features: Vec<Feature>,
}

pub struct PrimaryMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    pub baseline: f32,
    pub cap_height: f32,
    pub ascent: f32,
    pub descent: f32,
}

pub enum CollectionEntry {
    /// Only cmap + metrics loaded. Full font data loaded on first rasterize.
    Deferred {
        info: FontFaceInfo,
        scale_factor: f32,    // primary.cap_height / this.cap_height
        features: Vec<Feature>,
    },
    /// Fully loaded, ready for shaping and rasterization.
    Loaded {
        face: LoadedFace,
        scale_factor: f32,
        features: Vec<Feature>,
    },
}

/// A fully loaded font face: font data + rustybuzz face + fontdue font.
pub struct LoadedFace {
    /// Raw font file bytes (kept alive for rustybuzz/fontdue references).
    data: Arc<Vec<u8>>,
    /// rustybuzz face for shaping.
    shaping_face: rustybuzz::Face<'static>,
    /// fontdue font for rasterization.
    raster_font: fontdue::Font,
    /// Codepoint coverage (from cmap).
    coverage: RoaringBitmap,
    /// Face index within .ttc collection.
    face_index: u32,
}

/// An OpenType feature tag with enable/disable.
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub tag: [u8; 4],   // e.g., b"calt", b"liga", b"dlig"
    pub enabled: bool,
}

impl FontCollection {
    /// Build from config, loading primary font and deferring fallbacks.
    pub fn from_config(
        config: &FontConfig,
        db: &FontDatabase,
        size: f32,
    ) -> Result<Self, FontError> { /* ... */ }

    /// Rebuild at a new size (preserves faces, recomputes metrics, clears raster cache).
    #[must_use]
    pub fn resize(&self, new_size: f32) -> Self { /* ... */ }

    /// Find which face covers a codepoint. Returns face index in collection order.
    /// Searches primary first, then fallbacks (checking deferred coverage without loading).
    pub fn find_face_for_codepoint(&self, cp: u32, style: FontStyle) -> Option<FaceRef> { /* ... */ }

    /// Ensure a fallback entry is fully loaded (promote Deferred -> Loaded).
    pub fn ensure_loaded(&mut self, entry_idx: usize) -> &LoadedFace { /* ... */ }

    /// Get the primary face for a style.
    pub fn primary(&self, style: FontStyle) -> &LoadedFace { /* ... */ }

    /// Get the primary metrics (cell dimensions, baseline).
    pub fn metrics(&self) -> &PrimaryMetrics { /* ... */ }

    /// Whether the primary font has a real bold variant (not synthetic).
    pub fn has_real_bold(&self) -> bool { /* ... */ }
}

/// Reference to a face within the collection (primary or fallback).
#[derive(Debug, Clone, Copy)]
pub enum FaceRef {
    Primary(FontStyle),
    Fallback(usize),  // index into fallbacks vec
}


// ---- font/shaping.rs ----

/// A shaped glyph ready for atlas lookup and rendering.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// Glyph ID within the font face (not a Unicode codepoint).
    pub glyph_id: u16,
    /// Which face this glyph comes from.
    pub face_ref: FaceRef,
    /// X offset from the cell's left edge, in pixels (sub-pixel precision).
    pub x_offset: f32,
    /// Y offset from the baseline, in pixels.
    pub y_offset: f32,
    /// Advance width in pixels (may span multiple cells for ligatures).
    pub x_advance: f32,
    /// Grid column where this glyph starts.
    pub col_start: usize,
    /// Number of grid columns this glyph spans (1 for normal, 2+ for ligatures/wide).
    pub col_span: usize,
    /// True if this is a color emoji glyph (use RGBA atlas page).
    pub is_color: bool,
}

/// A run of characters sharing the same face and style.
struct ShapingRun {
    face_ref: FaceRef,
    style: FontStyle,
    features: Vec<Feature>,
    /// Column range in the grid line.
    col_start: usize,
    col_end: usize,
    /// The text content of this run.
    text: String,
}

/// Shape a line of cells into positioned glyphs.
///
/// 1. Segment the line into runs (by face + style).
/// 2. Shape each run through rustybuzz.
/// 3. Map shaped glyphs back to cell columns.
/// 4. Apply metric normalization for fallback fonts.
pub fn shape_line(
    cells: &[Cell],
    cols: usize,
    collection: &mut FontCollection,
    size: f32,
) -> Vec<ShapedGlyph> {
    let runs = segment_into_runs(cells, cols, collection);
    let mut glyphs = Vec::new();

    for run in &runs {
        let face = match run.face_ref {
            FaceRef::Primary(style) => collection.primary(style),
            FaceRef::Fallback(idx) => collection.ensure_loaded(idx),
        };

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(&run.text);
        buffer.set_direction(rustybuzz::Direction::LeftToRight);

        // Apply per-font OpenType features.
        let features: Vec<rustybuzz::Feature> = run.features.iter().map(|f| {
            rustybuzz::Feature::new(
                rustybuzz::Tag::from_bytes(&f.tag),
                if f.enabled { 1 } else { 0 },
                ..,
            )
        }).collect();

        let output = rustybuzz::shape(&face.shaping_face, &features, buffer);
        let positions = output.glyph_positions();
        let infos = output.glyph_infos();

        let scale = match run.face_ref {
            FaceRef::Primary(_) => 1.0,
            FaceRef::Fallback(idx) => collection.fallback_scale(idx),
        };

        let mut cursor_x = 0.0f32;
        for (pos, info) in positions.iter().zip(infos.iter()) {
            let x_advance = pos.x_advance as f32 / 64.0 * scale;
            let x_offset = pos.x_offset as f32 / 64.0 * scale;
            let y_offset = pos.y_offset as f32 / 64.0 * scale;

            // Map cluster index back to grid column.
            let cluster_col = run.col_start + info.cluster as usize;
            let col_span = compute_col_span(x_advance, collection.metrics().cell_width);

            glyphs.push(ShapedGlyph {
                glyph_id: info.glyph_id as u16,
                face_ref: run.face_ref,
                x_offset: cursor_x + x_offset,
                y_offset,
                x_advance,
                col_start: cluster_col,
                col_span,
                is_color: face.is_color_glyph(info.glyph_id),
            });

            cursor_x += x_advance;
        }
    }

    glyphs
}

/// Segment a line of cells into shaping runs by face and style.
fn segment_into_runs(
    cells: &[Cell],
    cols: usize,
    collection: &mut FontCollection,
) -> Vec<ShapingRun> {
    let mut runs = Vec::new();
    let mut current_face = FaceRef::Primary(FontStyle::Regular);
    let mut current_style = FontStyle::Regular;
    let mut run_start = 0;
    let mut run_text = String::new();

    for col in 0..cols {
        let cell = &cells[col];
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            continue;
        }
        let style = FontStyle::from_cell_flags(cell.flags);
        let face = collection
            .find_face_for_codepoint(cell.c as u32, style)
            .unwrap_or(FaceRef::Primary(style));

        // Break run on face or style change.
        if col > 0 && (face != current_face || style != current_style) {
            if !run_text.is_empty() {
                runs.push(ShapingRun {
                    face_ref: current_face,
                    style: current_style,
                    features: collection.features_for(current_face),
                    col_start: run_start,
                    col_end: col,
                    text: std::mem::take(&mut run_text),
                });
            }
            run_start = col;
        }

        current_face = face;
        current_style = style;
        run_text.push(cell.c);

        // Append combining marks from CellExtra.
        for &zw in cell.zerowidth() {
            run_text.push(zw);
        }
    }

    // Flush last run.
    if !run_text.is_empty() {
        runs.push(ShapingRun {
            face_ref: current_face,
            style: current_style,
            features: collection.features_for(current_face),
            col_start: run_start,
            col_end: cols,
            text: run_text,
        });
    }

    runs
}

/// Compute how many grid columns a glyph advance spans.
fn compute_col_span(x_advance: f32, cell_width: f32) -> usize {
    ((x_advance / cell_width).round() as usize).max(1)
}


// ---- font/atlas_manager.rs ----

/// Cache key: glyph ID + face reference + size (1/64th point precision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasKey {
    pub glyph_id: u16,
    pub face_idx: u16,     // compact face index (primary 0-3, fallbacks 4+)
    pub size_q6: u32,      // size * 64, rounded (26.6 fixed point)
}

fn size_to_q6(size: f32) -> u32 {
    (size * 64.0).round() as u32
}

/// Texture format for atlas pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasFormat {
    /// Single-channel coverage (normal text glyphs).
    Grayscale,
    /// Four-channel color (color emoji, COLR/CBLC/sbix).
    Rgba,
}

/// A single atlas page (one texture in the array).
struct AtlasPage {
    format: AtlasFormat,
    /// 2D rectangle packer state (Jylanki's algorithm).
    packer: RectPacker,
    /// Last frame this page was accessed (for LRU eviction).
    last_used_frame: u64,
    /// Number of glyphs stored in this page.
    glyph_count: u32,
}

/// Rectangle packer using the Guillotine algorithm (best short-side fit).
struct RectPacker {
    width: u32,
    height: u32,
    /// Free rectangles available for packing.
    free_rects: Vec<Rect>,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl RectPacker {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            free_rects: vec![Rect { x: 0, y: 0, w: width, h: height }],
        }
    }

    /// Find space for a glyph. Returns position or None if page is full.
    fn pack(&mut self, glyph_w: u32, glyph_h: u32) -> Option<(u32, u32)> {
        // Best Short Side Fit: choose the free rect where the shorter leftover
        // side is minimized, breaking ties by the longer leftover side.
        let mut best_idx = None;
        let mut best_short = u32::MAX;
        let mut best_long = u32::MAX;

        for (i, r) in self.free_rects.iter().enumerate() {
            if r.w >= glyph_w && r.h >= glyph_h {
                let leftover_w = r.w - glyph_w;
                let leftover_h = r.h - glyph_h;
                let short = leftover_w.min(leftover_h);
                let long = leftover_w.max(leftover_h);
                if short < best_short || (short == best_short && long < best_long) {
                    best_idx = Some(i);
                    best_short = short;
                    best_long = long;
                }
            }
        }

        let idx = best_idx?;
        let r = self.free_rects[idx];
        let pos = (r.x, r.y);

        // Guillotine split: split the free rect into two smaller rects.
        self.free_rects.swap_remove(idx);
        let leftover_w = r.w - glyph_w;
        let leftover_h = r.h - glyph_h;

        // Split along the shorter axis.
        if leftover_w < leftover_h {
            // Split horizontally.
            if leftover_w > 0 {
                self.free_rects.push(Rect {
                    x: r.x + glyph_w, y: r.y,
                    w: leftover_w, h: glyph_h,
                });
            }
            if leftover_h > 0 {
                self.free_rects.push(Rect {
                    x: r.x, y: r.y + glyph_h,
                    w: r.w, h: leftover_h,
                });
            }
        } else {
            // Split vertically.
            if leftover_h > 0 {
                self.free_rects.push(Rect {
                    x: r.x, y: r.y + glyph_h,
                    w: glyph_w, h: leftover_h,
                });
            }
            if leftover_w > 0 {
                self.free_rects.push(Rect {
                    x: r.x + glyph_w, y: r.y,
                    w: leftover_w, h: r.h,
                });
            }
        }

        Some(pos)
    }
}

/// Atlas entry: UV coordinates + metrics + page index.
pub struct AtlasEntry {
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
    pub page: u16,          // texture array layer index
    pub metrics: GlyphMetrics,
}

pub struct GlyphMetrics {
    pub width: u16,
    pub height: u16,
    pub bearing_x: i16,
    pub bearing_y: i16,
    pub advance: f32,
}

/// Multi-page atlas with LRU eviction, supporting grayscale + RGBA pages.
pub struct AtlasManager {
    /// wgpu texture array (D2Array) for grayscale pages.
    gray_texture: wgpu::Texture,
    gray_view: wgpu::TextureView,
    gray_pages: Vec<AtlasPage>,

    /// wgpu texture array (D2Array) for RGBA color emoji pages.
    color_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    color_pages: Vec<AtlasPage>,

    /// Glyph -> atlas entry lookup.
    entries: HashMap<AtlasKey, AtlasEntry>,

    /// Page dimensions.
    page_size: u32,   // e.g. 2048
    /// Maximum number of pages per format.
    max_pages: u32,   // e.g. 8
    /// Current frame counter (for LRU tracking).
    frame_counter: u64,
}

impl AtlasManager {
    pub fn new(device: &wgpu::Device, page_size: u32, max_pages: u32) -> Self {
        // Create initial grayscale texture array with 1 layer.
        let gray_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas_gray"),
            size: wgpu::Extent3d {
                width: page_size,
                height: page_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let gray_view = gray_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // Color texture (RGBA) created on demand when first color emoji is encountered.
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas_color"),
            size: wgpu::Extent3d {
                width: page_size,
                height: page_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        Self {
            gray_texture,
            gray_view,
            gray_pages: vec![AtlasPage {
                format: AtlasFormat::Grayscale,
                packer: RectPacker::new(page_size, page_size),
                last_used_frame: 0,
                glyph_count: 0,
            }],
            color_texture,
            color_view,
            color_pages: vec![AtlasPage {
                format: AtlasFormat::Rgba,
                packer: RectPacker::new(page_size, page_size),
                last_used_frame: 0,
                glyph_count: 0,
            }],
            entries: HashMap::new(),
            page_size,
            max_pages,
            frame_counter: 0,
        }
    }

    /// Look up or insert a glyph. Rasterizes via fontdue if not cached.
    pub fn get_or_insert(
        &mut self,
        key: AtlasKey,
        is_color: bool,
        collection: &mut FontCollection,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> &AtlasEntry {
        if !self.entries.contains_key(&key) {
            // Rasterize the glyph.
            let (bitmap, metrics, format) = self.rasterize_glyph(key, is_color, collection);

            // Find or create a page with space.
            let (page_idx, pos) = self.find_space(
                metrics.width as u32,
                metrics.height as u32,
                format,
                device,
                queue,
            );

            // Upload bitmap to the page.
            self.upload_to_page(page_idx, pos, &bitmap, &metrics, format, queue);

            // Create entry.
            let pages = match format {
                AtlasFormat::Grayscale => &self.gray_pages,
                AtlasFormat::Rgba => &self.color_pages,
            };
            let page = &pages[page_idx];
            let _ = page; // page used for validation

            let entry = AtlasEntry {
                uv_pos: [
                    pos.0 as f32 / self.page_size as f32,
                    pos.1 as f32 / self.page_size as f32,
                ],
                uv_size: [
                    metrics.width as f32 / self.page_size as f32,
                    metrics.height as f32 / self.page_size as f32,
                ],
                page: page_idx as u16,
                metrics,
            };

            self.entries.insert(key, entry);
        }

        // Mark the page as recently used.
        if let Some(entry) = self.entries.get(&key) {
            let pages = if is_color { &mut self.color_pages } else { &mut self.gray_pages };
            if let Some(page) = pages.get_mut(entry.page as usize) {
                page.last_used_frame = self.frame_counter;
            }
        }

        self.entries.get(&key).expect("entry just inserted")
    }

    /// Advance frame counter (call once per frame).
    pub fn begin_frame(&mut self) {
        self.frame_counter += 1;
    }

    /// Pre-cache ASCII glyphs for all 4 styles of the primary font.
    pub fn precache_ascii(
        &mut self,
        collection: &mut FontCollection,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        let size_q6 = size_to_q6(collection.metrics().cell_width);
        for style_idx in 0..4u16 {
            for ch in b' '..=b'~' {
                let glyph_id = collection.primary(FontStyle::from_idx(style_idx))
                    .raster_font
                    .lookup_glyph_index(ch as char);
                let key = AtlasKey {
                    glyph_id: glyph_id as u16,
                    face_idx: style_idx,
                    size_q6,
                };
                self.get_or_insert(key, false, collection, queue, device);
            }
        }
    }

    /// Find space in an existing page or allocate a new one.
    /// If all pages are full and max_pages reached, evict the LRU page.
    fn find_space(
        &mut self,
        w: u32,
        h: u32,
        format: AtlasFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (usize, (u32, u32)) {
        let pages = match format {
            AtlasFormat::Grayscale => &mut self.gray_pages,
            AtlasFormat::Rgba => &mut self.color_pages,
        };

        // Try existing pages.
        for (i, page) in pages.iter_mut().enumerate() {
            if let Some(pos) = page.packer.pack(w, h) {
                return (i, pos);
            }
        }

        // All pages full. Can we add a new one?
        if (pages.len() as u32) < self.max_pages {
            let new_idx = pages.len();
            pages.push(AtlasPage {
                format,
                packer: RectPacker::new(self.page_size, self.page_size),
                last_used_frame: self.frame_counter,
                glyph_count: 0,
            });
            // Recreate texture array with +1 layer.
            self.rebuild_texture_array(format, device, queue);
            let pos = pages[new_idx].packer.pack(w, h)
                .expect("fresh page must have space");
            return (new_idx, pos);
        }

        // All pages full and at max. Evict LRU page.
        let lru_idx = pages.iter()
            .enumerate()
            .min_by_key(|(_, p)| p.last_used_frame)
            .map(|(i, _)| i)
            .expect("must have at least one page");

        // Clear the LRU page.
        pages[lru_idx].packer = RectPacker::new(self.page_size, self.page_size);
        pages[lru_idx].glyph_count = 0;
        pages[lru_idx].last_used_frame = self.frame_counter;

        // Remove all entries pointing to this page.
        self.entries.retain(|_, e| e.page as usize != lru_idx);

        let pos = pages[lru_idx].packer.pack(w, h)
            .expect("freshly cleared page must have space");
        (lru_idx, pos)
    }

    /// Rebuild the texture array for a given format with the current page count.
    fn rebuild_texture_array(
        &mut self,
        format: AtlasFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) { /* ... copy existing pages to new larger array ... */ }

    /// Rasterize a single glyph via fontdue.
    fn rasterize_glyph(
        &self,
        key: AtlasKey,
        is_color: bool,
        collection: &mut FontCollection,
    ) -> (Vec<u8>, GlyphMetrics, AtlasFormat) { /* ... */ }

    /// Upload a bitmap to a specific page position.
    fn upload_to_page(
        &self,
        page_idx: usize,
        pos: (u32, u32),
        bitmap: &[u8],
        metrics: &GlyphMetrics,
        format: AtlasFormat,
        queue: &wgpu::Queue,
    ) { /* ... */ }

    pub fn gray_view(&self) -> &wgpu::TextureView { &self.gray_view }
    pub fn color_view(&self) -> &wgpu::TextureView { &self.color_view }

    /// Clear all entries and reset all pages.
    pub fn clear(&mut self) {
        self.entries.clear();
        for page in &mut self.gray_pages {
            page.packer = RectPacker::new(self.page_size, self.page_size);
            page.glyph_count = 0;
        }
        for page in &mut self.color_pages {
            page.packer = RectPacker::new(self.page_size, self.page_size);
            page.glyph_count = 0;
        }
    }
}


// ---- font/builtin_glyphs.rs ----

/// Range of codepoints handled by built-in rendering (bypass font pipeline).
pub fn is_builtin_glyph(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257F}' |  // Box Drawing
        '\u{2580}'..='\u{259F}' |  // Block Elements (already in render_grid.rs)
        '\u{2800}'..='\u{28FF}' |  // Braille Patterns
        '\u{E0A0}'..='\u{E0A3}' |  // Powerline
        '\u{E0B0}'..='\u{E0D4}'    // Powerline Extra
    )
}

/// Render a box drawing character as geometric primitives into the instance buffer.
/// Returns `true` if handled.
pub fn draw_builtin_glyph(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    if !is_builtin_glyph(c) {
        return false;
    }

    match c {
        // Block elements (existing implementation from render_grid.rs).
        '\u{2580}'..='\u{259F}' => draw_block_char(c, x, y, w, h, fg, bg),

        // Box drawing: light/heavy horizontal/vertical lines, corners, tees, crosses.
        '\u{2500}'..='\u{257F}' => draw_box_drawing(c, x, y, w, h, fg, bg),

        // Braille patterns: 2x4 dot grid within the cell.
        '\u{2800}'..='\u{28FF}' => draw_braille(c, x, y, w, h, fg, bg),

        // Powerline: triangles, arrows, rounded separators.
        '\u{E0A0}'..='\u{E0A3}' | '\u{E0B0}'..='\u{E0D4}' => {
            draw_powerline(c, x, y, w, h, fg, bg)
        }

        _ => false,
    }
}

/// Draw a box drawing character from lookup table.
/// Each character is decomposed into line segments: (x1, y1, x2, y2, thickness).
fn draw_box_drawing(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    // Center point of the cell.
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let thin = 1.0f32;
    let thick = 3.0f32;

    // Decode the box drawing character into line segments.
    // Each segment: (from_x, from_y, to_x, to_y, thickness)
    // Coordinates are relative: 0.0 = cell start, 0.5 = center, 1.0 = cell end.
    let idx = c as u32 - 0x2500;
    let segments = box_drawing_segments(idx);

    for (fx, fy, tx, ty, t) in segments {
        let lx = x + w * fx;
        let ly = y + h * fy;
        let rx = x + w * tx;
        let ry = y + h * ty;

        if (ly - ry).abs() < 0.01 {
            // Horizontal line.
            let line_y = ly - t / 2.0;
            bg.push_rect(lx, line_y, rx - lx, t, fg);
        } else if (lx - rx).abs() < 0.01 {
            // Vertical line.
            let line_x = lx - t / 2.0;
            bg.push_rect(line_x, ly, t, ry - ly, fg);
        }
    }

    true
}

/// Draw a braille pattern (2 columns x 4 rows of dots).
fn draw_braille(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    let bits = c as u32 - 0x2800;
    let dot_w = (w / 4.0).round().max(2.0);
    let dot_h = (h / 8.0).round().max(2.0);

    // Braille bit layout:
    // bit 0 = (0,0), bit 1 = (0,1), bit 2 = (0,2),
    // bit 3 = (1,0), bit 4 = (1,1), bit 5 = (1,2),
    // bit 6 = (0,3), bit 7 = (1,3)
    let positions: [(usize, usize, u32); 8] = [
        (0, 0, 0), (0, 1, 1), (0, 2, 2),
        (1, 0, 3), (1, 1, 4), (1, 2, 5),
        (0, 3, 6), (1, 3, 7),
    ];

    for (col, row, bit) in positions {
        if bits & (1 << bit) != 0 {
            let dx = x + w * (0.25 + col as f32 * 0.5) - dot_w / 2.0;
            let dy = y + h * ((row as f32 + 0.5) / 4.0) - dot_h / 2.0;
            bg.push_rect(dx, dy, dot_w, dot_h, fg);
        }
    }

    true
}


// ---- font/async_fallback.rs ----

use std::sync::mpsc;

/// Message from the async fallback resolver to the main thread.
pub enum FallbackResult {
    /// A font was found covering the requested codepoints.
    Found {
        codepoints: Vec<u32>,
        face_info: FontFaceInfo,
    },
    /// No font found for these codepoints.
    NotFound { codepoints: Vec<u32> },
}

/// Handle for the background fallback resolver thread.
pub struct AsyncFallbackResolver {
    /// Send codepoints that need fallback resolution.
    tx: mpsc::Sender<Vec<u32>>,
    /// Receive resolved faces.
    rx: mpsc::Receiver<FallbackResult>,
    /// Codepoints currently being resolved (avoid duplicate requests).
    pending: HashSet<u32>,
}

impl AsyncFallbackResolver {
    /// Spawn the background resolver thread.
    pub fn new(db: Arc<FontDatabase>) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<Vec<u32>>();
        let (res_tx, res_rx) = mpsc::channel::<FallbackResult>();

        std::thread::Builder::new()
            .name("font-fallback".into())
            .spawn(move || {
                while let Ok(codepoints) = req_rx.recv() {
                    let faces = db.find_faces_for_codepoints(&codepoints);
                    if let Some(&face_idx) = faces.first() {
                        let info = db.face_info(face_idx).clone();
                        let _ = res_tx.send(FallbackResult::Found {
                            codepoints,
                            face_info: info,
                        });
                    } else {
                        let _ = res_tx.send(FallbackResult::NotFound { codepoints });
                    }
                }
            })
            .expect("failed to spawn fallback resolver thread");

        Self {
            tx: req_tx,
            rx: res_rx,
            pending: HashSet::new(),
        }
    }

    /// Request fallback resolution for uncovered codepoints.
    /// Deduplicates against pending requests.
    pub fn request(&mut self, codepoints: &[u32]) {
        let new: Vec<u32> = codepoints
            .iter()
            .copied()
            .filter(|cp| self.pending.insert(*cp))
            .collect();
        if !new.is_empty() {
            let _ = self.tx.send(new);
        }
    }

    /// Poll for completed resolutions (non-blocking).
    /// Call once per frame from the event loop.
    pub fn poll(&mut self) -> Vec<FallbackResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.rx.try_recv() {
            match &result {
                FallbackResult::Found { codepoints, .. }
                | FallbackResult::NotFound { codepoints } => {
                    for cp in codepoints {
                        self.pending.remove(cp);
                    }
                }
            }
            results.push(result);
        }
        results
    }
}


// ---- font/config.rs ----

/// Expanded font configuration (replaces the current FontConfig).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Primary font family name.
    pub family: Option<String>,
    /// Font size in points.
    pub size: f32,
    /// OpenType features for the primary font (e.g., ["calt", "liga", "-dlig"]).
    pub features: Vec<String>,
    /// Variable font axis overrides (e.g., { wght = 400, wdth = 100 }).
    pub axes: HashMap<String, f32>,
    /// Ordered fallback font chain.
    pub fallback: Vec<FallbackFontConfig>,
    /// Built-in glyph rendering (box drawing, braille, Powerline).
    pub builtin_glyphs: bool,
    /// Pre-cache strategy: which Unicode blocks to pre-rasterize at startup.
    pub precache: Vec<String>,  // e.g., ["ascii", "latin-1", "box-drawing"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackFontConfig {
    /// Font family name.
    pub family: String,
    /// Size offset from primary (in points). E.g., -1.0 for slightly smaller.
    pub size_offset: Option<f32>,
    /// OpenType feature overrides for this font.
    pub features: Option<Vec<String>>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: None,
            size: 16.0,
            features: vec!["calt".into(), "liga".into()],  // dlig, clig, kern OFF by default
            axes: HashMap::new(),
            fallback: Vec::new(),
            builtin_glyphs: true,
            precache: vec!["ascii".into()],
        }
    }
}


// ---- Updated WGSL shader (fg pipeline) to support texture arrays + color emoji ----

// Fragment shader (pseudocode for the key changes):
//
// @group(1) @binding(0) var glyph_gray: texture_2d_array<f32>;
// @group(1) @binding(1) var glyph_color: texture_2d_array<f32>;
// @group(1) @binding(2) var glyph_sampler: sampler;
//
// struct VertexOutput {
//     @builtin(position) position: vec4<f32>,
//     @location(0) uv: vec2<f32>,
//     @location(1) fg_color: vec4<f32>,
//     @location(2) @interpolate(flat) bg_color: vec4<f32>,
//     @location(3) @interpolate(flat) atlas_page: u32,   // page index
//     @location(4) @interpolate(flat) is_color: u32,     // 0 = grayscale, 1 = RGBA
// }
//
// @fragment
// fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
//     if (input.is_color != 0u) {
//         // Color emoji: sample RGBA directly, ignore fg_color tint.
//         let c = textureSample(glyph_color, glyph_sampler, input.uv, input.atlas_page);
//         return c;
//     }
//
//     // Grayscale glyph: existing alpha-multiply path.
//     var a = textureSample(glyph_gray, glyph_sampler, input.uv, input.atlas_page).r;
//     let color = input.fg_color;
//     // ... linear alpha correction (same as current) ...
//     return vec4<f32>(color.rgb * a, a) * color.a;
// }
```

### Pre-caching Strategy

Beyond ASCII, pre-cache common Unicode blocks during startup based on config:

| Block Name      | Range              | When to Pre-cache           |
|-----------------|--------------------|-----------------------------|
| `ascii`         | U+0020 - U+007E   | Always (default)            |
| `latin-1`       | U+00A0 - U+00FF   | If configured               |
| `box-drawing`   | U+2500 - U+257F   | If `builtin_glyphs = false` |
| `block-elements`| U+2580 - U+259F   | If `builtin_glyphs = false` |
| `braille`       | U+2800 - U+28FF   | If `builtin_glyphs = false` |
| `powerline`     | U+E0A0 - U+E0D4   | If `builtin_glyphs = false` |
| `cjk-common`    | U+4E00 - U+9FFF   | If configured (large!)      |

When `builtin_glyphs = true` (default), box drawing / block elements / braille / Powerline are rendered geometrically and skip the atlas entirely.

## Implementation Roadmap

### Phase 1: Foundation (Atlas + Built-in Glyphs)

- [x] Implement `RectPacker` (Guillotine best-short-side-fit algorithm) replacing row-based shelf packing in `atlas.rs`
- [x] Add multi-page support to `GlyphAtlas`: grow from 1 page to N pages with `wgpu::TextureViewDimension::D2Array`
- [x] Add LRU eviction: track per-page `last_used_frame`, evict oldest page when all full
- [x] Increase page size from 1024x1024 to 2048x2048 (4x glyph capacity per page, ~16 MB per RGBA page)
- [x] Replace size key `(size * 10.0).round() as u16` with `(size * 64.0).round() as u32` in atlas key
- [x] Update WGSL fragment shader to sample `texture_2d_array` with page index
- [x] Add `atlas_page` and `is_color` fields to the instance data (update instance stride and vertex attributes)
- [x] Extend `draw_block_char()` to full box drawing range (U+2500-U+257F) with segment lookup table
- [x] Add braille pattern rendering (U+2800-U+28FF)
- [x] Add Powerline glyph rendering (U+E0A0-U+E0D4: triangles, arrows, rounded separators)
- [x] Extract built-in glyph logic into `gpu/builtin_glyphs.rs` module

### Phase 2: Core (Shaping + Collection)

- [ ] Add `rustybuzz` dependency to `Cargo.toml` (pure Rust HarfBuzz port, cross-compiles cleanly)
- [ ] Implement `FontDatabase`: system font enumeration with cmap-based coverage indexing
- [ ] Implement `DeferredFace` / `LoadedFace` with lazy loading (extend existing `ensure_font_loaded()`)
- [ ] Implement `FontCollection` with primary + fallback chain, metric normalization (cap height scaling)
- [ ] Implement `shape_line()`: segment cells into runs, shape via rustybuzz, map glyphs back to columns
- [ ] Update `build_grid_instances()` in `render_grid.rs` to use shaped glyphs instead of per-cell atlas lookup
- [ ] Handle ligatures in grid rendering: multi-column glyph placement with correct cell spanning
- [ ] Add `Feature` type and per-font feature parsing from config
- [ ] Migrate `FontConfig` to expanded config format (features, fallback chain, axes)
- [ ] Implement combining mark shaping (zero-width characters via shaping runs, replacing current overlay loop)
- [ ] Add grid clamping post-shaping: quantize glyph advances to cell-width multiples to eliminate sub-pixel gaps with hinting enabled
- [ ] Implement OpenType BASE table reading via `ttf-parser` for per-script baseline offsets (CJK ideographic baseline, etc.)

### Phase 2.5: Color Emoji (CBDT/CBLC Bitmap)

- [ ] Add RGBA atlas pages for color emoji (separate texture array, `Rgba8UnormSrgb` format)
- [ ] Implement CBDT/CBLC bitmap extraction from color emoji fonts (read embedded PNG/bitmaps — covers Noto Color Emoji). Store at native resolution, GPU scales in shader.
- [ ] Update WGSL shader with color emoji branch (sample RGBA when `is_color` flag set, bypass fg tint)
- [ ] Add dotted-box-with-hex-codepoint as a built-in glyph for async fallback placeholder rendering

### Phase 3: Polish (COLR Vector Emoji + Async + Variable Fonts)

- [ ] Implement COLR/CPAL vector emoji rendering (layered glyph paths with palette colors — covers Windows Segoe UI Emoji)
- [ ] Implement `AsyncFallbackResolver`: background thread with `mpsc` channel for missing codepoint discovery
- [ ] Integrate async results into event loop: poll per frame, merge discovered faces into `FontCollection`
- [ ] Add variable font axis support: parse `fvar` table, apply axis values during face loading
- [ ] Implement configurable pre-caching (beyond ASCII: Latin-1, CJK common, etc.)
- [ ] Add font hot-reload: watch config file, rebuild `FontCollection` and clear atlas on change
- [ ] Performance optimization: shape only dirty lines (integrate with row-level dirty tracking from grid)
- [ ] Add synthetic italic (shear transform) for fonts without italic variant

## References

- `src/render/mod.rs` -- Current `FontSet` type with glyph cache and fallback chain
- `src/render/font_discovery.rs` -- Platform-specific font file resolution (DirectWrite, filesystem scan)
- `src/render/font_loading.rs` -- Lazy variant loading, family resolution, UI font loading
- `src/gpu/atlas.rs` -- Current `GlyphAtlas` with row-based shelf packing (1024x1024 R8Unorm)
- `src/gpu/render_grid.rs` -- Per-cell rendering loop with atlas lookup, block elements, combining marks
- `src/gpu/pipeline.rs` -- WGSL shaders (bg + fg), instance layout, bind group layouts
- `src/gpu/renderer.rs` -- `GpuRenderer` + `FrameParams` + `PreparedFrame` caching
- `src/cell.rs` -- `CellFlags` (BOLD, ITALIC, WIDE_CHAR, etc.) and `CellExtra` (zerowidth marks)
- `src/config/mod.rs` -- Current `FontConfig { size, family }` and config loading
- Alacritty `crossfont` rasterizer abstraction, `GlyphCache`, built-in box drawing / Powerline glyphs
- Alacritty font fallback issues: #3573 (Windows fallback), #6867 (macOS fallback), #4617 (emoji)
- WezTerm font library: `LoadedFont`, HarfBuzz shaping, async fallback resolution via `schedule_fallback_resolve()`
- WezTerm issues: #4874, #2888 (ligature misalignment), #6931 (small font gaps), #1803 (DPI scaling)
- Ghostty `Collection`: deferred faces, `size_adjustment` metric normalization, 2D rectangle bin packing
- Ghostty issues: #5372 (CJK + dlig interference)
- Jukka Jylanki, "A Thousand Ways to Pack the Bin" (2010) -- rectangle packing algorithms
- `rustybuzz` -- Pure Rust HarfBuzz port (https://github.com/harfbuzz/rustybuzz)
- `roaring-rs` -- Roaring bitmap for codepoint coverage sets (https://github.com/RoaringBitmap/roaring-rs)

## Test Plan — 90% Coverage Target

### Prior Art Survey

| Emulator | Font Tests | Atlas Tests | Shaping Tests | Fallback Tests | Error Recovery |
|----------|-----------|-------------|---------------|----------------|----------------|
| Alacritty | 2 (builtin glyph coverage) | 0 | 0 | 0 | 0 |
| WezTerm | 1 (memory datasource) | 0 | ~10 (ligatures, emoji, CJK) | 0 | 0 |
| Ghostty | 6 (collection) + 7 (discovery) + 10 (metrics) | 13 (pack/grow/OOM) | 45+ (ligatures, complex scripts, emoji) | 2 (codepoint resolver) | 4 (OOM injection, cache rollback) |

Ghostty is the gold standard. We aim to **exceed** their coverage by testing every layer of our four-layer architecture plus the community pain points we committed to solving.

### Testing Infrastructure & Strategy

#### Research Summary

**How Ghostty tests (the gold standard — 130+ tests):**
- **Real fonts, no mocks.** 19 TTF/OTF/BDF/PCF files embedded in `src/font/res/`. Tests use actual font libraries (FreeType/CoreText), never stubs.
- **Atlas tests are pure CPU math.** The atlas is a `Vec<u8>` buffer with CPU-side packing. No GPU needed. Tests verify rectangle placement, data writes at calculated offsets, and grow/resize behavior.
- **Shaping validates structure, not pixels.** Shaper tests check cell count, cluster offsets, col_span — the data structures, not rendered bitmaps. Example: shape `">="`  → assert 1 cell (ligature).
- **Sprite (built-in glyph) tests use PNG snapshot comparison.** 32 reference PNGs at 4 size/thickness combinations. Byte-exact comparison first (fast), then pixel-by-pixel fallback with visual diff output (red=reference, green=test).
- **Error injection via "tripwire" module.** Compile-time failure points at every allocation site. Tests loop over all failure points, inject OOM, verify no memory leaks and state preservation.

**How WezTerm tests:**
- `k9::snapshot!()` for shaping output (inline snapshot updates).
- Real JetBrains Mono / Fira Code fonts for ligature/emoji tests.
- No atlas or GPU tests.

**Rust ecosystem tools:**

| Tool | Purpose | Used By |
|------|---------|---------|
| **`insta`** | Snapshot testing (JSON/YAML/Debug format). `cargo-insta review` CLI. | Widespread |
| **`nv-flip`** | NVIDIA FLIP perceptual image comparison. Models human vision. | **wgpu's own test suite** |
| **`image-compare`** | MSSIM, RMS, histogram distance. MIT licensed. | General |
| **`proptest`** | Property-based testing (random inputs). | General |
| **`image`/`png`** | Read/write reference PNGs for snapshot tests. | General |

**wgpu headless rendering (confirmed working):**
```rust
// Create instance without a surface (headless)
let instance = wgpu::Instance::new(Default::default());
let adapter = instance.request_adapter(&RequestAdapterOptions {
    compatible_surface: None, ..Default::default()
}).await.unwrap();

// Render to a texture with COPY_SRC
let texture = device.create_texture(&TextureDescriptor {
    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC, ..
});

// Read back pixels
let buffer = device.create_buffer(&BufferDescriptor {
    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, ..
});
encoder.copy_texture_to_buffer(&texture, &buffer, ..);
// Map buffer → read &[u8] → compare against reference PNG
```

wgpu's own test suite (`tests/src/image.rs`) uses this exact pattern with `nv-flip` for perceptual comparison. They support `ComparisonType::Mean(threshold)` and `ComparisonType::Percentile { percentile, threshold }`. On first run, the rendered output becomes the reference image.

#### Testing Principles

1. **Real fonts, not mocks** — embed test fonts via `include_bytes!()`. Shaping requires real OpenType tables (GSUB, GPOS, cmap); you cannot mock them meaningfully.
2. **Separate CPU logic from GPU** — atlas packing, font collection, shaping are all testable without wgpu. Only the final rendering step needs headless GPU.
3. **Structure-not-pixels for shaping** — validate glyph IDs, cluster indices, col_span, positions. Don't pixel-compare shaping output.
4. **PNG snapshots for visual output** — built-in glyphs and GPU-rendered frames compared against reference PNGs. Byte-exact first, perceptual fallback.
5. **Error injection at every allocation** — Rust equivalent of Ghostty's tripwire: custom allocator or `Result`-returning constructors with injectable failures.
6. **`proptest` for packing** — random rectangle sequences to verify no overlaps, no out-of-bounds, correct fill rate.
7. **`insta` for metric snapshots** — font metrics, shaping output, config parsing all snapshot-tested for regression detection.

#### Test Dependencies (dev-dependencies)

```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml"] }  # snapshot testing
proptest = "1"                                    # property-based testing
image = "0.25"                                    # PNG read/write for references
image-compare = "0.4"                             # MSSIM/RMS for glyph bitmaps
nv-flip = "0.1"                                   # perceptual comparison (GPU frames)
png = "0.17"                                      # low-level PNG for atlas dumps
pollster = "0.4"                                  # block_on for async wgpu tests
```

#### Layer-by-Layer Testing Approach

**Layer 1: FontDatabase** — Pure CPU. Embed test fonts via `include_bytes!`. Build database from in-memory font data (no filesystem). Validate coverage bitmaps against known codepoint sets. Use `insta` to snapshot `FontFaceInfo` metadata. Test corrupt font handling (truncated bytes).

**Layer 2: FontCollection** — Pure CPU. Load from embedded fonts. Validate metric normalization math: `assert!((scale_factor - expected).abs() < 0.001)`. Test deferred→loaded promotion. Use `insta` to snapshot `PrimaryMetrics`. Test BASE table reading with fonts that have it (Noto CJK).

**Layer 3: ShapingEngine** — Pure CPU (rustybuzz is CPU-only). Shape known strings, snapshot `(glyph_id, x_advance, y_offset, col_start, col_span)` tuples with `insta`. Validate against rustybuzz's own HarfBuzz conformance (2221/2252 tests pass). Test grid clamping: `assert_eq!(clamped_advance % cell_width, 0.0)`.

**Layer 4: AtlasManager** — Split into two test tiers:
- **Tier A (CPU, no GPU):** Extract `RectPacker` as a standalone type. Test with `proptest`: generate random `(width, height)` sequences, verify no overlaps, all within bounds, correct fill rate. Test LRU eviction logic with mock frame counters. Test `AtlasKey` hashing and 26.6 size key distinctness.
- **Tier B (headless GPU):** Use wgpu headless rendering. Upload known bitmaps, read back via `copy_texture_to_buffer`, verify byte-exact match. Test multi-page texture array creation. Test grayscale vs RGBA page separation.

**Layer 5: Built-in Glyphs** — Two approaches:
- **Structure tests:** Verify `is_builtin_glyph()` returns true/false for expected ranges. Verify `draw_builtin_glyph()` produces non-empty instance data.
- **PNG snapshot tests (Ghostty's pattern):** Render each built-in glyph at 4 size combinations (e.g., 9x17, 12x24, 16x32, 20x40) to a CPU-side RGBA buffer. Save as PNG. Compare against reference PNGs. On mismatch, output visual diff (red=reference, green=actual). Store references in `tests/snapshots/builtin_glyphs/`.

**Layer 6: Config Parsing** — Pure CPU. Parse TOML strings, snapshot results with `insta`. Test feature parsing (`"calt"` → enabled, `"-dlig"` → disabled). Test default values. Test invalid config (unknown family, bad feature tag).

**Layer 7: AsyncFallbackResolver** — Spawn real background thread with embedded `FontDatabase`. Send codepoints, poll results. Test deduplication by sending same codepoint twice, asserting one result. Use `std::thread::sleep` + polling loop for deterministic sequencing.

**Layer 8: Full Pipeline GPU Rendering** — Headless wgpu. Render a test grid (e.g., 80x24 with known cell contents including ASCII, CJK, emoji, ligatures, box drawing). Read back framebuffer. Compare against reference PNG using `nv-flip` with `Mean(0.01)` threshold. Store references in `tests/snapshots/rendered_frames/`. This catches regressions in the entire pipeline: shaping → atlas → GPU → output.

#### Error Injection Strategy (Rust equivalent of Ghostty's tripwire)

Ghostty uses compile-time tripwires in Zig. In Rust, we can achieve similar coverage:

```rust
/// Allocator wrapper that fails after N allocations (for testing).
#[cfg(test)]
struct FailingAllocator {
    inner: std::alloc::Global,
    fail_after: AtomicUsize,
}

// Alternative: Result-returning constructors with injectable errors
#[cfg(test)]
impl FontCollection {
    /// For testing: build with a callback that can inject errors at any step.
    fn from_config_with_hooks(
        config: &FontConfig,
        db: &FontDatabase,
        hooks: &TestHooks,
    ) -> Result<Self, FontError> {
        hooks.check("load_primary")?;
        let primary = load_primary(config, db)?;
        hooks.check("load_fallbacks")?;
        // ...
    }
}
```

For atlas/packer testing, the simpler approach: make `RectPacker::pack()` return `Option<(u32, u32)>` (already proposed) and test the `None` path explicitly by filling the packer first.

#### Test Font Bundle

Embed via `include_bytes!` in a `tests/fonts.rs` module:

| Font | Size | Purpose |
|------|------|---------|
| JetBrains Mono Regular | ~150 KB | Primary font (ASCII, Latin, ligatures via calt) |
| JetBrains Mono Bold | ~150 KB | Bold variant, synthetic bold comparison |
| Noto Sans CJK subset | ~500 KB | CJK fallback, cap-height normalization, BASE table |
| Noto Color Emoji subset | ~200 KB | CBDT/CBLC bitmap extraction, color atlas |
| Noto Sans Devanagari | ~100 KB | Complex script shaping (conjuncts) |
| Noto Sans Arabic | ~100 KB | RTL shaping, initial/medial/final forms |
| BIZ UDGothic subset | ~200 KB | The `dlig` CJK breakage regression test (Ghostty #5372) |
| A variable font subset (Recursive) | ~100 KB | Variable font axis support (Phase 3) |

**Total: ~1.5 MB** embedded in test binary. Acceptable for a test suite.

Note: Use `pyftsubset` (from fonttools) to create minimal subsets containing only the codepoints needed for tests, keeping the bundle small.

#### Reference Snapshot Storage

```
tests/
  fonts.rs                           # include_bytes! for all test fonts
  snapshots/
    builtin_glyphs/                  # PNG references for box drawing, braille, powerline
      U+2500_U+257F_12x24.png       # box drawing at 12x24 cell size
      U+2500_U+257F_16x32.png
      U+2800_U+28FF_12x24.png       # braille
      U+E0A0_U+E0D4_12x24.png       # powerline
      ...
    rendered_frames/                  # Full pipeline GPU output references
      ascii_grid_80x24.png
      cjk_mixed_80x24.png
      ligature_fira_code.png
      emoji_color_grid.png
      ...
    shaping/                          # insta snapshots (YAML)
      shape_line@ascii_hello.snap
      shape_line@ligature_ne.snap
      shape_line@cjk_fallback.snap
      shape_line@emoji_zwj.snap
      ...
```

Update snapshots: `cargo insta review` (shaping) or `UPDATE_SNAPSHOTS=1 cargo test` (PNG).
- A variable font (e.g., Recursive subset — axis support)
- A font with `dlig` that triggers CJK breakage (BIZ UDGothic subset — the Ghostty #5372 scenario)

### Layer 1: FontDatabase (~15 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Build database from directory with known fonts | `FontDatabase::build()` finds all font files, parses cmap tables | Ghostty font discovery |
| 2 | Coverage index lookup — ASCII character | `find_faces_for_codepoints()` returns primary font for 'A' | Ghostty codepoint resolver |
| 3 | Coverage index lookup — CJK character | Finds Noto Sans CJK for U+4E00 | Ghostty codepoint resolver |
| 4 | Coverage index lookup — emoji | Finds Noto Color Emoji for U+1F600 | Novel |
| 5 | Coverage index — uncovered codepoint | Returns empty vec for a PUA codepoint with no font | Novel |
| 6 | Find family by name — exact match | `find_family("JetBrains Mono")` returns 4 style variants | Ghostty collection |
| 7 | Find family by name — case insensitive | `find_family("jetbrains mono")` still works | Novel |
| 8 | Find family by name — not found | Returns `None` for nonexistent family | Novel |
| 9 | .ttc collection handling | Parses face_index correctly for multi-face font files | Ghostty face loading |
| 10 | FontFaceInfo metadata accuracy | weight, is_monospace, style parsed correctly from OS/2 table | Ghostty OpenType table tests |
| 11 | Coverage bitmap — RoaringBitmap correctness | Coverage for known font matches expected codepoint set | Novel |
| 12 | Coverage intersection ranking | `find_faces_for_codepoints([U+4E00, U+4E01, U+4E02])` ranks by coverage overlap | Novel |
| 13 | Empty font directory | `FontDatabase::build()` returns empty database, no panic | Ghostty error recovery |
| 14 | Corrupt font file skipped | Directory with one valid and one corrupt .ttf — corrupt skipped, valid loaded | Ghostty error recovery |
| 15 | Monospace detection | JetBrains Mono → `is_monospace = true`, Noto Sans CJK → `is_monospace = false` | Novel |

### Layer 2: FontCollection (~20 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Build from config with primary font | `from_config()` loads 4 style variants of primary font | Ghostty collection |
| 2 | Primary font metrics — cell dimensions | `cell_width`, `cell_height`, `baseline` computed correctly | Ghostty font metrics |
| 3 | Deferred face stays deferred until accessed | Fallback font in Deferred state, coverage query works without full load | Ghostty deferred faces |
| 4 | Deferred → Loaded promotion | `ensure_loaded()` promotes a deferred fallback, subsequent access uses loaded face | Ghostty deferred faces |
| 5 | Find face for ASCII codepoint | `find_face_for_codepoint('A', Regular)` returns `FaceRef::Primary(Regular)` | Ghostty codepoint resolver |
| 6 | Find face for CJK — hits fallback | `find_face_for_codepoint(U+4E00, Regular)` returns `FaceRef::Fallback(idx)` | Ghostty codepoint resolver |
| 7 | Find face — style fallback to Regular | Request Bold for codepoint only in Regular fallback font → returns that fallback | Ghostty style completion |
| 8 | Cap-height metric normalization | Fallback scale_factor = `primary.cap_height / fallback.cap_height`, verified ≠ 1.0 for CJK font | Novel (Ghostty-inspired) |
| 9 | Resize preserves faces | `resize(new_size)` returns new collection with same faces, updated metrics | Novel |
| 10 | Resize clears raster cache | After resize, glyph cache is empty (stale size purged) | Novel |
| 11 | has_real_bold — with bold variant | Returns true when Bold font file exists | Novel |
| 12 | has_real_bold — without bold variant | Returns false, synthetic bold should be used | Novel |
| 13 | features_for — primary font | Returns `[calt, liga]` (defaults) | Novel |
| 14 | features_for — fallback with overrides | Fallback configured with `["-dlig"]` → dlig disabled in returned features | Novel (Ghostty #5372) |
| 15 | size_offset applied to fallback | Fallback with `size_offset = -1.0` rasterizes at `primary_size - 1.0` | Novel (WezTerm #1803) |
| 16 | BASE table baseline offsets read | CJK font with BASE table → ideographic baseline offset extracted | Novel (HarfBuzz maintainer suggestion) |
| 17 | Empty fallback chain | Collection with only primary font, no fallbacks — CJK returns None | Novel |
| 18 | Max fallback chain length | 50+ fallback fonts — no performance cliff, coverage queries still fast | Ghostty collection (capacity) |
| 19 | Primary font missing — error | Config references nonexistent family → returns `FontError` | Ghostty error recovery |
| 20 | Fallback font missing — skipped gracefully | One fallback missing, others load fine, warning logged | Ghostty error recovery |

### Layer 3: ShapingEngine (~25 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Simple ASCII text | `shape_line("hello")` → 5 glyphs, each col_span=1 | WezTerm `ligatures` |
| 2 | ASCII with spaces | `shape_line("a b c")` → 5 glyphs with correct positions | WezTerm `ligatures` |
| 3 | Programming ligature `!=` | 2 chars → 1 glyph, col_span=2, col_start at correct position | WezTerm `ligatures_jetbrains` |
| 4 | Programming ligature `-->` | 3 chars → 1 glyph, col_span=3 | WezTerm `ligatures_jetbrains` |
| 5 | Programming ligature `<!--` | 4 chars → 1 glyph, col_span=4 (HTML comment) | WezTerm `ligatures_jetbrains` |
| 6 | Mixed ligature + normal | `"if x != y"` → runs segmented correctly, `!=` is ligature, rest individual | Novel |
| 7 | CJK wide character | U+4E00 → col_span=2, falls back to CJK font | WezTerm `ligatures` (ideographic space) |
| 8 | Emoji — basic | U+1F600 (grinning face) → col_span=2, is_color=true | WezTerm `ligatures_jetbrains` |
| 9 | Emoji — ZWJ sequence | 👨‍👩‍👧 (family ZWJ) → single glyph, col_span=2 | WezTerm `ligatures_jetbrains` |
| 10 | Emoji — skin tone modifier | 🧏🏼 → single glyph with modifier applied | WezTerm `ligatures_jetbrains` |
| 11 | Emoji — variation selector VS16 | U+2764 U+FE0F (red heart emoji presentation) → col_span=2, is_color=true | Ghostty shaper |
| 12 | Combining marks | `e` + U+0301 (é via combining acute) → single glyph, col_span=1 | Ghostty shaper (diacritics) |
| 13 | Multiple combining marks | `a` + U+0308 + U+0301 (ä́) → single glyph with both marks | Ghostty shaper |
| 14 | Arabic script (RTL) | Basic Arabic text shaped correctly (glyph substitution for initial/medial/final forms) | Ghostty shaper (Arabic) |
| 15 | Devanagari script | Hindi text with conjuncts shaped correctly | Ghostty shaper (Devanagari) |
| 16 | Run segmentation — font change | Line with ASCII + CJK → two runs (primary font, fallback font) | Novel |
| 17 | Run segmentation — style change | Line with normal + bold → two runs (same font, different style) | Ghostty shaper (attribute changes) |
| 18 | Grid clamping — advance quantized | Shaped glyph advance rounded to nearest cell_width multiple | Novel (community issue #3/#6) |
| 19 | dlig OFF by default | Font with dlig rules → no discretionary ligatures in output | Novel (Ghostty #5372) |
| 20 | calt ON by default | Fira Code `!=` → ligature produced (calt feature active) | Novel |
| 21 | Per-font feature override — dlig disabled | CJK fallback with `features=["-dlig"]` → "ます" NOT replaced with "〼" | Novel (Ghostty #5372) |
| 22 | Wide char spacer skipped | WIDE_CHAR_SPACER cells not included in shaping run text | Novel |
| 23 | Empty line | `shape_line("")` → empty vec, no crash | Novel |
| 24 | Line of only spaces | `shape_line("   ")` → 3 space glyphs, fast path | WezTerm `bench_shaping` |
| 25 | Shaped run cache hit | Shape same line twice → second call returns cached result | Novel |

### Layer 4: AtlasManager (~20 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Insert single glyph | `get_or_insert()` returns valid AtlasEntry with UV coords | Ghostty atlas |
| 2 | Insert returns same entry on second call | Cache hit — no re-rasterization | Ghostty atlas |
| 3 | ASCII pre-cache | `precache_ascii()` populates entries for ' ' through '~' × 4 styles | Novel |
| 4 | RectPacker — single glyph | Pack 16x20 glyph into 2048x2048 → position (0,0) | Ghostty atlas |
| 5 | RectPacker — multiple glyphs | Pack 100 glyphs → no overlaps, all within page bounds | Ghostty atlas |
| 6 | RectPacker — page full | Pack until no space → returns None | Ghostty atlas |
| 7 | RectPacker — best short side fit | Two free rects available → chooses the one with minimal short-side leftover | Novel (Jylänki algorithm) |
| 8 | Multi-page growth | Fill page 0, insert another glyph → page 1 created automatically | Ghostty atlas (grow) |
| 9 | Max pages respected | Fill all pages to max_pages → triggers LRU eviction instead of growth | Novel |
| 10 | LRU eviction — oldest page evicted | Pages 0,1,2,3 with different last_used_frame → page with oldest frame evicted | Novel |
| 11 | LRU eviction — entries cleared | After evicting page N, no entries point to page N | Novel |
| 12 | Re-insert after eviction | Glyph on evicted page → re-rasterized on next get_or_insert | Novel |
| 13 | Grayscale vs RGBA page separation | Normal glyph → grayscale page, color emoji → RGBA page | Novel |
| 14 | begin_frame advances counter | `frame_counter` increments, page `last_used_frame` updated on access | Novel |
| 15 | Size key — 26.6 fixed-point | `size_to_q6(16.0) = 1024`, `size_to_q6(16.5) = 1056`, distinct keys | Novel |
| 16 | Size key — fractional DPI sizes distinct | `size_to_q6(13.95)` ≠ `size_to_q6(14.05)` (would collide with old 0.1pt key) | Novel (community issue #6) |
| 17 | Atlas clear | `clear()` empties all entries, resets all packers | Novel |
| 18 | Zero-size glyph | Glyph with 0x0 bitmap → valid entry with zero UV size, no crash | Ghostty atlas |
| 19 | Large glyph — color emoji | 128x128 emoji bitmap inserted into RGBA page at native resolution | Novel |
| 20 | Page UV coordinates correct | Inserted glyph UV = `(x/page_size, y/page_size)`, verified against known position | Ghostty atlas |

### Layer 5: Built-in Glyphs (~15 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | `is_builtin_glyph` — box drawing range | U+2500-U+257F all return true | Alacritty `builtin_line_drawing_glyphs_coverage` |
| 2 | `is_builtin_glyph` — block elements range | U+2580-U+259F all return true | Alacritty `builtin_line_drawing_glyphs_coverage` |
| 3 | `is_builtin_glyph` — braille range | U+2800-U+28FF all return true | Novel |
| 4 | `is_builtin_glyph` — powerline range | U+E0A0-U+E0D4 all return true | Alacritty `builtin_powerline_glyphs_coverage` |
| 5 | `is_builtin_glyph` — exclusion | Regular ASCII, CJK, emoji all return false | Alacritty (exclusion test) |
| 6 | Box drawing — horizontal line (U+2500) | Produces horizontal line segment at cell center | Novel |
| 7 | Box drawing — vertical line (U+2502) | Produces vertical line segment at cell center | Novel |
| 8 | Box drawing — corner (U+250C) | Produces two segments: right + down from center | Novel |
| 9 | Box drawing — double lines (U+2550) | Produces two parallel horizontal segments | Novel |
| 10 | Box drawing — cross (U+253C) | Produces 4 segments from center to edges | Novel |
| 11 | Braille — empty (U+2800) | No dots rendered | Novel |
| 12 | Braille — all dots (U+28FF) | All 8 dot positions filled | Novel |
| 13 | Braille — single dot patterns | Each bit position independently verified | Novel |
| 14 | Powerline — right triangle (U+E0B0) | Triangle fills cell from left edge to right point | Novel |
| 15 | Powerline — rounded separator (U+E0B4) | Arc geometry produced | Novel |

### Layer 6: Config Parsing (~8 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Default config | `FontConfig::default()` → size=16.0, features=["calt","liga"], builtin_glyphs=true | Novel |
| 2 | Full config roundtrip | TOML with family, size, features, fallback chain, axes → parsed correctly | Novel |
| 3 | Feature parsing — enable | `"calt"` → `Feature { tag: b"calt", enabled: true }` | Novel |
| 4 | Feature parsing — disable | `"-dlig"` → `Feature { tag: b"dlig", enabled: false }` | Novel |
| 5 | Fallback with size_offset | `size_offset = -1.0` parsed into `FallbackFontConfig` | Novel |
| 6 | Fallback with feature overrides | `features = ["-dlig"]` parsed per-fallback | Novel |
| 7 | Variable font axes | `axes = { wght = 400, wdth = 100 }` parsed into HashMap | Novel |
| 8 | Empty fallback list | No `[[font.fallback]]` entries → empty vec, no error | Novel |

### Layer 7: AsyncFallbackResolver (~8 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Request + receive found font | Send codepoint, background thread finds font, poll returns Found | Novel |
| 2 | Request + receive not found | Send uncoverable codepoint, poll returns NotFound | Novel |
| 3 | Deduplication | Request same codepoint twice → only one background query | Novel |
| 4 | Pending cleared after response | After receiving result, codepoint removed from pending set | Novel |
| 5 | Poll — non-blocking when empty | `poll()` returns empty vec immediately when no results ready | Novel |
| 6 | Multiple concurrent requests | Send 5 different codepoints, receive 5 results | Novel |
| 7 | Batch efficiency | Request [U+4E00, U+4E01, U+4E02] → single font covers all three, one Found result | Novel |
| 8 | Thread name | Background thread named "font-fallback" | Novel |

### Layer 8: Integration / Error Recovery (~10 tests)

| # | Test Scenario | What It Validates | Inspired By |
|---|--------------|-------------------|-------------|
| 1 | Full pipeline: ASCII text → atlas entries | `shape_line` → `get_or_insert` for each glyph → valid UV coords | Novel |
| 2 | Full pipeline: ligature → atlas entry | `!=` shaped → single glyph_id → atlas entry with col_span=2 | Novel |
| 3 | Full pipeline: CJK fallback → normalized metrics | CJK char → fallback font → scale_factor applied to glyph metrics | Novel |
| 4 | Builtin glyph bypasses font pipeline | Box drawing char → `is_builtin_glyph` returns true, no atlas insertion | Novel |
| 5 | Missing font graceful degradation | Primary font not found → error, not panic | Ghostty error recovery |
| 6 | Corrupt font file recovery | Corrupt .ttf in fallback chain → skipped, warning logged, other fallbacks work | Ghostty error recovery |
| 7 | Atlas rasterization failure | fontdue returns empty bitmap → zero-size entry, no crash | Ghostty error recovery (cache rollback) |
| 8 | Font hot-reload smoke test | Change config font → collection rebuilt, atlas cleared, new glyphs render | Novel |
| 9 | Zero-width char in shaping | Combining mark (U+0301) → width 0, not counted as cell | Novel |
| 10 | Placeholder glyph for async pending | Missing codepoint with async in-flight → dotted box rendered | Novel |

### Total: ~121 test scenarios

| Layer | Count | Coverage Target |
|-------|-------|----------------|
| FontDatabase | 15 | 95% |
| FontCollection | 20 | 95% |
| ShapingEngine | 25 | 90% |
| AtlasManager | 20 | 95% |
| Built-in Glyphs | 15 | 95% |
| Config Parsing | 8 | 100% |
| AsyncFallbackResolver | 8 | 90% |
| Integration / Error Recovery | 10 | 85% |
| **Total** | **121** | **~92%** |

### What We Test That Nobody Else Does

| Scenario | Alacritty | WezTerm | Ghostty | ori_term |
|----------|-----------|---------|---------|----------|
| Atlas LRU eviction | - | - | - | ✓ |
| Multi-page atlas growth | - | - | ✓ | ✓ |
| Fallback metric normalization | - | - | - | ✓ |
| dlig CJK breakage prevention | - | - | - | ✓ |
| Grid clamping (anti-stair-step) | - | - | - | ✓ |
| Async fallback deduplication | - | - | - | ✓ |
| BASE table baseline reading | - | - | - | ✓ |
| Per-font feature overrides | - | - | - | ✓ |
| Color emoji native resolution | - | - | - | ✓ |
| Configurable builtin glyph toggle | - | - | - | ✓ |
| 26.6 fixed-point size key collision prevention | - | - | - | ✓ |
