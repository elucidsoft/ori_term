---
section: "18"
title: Visual Polish
status: in-progress
goal: Implement cursor blinking, hide-while-typing, minimum contrast, HiDPI, smooth scrolling, and background images
sections:
  - id: "18.1"
    title: Cursor Blinking
    status: not-started
  - id: "18.2"
    title: Hide Cursor While Typing
    status: not-started
  - id: "18.3"
    title: Minimum Contrast
    status: complete
  - id: "18.4"
    title: HiDPI & Display Scaling
    status: complete
  - id: "18.5"
    title: Smooth Scrolling
    status: not-started
  - id: "18.6"
    title: Background Images
    status: not-started
  - id: "18.7"
    title: Completion Checklist
    status: in-progress
---

# Section 18: Visual Polish

**Status:** In Progress (18.3 Minimum Contrast complete, 18.4 HiDPI complete)
**Goal:** Small visual features that collectively create a polished, modern feel.
Each is low-to-medium effort but highly visible.

**Why this matters:** These are the details people notice in the first 5 minutes.
Ghostty's "just works" reputation comes from getting dozens of small things right.
Missing cursor blink, broken HiDPI, or unreadable colors are dealbreakers.

---

## 18.1 Cursor Blinking

Toggle cursor visibility on a timer.

**Current state:** Cursor shapes (Block/Underline/Beam) render correctly in
`src/gpu/renderer.rs`. DECSCUSR sets `tab.cursor_shape`. No animation logic
exists — cursor is always visible.

- [ ] Blink state tracking:
  - [ ] Add `cursor_visible: bool` and `cursor_blink_deadline: Instant` to `App`
  - [ ] Blink interval: 530ms on / 530ms off (configurable)
  - [ ] Toggle `cursor_visible` when deadline elapses
- [ ] Only blink when DECSCUSR sets a blinking style:
  - [ ] DECSCUSR values 1 (blinking block), 3 (blinking underline), 5 (blinking bar)
  - [ ] Even values (2, 4, 6) = steady — no blink
  - [ ] Default (0) = implementation-defined — follow config
  - [ ] Store `cursor_blinking: bool` per tab alongside `cursor_shape`
- [ ] Reset blink to visible state on:
  - [ ] Any keypress (reset deadline to now + interval)
  - [ ] PTY output that moves cursor
  - [ ] Mouse click in grid area
- [ ] Timer implementation using winit:
  - [ ] Use `ControlFlow::WaitUntil(next_blink_deadline)` when cursor is blinking
  - [ ] In `about_to_wait()` handler: check if deadline elapsed, toggle visibility,
    request redraw, set next deadline
  - [ ] When no blink needed: revert to `ControlFlow::Wait`
- [ ] Renderer integration:
  - [ ] `FrameParams` gains `cursor_visible: bool`
  - [ ] `build_grid_instances()` skips cursor rendering when `!cursor_visible`
- [ ] Focus handling:
  - [ ] When window loses focus: show steady cursor (no blink), or hide cursor
  - [ ] When window gains focus: restart blink timer
  - [ ] Unfocused window: render cursor as hollow block (outline only)
- [ ] Config: `terminal.cursor_blink = true | false` (default: true)
- [ ] Config: `terminal.cursor_blink_interval = 530` (ms)

---

## 18.2 Hide Cursor While Typing

Mouse cursor disappears when typing, reappears on mouse move.

- [ ] On `KeyboardInput` event (non-modifier keys): `window.set_cursor_visible(false)`
  - [ ] Track `mouse_cursor_hidden: bool` on `App`
  - [ ] Only hide if mouse is over the grid area (not tab bar or resize border)
- [ ] On `CursorMoved` event: `window.set_cursor_visible(true)`
  - [ ] Reset `mouse_cursor_hidden = false`
- [ ] Config: `behavior.hide_mouse_when_typing = true | false` (default: true)
- [ ] Don't hide during mouse reporting mode (application is using the mouse):
  - [ ] Check `TermMode::MOUSE_REPORT | MOUSE_MOTION | MOUSE_ALL`
  - [ ] If any mouse mode is on, don't hide

---

## 18.3 Minimum Contrast

**Status: Complete** — Implemented in GPU shader.

Ensure text is always readable regardless of color scheme.

- [x] Config: `colors.minimum_contrast` (`src/config.rs:52-53`)
  - [x] Range 1.0 (disabled) to 21.0 (maximum contrast)
  - [x] `effective_minimum_contrast()` clamps value (`src/config.rs:105-107`)
  - [x] Default: 1.0 (disabled)
- [x] WCAG 2.0 implementation in WGSL shader (`src/gpu/pipeline.rs:153-216`):
  - [x] `luminance()`: ITU-R BT.709 relative luminance from linear RGB
  - [x] `contrast_ratio()`: WCAG formula `(L1 + 0.05) / (L2 + 0.05)`
  - [x] `contrasted_color()`: adjusts fg toward white or black to meet ratio
    - [x] Binary search for minimum alpha mix that achieves target ratio
    - [x] Tries white first (for dark backgrounds), then black (for light)
    - [x] Picks whichever achieves better contrast
- [x] Per-vertex enforcement: contrast applied in vertex shader (`pipeline.rs:249`)
  ```wgsl
  out.fg_color = contrasted_color(uniforms.min_contrast, input.fg_color, input.bg_color);
  ```
- [x] Uniform buffer passes `min_contrast` from `Config` to shader (`renderer.rs:546-549`)
- [x] Hot-reload: changing `minimum_contrast` in config takes effect immediately

**Files:** `src/gpu/pipeline.rs` (shader), `src/config.rs` (config), `src/gpu/renderer.rs` (uniform)

**Ref:** Ghostty's minimum contrast feature, iTerm2's minimum contrast slider

---

## 18.4 HiDPI & Display Scaling

**Status: Complete** — Implemented with DPI-aware rendering.

Render correctly on high-DPI displays.

- [x] Track `scale_factor: f64` on `App` struct (`src/app.rs:94`)
  - [x] Initial value: 1.0, updated on first window creation
- [x] Handle `ScaleFactorChanged` event (`src/app.rs:2283-2284`):
  - [x] `handle_scale_factor_changed()` method (`src/app.rs:790-795`)
  - [x] Updates `self.scale_factor`
  - [x] Re-rasterizes fonts at `config.font.size * scale_factor`
  - [x] Triggers font set rebuild (atlas clear + re-render)
  - [x] Recalculates grid layout with new cell dimensions
- [x] Font size scaling:
  - [x] Font rasterized at `font_size * scale_factor` (`src/app.rs:221, 236, 763`)
  - [x] Zoom operations account for scale factor
  - [x] `reset_font_size()` resets to `config.font.size * scale_factor`
- [x] Multi-monitor DPI handling:
  - [x] Manual window drag replaces native `drag_window()` to prevent
    `WM_DPICHANGED` oscillation at per-monitor DPI boundaries (`src/app.rs:47-55`)
  - [x] `WindowDrag` struct tracks screen-space cursor and window positions
  - [x] Periodic scale factor check during drag (`src/app.rs:2227-2229`)
- [x] sRGB-correct rendering pipeline:
  - [x] GPU pipeline uses sRGB surface format for gamma-correct blending
  - [x] Luminance-based alpha correction option (`AlphaBlending::LinearCorrected`)
  - [x] Config: `colors.alpha_blending = "linear" | "linear_corrected"` (default: linear_corrected)

**Files:** `src/app.rs` (DPI tracking, WindowDrag), `src/gpu/pipeline.rs` (sRGB, alpha correction)

**Commits:** `bdf598b` (DPI scaling + sRGB), `8ba3441` (OS-native dragging),
`0e81db1` (manual drag to prevent DPI oscillation)

---

## 18.5 Smooth Scrolling

Pixel-level smooth scrolling instead of line-level jumps.

**Current state:** Mouse wheel scrolls 3 lines per tick (instant jump).
Keyboard scroll (Shift+PgUp) jumps one page. All scrolling is line-quantized
via `Grid.display_offset`.

- [ ] Fractional scroll tracking:
  - [ ] Add `scroll_pixel_offset: f32` to rendering state (0.0 to cell_height)
  - [ ] Mouse wheel: accumulate pixel deltas from `MouseScrollDelta::PixelDelta`
    or convert `LineDelta` to pixels
  - [ ] When pixel offset accumulates past `cell_height`, decrement `display_offset`
    and subtract `cell_height` from pixel offset
- [ ] Render with sub-line offset:
  - [ ] Shift all cell y-positions by `scroll_pixel_offset`
  - [ ] Top and bottom rows may be partially visible (clip at grid boundaries)
  - [ ] Need to render one extra row above and below for partial visibility
- [ ] Kinetic scroll (optional):
  - [ ] On mouse wheel release, continue scrolling with decelerating velocity
  - [ ] Friction coefficient: velocity *= 0.95 per frame
  - [ ] Snap to exact line when velocity drops below threshold
- [ ] Keyboard scroll: instant jump (no animation) — keep current behavior
  - [ ] Or: short animation (100ms ease-out) for page jumps
- [ ] Config: `behavior.smooth_scroll = true | false` (default: true)
- [ ] Touchpad support: honor precise pixel deltas from trackpad gestures

---

## 18.6 Background Images

Display a background image behind the terminal grid.

- [ ] Config options:
  ```toml
  [window]
  background_image = "/path/to/image.png"
  background_image_opacity = 0.1
  background_image_position = "center"  # center | stretch | tile | fill
  ```
- [ ] Image loading:
  - [ ] Load at startup and on config reload (hot-reload)
  - [ ] Decode PNG/JPEG/BMP via `image` crate (add dependency)
  - [ ] Convert to RGBA8 texture for wgpu
  - [ ] Handle errors gracefully (missing file, corrupt image)
- [ ] GPU rendering:
  - [ ] Create a wgpu texture from the decoded image
  - [ ] Add a new render pass before cell backgrounds:
    - [ ] Full-screen quad with image texture
    - [ ] Apply `background_image_opacity` as alpha multiplier
  - [ ] Cell backgrounds blend over the image
  - [ ] Position/scale image according to `background_image_position`
- [ ] Position modes:
  - [ ] `center`: original size, centered, crop if larger than window
  - [ ] `stretch`: scale to fill window, may distort aspect ratio
  - [ ] `fill`: scale to fill, maintaining aspect ratio, crop excess
  - [ ] `tile`: repeat at original size
- [ ] Handle window resize: rescale/reposition image
- [ ] Memory: keep decoded texture in GPU memory, not system RAM

---

## 18.7 Completion Checklist

- [ ] Cursor blinks at configured rate for blinking styles
- [ ] Cursor blink resets on keypress
- [ ] Mouse cursor hides when typing, reappears on move
- [x] Minimum contrast enforces readable text (WCAG 2.0 in shader)
- [x] HiDPI displays render crisp text at correct scale
- [x] Moving between monitors with different DPI works
- [ ] Smooth scrolling feels natural with mouse wheel
- [ ] Background images render behind terminal content

**Exit Criteria:** Terminal feels visually polished at first launch — cursor blinks,
text is readable, HiDPI works, scrolling is smooth.
