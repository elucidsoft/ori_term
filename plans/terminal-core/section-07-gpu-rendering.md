---
section: "07"
title: GPU Rendering
status: not-started
goal: Migrate from CPU softbuffer rendering to GPU-accelerated rendering via wgpu
sections:
  - id: "07.1"
    title: wgpu Setup
    status: not-started
  - id: "07.2"
    title: Glyph Atlas
    status: not-started
  - id: "07.3"
    title: Cell Rendering Pipeline
    status: not-started
  - id: "07.4"
    title: Damage Tracking
    status: not-started
  - id: "07.5"
    title: Decorations & Effects
    status: not-started
  - id: "07.6"
    title: Completion Checklist
    status: not-started
---

# Section 07: GPU Rendering

**Status:** Not Started
**Goal:** Replace the current CPU softbuffer pixel-by-pixel rendering with
GPU-accelerated rendering for smooth 60fps+ terminal display.

**Inspired by:**
- Ghostty's Metal/OpenGL/Vulkan renderer with texture atlas and instanced drawing
- Alacritty's wgpu-based renderer with glyph atlas and batch rendering
- WezTerm's OpenGL renderer with texture atlas

**Current state:** `render.rs` iterates every cell, rasterizes with fontdue, alpha-blends
pixel-by-pixel into a `&mut [u32]` softbuffer. Works but is CPU-bound and scales
poorly with window size.

---

## 07.1 wgpu Setup

Initialize GPU rendering pipeline.

- [ ] Add `wgpu` dependency (cross-platform GPU abstraction over Vulkan/Metal/DX12/OpenGL)
- [ ] Create wgpu `Instance`, `Surface`, `Device`, `Queue` from winit window
- [ ] Configure swap chain / surface with window size
- [ ] Handle surface resize on window resize
- [ ] Fallback: keep softbuffer as fallback for systems without GPU support
- [ ] Choose backend: Vulkan on Windows/Linux, Metal on macOS, DX12 as Windows fallback

**Ref:** Alacritty `alacritty/src/renderer/` wgpu setup, wgpu examples

---

## 07.2 Glyph Atlas

Pack rasterized glyphs into a GPU texture atlas.

- [ ] Create atlas texture (e.g., 1024x1024 RGBA)
- [ ] Atlas packing algorithm: shelf-based or skyline for simplicity
- [ ] Upload rasterized glyphs to atlas texture
- [ ] Track atlas regions: `HashMap<GlyphKey, AtlasRegion>` where `AtlasRegion` has UV coords
- [ ] Handle atlas full: create new atlas page or resize
- [ ] Separate atlas for color glyphs (emoji) vs alpha-only glyphs
- [ ] Invalidate atlas on font change or size change

**Ref:** Ghostty `font/Atlas.zig`, Alacritty glyph cache / atlas

---

## 07.3 Cell Rendering Pipeline

Render terminal cells efficiently with instanced drawing.

- [ ] Define vertex/instance data for cell rendering:
  ```
  CellInstance {
      grid_pos: [f32; 2],    // cell column, row
      glyph_uv: [f32; 4],   // atlas UV rect
      fg_color: [f32; 4],   // foreground RGBA
      bg_color: [f32; 4],   // background RGBA
  }
  ```
- [ ] Background pass: render cell backgrounds as colored quads
  - [ ] Only emit quads for cells with non-default background
  - [ ] Wide cells get double-width quads
- [ ] Foreground pass: render glyphs as textured quads with alpha blending
  - [ ] Sample from atlas texture
  - [ ] Multiply alpha by foreground color
- [ ] Cursor pass: render cursor as colored quad (block, bar, or underline)
- [ ] Batch all cells into a single draw call per pass (instanced rendering)
- [ ] Uniform buffer: projection matrix, cell dimensions, atlas size

**Ref:** Ghostty `renderer/OpenGL.zig` / `renderer/Metal.zig`, Alacritty renderer

---

## 07.4 Damage Tracking

Only re-render cells that changed since last frame.

- [ ] Track dirty regions: which rows/cells changed since last render
- [ ] On PTY output: mark affected rows as dirty
- [ ] On scroll: mark all visible rows as dirty (or use GPU scroll)
- [ ] On cursor move: mark old and new cursor positions as dirty
- [ ] Rebuild instance buffer only for dirty cells
- [ ] Full redraw on resize, font change, or scroll position change
- [ ] Frame rate limiting: don't render faster than display refresh rate

**Ref:** Ghostty damage tracking, Alacritty dirty state

---

## 07.5 Decorations & Effects

GPU-rendered terminal decorations.

- [ ] Underline rendering: single, double, curly, dotted, dashed
  - [ ] Render as additional quads below the glyph baseline
  - [ ] Colored underlines (SGR 58)
- [ ] Strikethrough: horizontal line through middle of cell
- [ ] Cursor styles: block (filled), bar (thin vertical), underline (thin horizontal)
- [ ] Blinking cursor: toggle visibility on timer
- [ ] Selection highlight: colored background overlay on selected cells
- [ ] Window padding: configurable padding around the terminal grid
- [ ] Background opacity: support transparent/semi-transparent backgrounds

**Ref:** Ghostty decoration rendering, Alacritty visual bell / cursor rendering

---

## 07.6 Completion Checklist

- [ ] Terminal renders via GPU (wgpu)
- [ ] Glyph atlas packs and renders all visible characters
- [ ] Cell backgrounds render correctly (including non-default colors)
- [ ] Alpha blending for subpixel glyph rendering
- [ ] Cursor renders in correct style (block/bar/underline)
- [ ] Underline variants render (single, double, curly)
- [ ] Smooth resize without flicker
- [ ] Frame rate is stable 60fps+ with full screen of colored text
- [ ] Fallback to softbuffer if GPU unavailable
- [ ] No visible rendering artifacts

**Exit Criteria:** Terminal renders at 60fps+ via GPU with proper glyph rendering,
colored cells, cursor, and underlines. Visually identical to softbuffer but faster.
