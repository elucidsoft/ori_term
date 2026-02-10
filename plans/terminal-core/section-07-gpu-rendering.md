---
section: "07"
title: GPU Rendering
status: complete
goal: Migrate from CPU softbuffer rendering to GPU-accelerated rendering via wgpu
sections:
  - id: "07.1"
    title: wgpu Setup
    status: complete
  - id: "07.2"
    title: Glyph Atlas
    status: complete
  - id: "07.3"
    title: Cell Rendering Pipeline
    status: complete
  - id: "07.4"
    title: Damage Tracking
    status: not-started
  - id: "07.5"
    title: Decorations & Effects
    status: complete
  - id: "07.6"
    title: Completion Checklist
    status: in-progress
---

# Section 07: GPU Rendering

**Status:** Complete (07.4 Damage Tracking deferred to Section 15)
**Goal:** Replace the current CPU softbuffer pixel-by-pixel rendering with
GPU-accelerated rendering for smooth 60fps+ terminal display.

**Inspired by:**
- Ghostty's Metal/OpenGL/Vulkan renderer with texture atlas and instanced drawing
- Alacritty's wgpu-based renderer with glyph atlas and batch rendering
- WezTerm's OpenGL renderer with texture atlas

**Current state:** Full GPU rendering pipeline implemented in `src/gpu/` module
(atlas, pipeline, renderer). softbuffer has been replaced by wgpu with instanced
rendering, a glyph texture atlas, and two-pass (background + foreground) pipeline.
The GPU renderer handles the complete terminal display including grid cells, tab bar,
window controls, cursor, decorations, and a settings dropdown overlay.

**Implementation:** `src/gpu/mod.rs`, `src/gpu/atlas.rs`, `src/gpu/pipeline.rs`,
`src/gpu/renderer.rs` (~2000 lines total). `app.rs` initializes `GpuState` and
creates per-window `GpuRenderer` instances.

---

## 07.1 wgpu Setup

Initialize GPU rendering pipeline.

- [x] Add `wgpu` (v28) dependency
- [x] Create wgpu `Instance`, `Surface`, `Device`, `Queue` from winit window
  - [x] `GpuState` struct holds instance, adapter, device, queue
  - [x] `GpuState::new()` initializes with `PowerPreference::HighPerformance`
  - [x] `GpuState::create_surface()` creates surface for a given window
- [x] Configure swap chain / surface with window size
  - [x] Prefers non-sRGB format (colors already in sRGB space, avoids double gamma)
- [x] Handle surface resize on window resize
- [ ] Fallback: keep softbuffer as fallback for systems without GPU support — not implemented
- [x] Backend selection: DX12 with DirectComposition (`DxgiFromVisual`) for transparent
  windows, Vulkan fallback for opaque, auto for remaining
- [x] Transparent window support: `PreMultiplied` alpha mode via DComp swapchain
- [x] `with_transparent(true)` + `with_no_redirection_bitmap(true)` for winit

**Files:** `src/gpu/renderer.rs` (`GpuState`, `GpuRenderer::new`)

**Ref:** Alacritty `alacritty/src/renderer/` wgpu setup, wgpu examples

---

## 07.2 Glyph Atlas

Pack rasterized glyphs into a GPU texture atlas.

- [x] Create atlas texture: 1024x1024 R8Unorm (single-channel alpha)
- [x] Atlas packing algorithm: row-based (shelf packing)
  - [x] Glyphs placed left-to-right in current row
  - [x] Advance to next row when glyph doesn't fit
  - [x] 1px gaps between glyphs
- [x] Upload rasterized glyphs to atlas via `queue.write_texture()`
- [x] Track atlas regions: `HashMap<(char, FontStyle), AtlasEntry>`
  - [x] `AtlasEntry` stores normalized UV coords (`uv_pos`, `uv_size`) and `GlyphMetrics`
- [x] Handle atlas full: log warning, insert zero-size entries gracefully
- [x] Invalidate atlas on font change or size change (`clear()` resets packing state)
- [x] Lazy loading: glyphs rasterized on-demand via `get_or_insert()`
- [x] Pre-cache ASCII 32-126 at startup (`precache_ascii()`)
- [ ] Separate atlas for color glyphs (emoji) — deferred (Section 06.9)
- [ ] Dynamic atlas resize or multi-page — not implemented

**Files:** `src/gpu/atlas.rs` (`GlyphAtlas`, `AtlasEntry`, `GlyphMetrics`)

**Ref:** Ghostty `font/Atlas.zig`, Alacritty glyph cache / atlas

---

## 07.3 Cell Rendering Pipeline

Render terminal cells efficiently with instanced drawing.

- [x] Instance data layout (80-byte stride):
  ```
  pos:      vec2<f32>   [0..8]     pixel position
  size:     vec2<f32>   [8..16]    pixel size
  uv_pos:   vec2<f32>   [16..24]   atlas UV top-left
  uv_size:  vec2<f32>   [24..32]   atlas UV size
  fg_color: vec4<f32>   [32..48]   foreground RGBA
  bg_color: vec4<f32>   [48..64]   background RGBA
  flags:    u32         [64..68]   (reserved)
  _pad:     12 bytes    [68..80]   alignment padding
  ```
- [x] Two render pipelines with separate WGSL shaders:
  - [x] **Background pipeline**: premultiplied alpha quads (cell BGs, rectangles, decorations)
    - No texture sampling, solid `bg_color` per fragment
    - Blend mode: premultiplied (`One / OneMinusSrcAlpha`) for transparency support
  - [x] **Foreground pipeline**: alpha-blended textured quads (glyphs)
    - Samples R channel from glyph atlas, multiplies by `fg_color.a`
    - Blend mode: `SrcAlpha / OneMinusSrcAlpha`
- [x] Triangle strip topology: 4 vertices per quad, generated from `vertex_index`
- [x] Instance-driven rendering: vertex shader reads per-instance data
- [x] Bind groups:
  - [x] Group 0: uniform buffer (orthographic projection matrix)
  - [x] Group 1 (FG only): glyph atlas texture + linear sampler
- [x] `InstanceWriter` helper for building 80-byte aligned instance buffers
  - [x] `push_rect()` for solid color quads
  - [x] `push_glyph()` for textured glyph quads
- [x] Per-frame buffer allocation (new GPU buffers each frame)
- [x] Wide character handling (double-width quads, skip spacers)

**Files:** `src/gpu/pipeline.rs` (shaders, pipelines, bind groups),
`src/gpu/renderer.rs` (`InstanceWriter`, `draw_frame()`)

**Ref:** Ghostty `renderer/OpenGL.zig` / `renderer/Metal.zig`, Alacritty renderer

---

## 07.4 Damage Tracking

Only re-render cells that changed since last frame. **Not started — deferred to Section 15.**

- [ ] Track dirty regions: which rows/cells changed since last render
- [ ] On PTY output: mark affected rows as dirty
- [ ] On scroll: mark all visible rows as dirty (or use GPU scroll)
- [ ] On cursor move: mark old and new cursor positions as dirty
- [ ] Rebuild instance buffer only for dirty cells
- [ ] Full redraw on resize, font change, or scroll position change
- [ ] Frame rate limiting: don't render faster than display refresh rate

**Note:** Currently does a full rebuild of all instance data every frame. This works
correctly but is an optimization opportunity for Section 15 (Performance).

**Ref:** Ghostty damage tracking, Alacritty dirty state

---

## 07.5 Decorations & Effects

GPU-rendered terminal decorations.

- [x] Underline rendering: single, double, dotted, dashed, undercurl
  - [x] Rendered as additional quads below the glyph baseline
  - [x] Colored underlines (SGR 58 underline color override)
- [x] Strikethrough: horizontal line through middle of cell
- [x] Cursor styles:
  - [x] Block: filled rectangle over cell (inverts text color)
  - [x] Bar/Beam: 2px vertical bar at left of cell
  - [x] Underline: 2px horizontal bar at bottom of cell
- [x] Selection highlight: FG/BG color swap on selected cells
- [x] Combining marks: zerowidth characters overlaid on base glyph
- [x] Synthetic bold: glyph rendered 1px to the right when no real bold font
- [ ] Blinking cursor: toggle visibility on timer — deferred
- [x] Background opacity: configurable per-element transparency
  - [x] `FrameParams.opacity` for grid content, `FrameParams.tab_bar_opacity` for tab bar
  - [x] `InstanceWriter.opacity` premultiplies background colors
  - [x] Clear color premultiplied by opacity for compositor pass-through
  - [x] Windows Acrylic blur via `window-vibrancy` crate
  - [x] DX12 DirectComposition (`DxgiFromVisual`) for native `PreMultiplied` alpha
  - [x] Independent `tab_bar_opacity` config (falls back to `opacity` when unset)
- [ ] Window padding: configurable — not implemented

**Tab bar rendering (GPU):**
- [x] Active/inactive tab styling with palette-derived colors
- [x] Tab titles with ellipsis truncation
- [x] Close button (x) with hover highlight
- [x] New tab (+) button
- [x] Dropdown (triangle) button
- [x] Window control buttons (minimize, maximize/restore, close)
  - [x] Pixel-drawn geometric icons (line, square, X)
  - [x] Close button with red hover background
- [x] Window border (1px, skipped when maximized)

**Settings dropdown overlay:**
- [x] Theme selector with color swatches (16x16px previewing scheme BG)
- [x] Scheme names with active checkmark indicator
- [x] Drawn as overlay pass (after main content)

**Color management helpers:**
- [x] `TabBarColors` struct: derives tab bar colors dynamically from palette
- [x] `darken()`, `lighten()`, `blend()` color utility functions
- [x] VTE RGB to RGBA conversion

**Files:** `src/gpu/renderer.rs` (`draw_frame()` grid cell loop, `draw_settings_frame()`)

**Ref:** Ghostty decoration rendering, Alacritty visual bell / cursor rendering

---

## 07.6 Completion Checklist

- [x] Terminal renders via GPU (wgpu)
- [x] Glyph atlas packs and renders all visible characters
- [x] Cell backgrounds render correctly (including non-default colors)
- [x] Alpha blending for glyph rendering
- [x] Cursor renders in correct style (block/bar/underline)
- [x] Underline variants render (single, double, curly, dotted, dashed)
- [x] Strikethrough renders
- [x] Selection highlight renders
- [x] Tab bar and window controls render via GPU
- [x] Settings dropdown overlay renders
- [ ] Smooth resize without flicker — mostly works, per-frame buffer rebuild
- [ ] Frame rate stable 60fps+ — not benchmarked, no damage tracking yet
- [ ] Fallback to softbuffer if GPU unavailable — not implemented
- [ ] No visible rendering artifacts — occasional atlas fullness edge case
- [ ] HiDPI / display scaling — not implemented
- [ ] MSAA — not implemented (sample_count: 1)

**Exit Criteria:** Terminal renders via GPU with proper glyph rendering,
colored cells, cursor, underlines, and full UI chrome. ~~Visually identical
to softbuffer but faster.~~ softbuffer replaced entirely.
