---
section: 24
title: Visual Polish
status: not-started
tier: 6
goal: Cursor blinking, hide-while-typing, minimum contrast, HiDPI, smooth scrolling, background images
sections:
  - id: "24.1"
    title: Cursor Blinking
    status: not-started
  - id: "24.2"
    title: Hide Cursor While Typing
    status: not-started
  - id: "24.3"
    title: Minimum Contrast
    status: not-started
  - id: "24.4"
    title: HiDPI & Display Scaling
    status: not-started
  - id: "24.5"
    title: Smooth Scrolling
    status: not-started
  - id: "24.6"
    title: Background Images
    status: not-started
  - id: "24.7"
    title: Section Completion
    status: not-started
---

# Section 24: Visual Polish

**Status:** Not Started
**Goal:** Small visual features that collectively create a polished, modern feel. Each is low-to-medium effort but highly visible. These are the details people notice in the first 5 minutes. Missing cursor blink, broken HiDPI, or unreadable colors are dealbreakers.

**Crate:** `oriterm` (rendering + app layer)
**Dependencies:** `image` (for background images), existing wgpu pipeline

---

## 24.1 Cursor Blinking

Toggle cursor visibility on a timer. Cursor shapes (Block/Underline/Beam) render correctly but no animation logic exists -- cursor is always visible.

**File:** `oriterm/src/app.rs` (blink state), `oriterm/src/gpu/renderer.rs` (render skip)

**Reference:** `_old/src/app.rs` (cursor rendering), `_old/src/gpu/renderer.rs` (FrameParams)

- [ ] Blink state tracking:
  - [ ] Add `cursor_visible: bool` and `cursor_blink_deadline: Instant` to `App`
  - [ ] Blink interval: 530ms on / 530ms off (configurable)
  - [ ] Toggle `cursor_visible` when deadline elapses
- [ ] Only blink when DECSCUSR sets a blinking style:
  - [ ] DECSCUSR values 1 (blinking block), 3 (blinking underline), 5 (blinking bar)
  - [ ] Even values (2, 4, 6) = steady -- no blink
  - [ ] Default (0) = implementation-defined -- follow config
  - [ ] Store `cursor_blinking: bool` per tab alongside `cursor_shape`
- [ ] Reset blink to visible state on:
  - [ ] Any keypress (reset deadline to now + interval)
  - [ ] PTY output that moves cursor
  - [ ] Mouse click in grid area
- [ ] Timer implementation using winit:
  - [ ] Use `ControlFlow::WaitUntil(next_blink_deadline)` when cursor is blinking
  - [ ] In `about_to_wait()` handler: check if deadline elapsed, toggle visibility, request redraw, set next deadline
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

**Tests:**
- [ ] Blink state toggles after interval elapses
- [ ] Keypress resets blink to visible
- [ ] Even DECSCUSR values disable blinking
- [ ] Odd DECSCUSR values enable blinking
- [ ] Focus loss stops blinking, shows steady cursor

---

## 24.2 Hide Cursor While Typing

Mouse cursor disappears when typing, reappears on mouse move.

**File:** `oriterm/src/app.rs` (input handling)

- [ ] On `KeyboardInput` event (non-modifier keys): `window.set_cursor_visible(false)`
  - [ ] Track `mouse_cursor_hidden: bool` on `App`
  - [ ] Only hide if mouse is over the grid area (not tab bar or resize border)
- [ ] On `CursorMoved` event: `window.set_cursor_visible(true)`
  - [ ] Reset `mouse_cursor_hidden = false`
- [ ] Don't hide during mouse reporting mode (application is using the mouse):
  - [ ] Check `TermMode::MOUSE_REPORT | MOUSE_MOTION | MOUSE_ALL`
  - [ ] If any mouse mode is on, don't hide
- [ ] Config: `behavior.hide_mouse_when_typing = true | false` (default: true)

**Tests:**
- [ ] Keypress hides mouse cursor
- [ ] Mouse move restores mouse cursor
- [ ] Mouse reporting mode prevents hiding
- [ ] Config false disables the feature entirely

---

## 24.3 Minimum Contrast

Ensure text is always readable regardless of color scheme. WCAG 2.0 contrast enforcement in the GPU shader.

**File:** `oriterm/src/gpu/pipeline.rs` (WGSL shader), `oriterm/src/config.rs` (config), `oriterm/src/gpu/renderer.rs` (uniform)

**Reference:** Ghostty's minimum contrast feature, iTerm2's minimum contrast slider

- [ ] Config: `colors.minimum_contrast` (range 1.0 disabled to 21.0 maximum, default 1.0)
  - [ ] `effective_minimum_contrast()` clamps value
- [ ] WCAG 2.0 implementation in WGSL shader:
  - [ ] `luminance()`: ITU-R BT.709 relative luminance from linear RGB
  - [ ] `contrast_ratio()`: WCAG formula `(L1 + 0.05) / (L2 + 0.05)`
  - [ ] `contrasted_color()`: adjusts fg toward white or black to meet ratio
    - [ ] Binary search for minimum alpha mix that achieves target ratio
    - [ ] Tries white first (for dark backgrounds), then black (for light)
    - [ ] Picks whichever achieves better contrast
- [ ] Per-vertex enforcement: contrast applied in vertex shader
  ```wgsl
  out.fg_color = contrasted_color(uniforms.min_contrast, input.fg_color, input.bg_color);
  ```
- [ ] Uniform buffer passes `min_contrast` from config to shader
- [ ] Hot-reload: changing `minimum_contrast` in config takes effect immediately

**Tests:**
- [ ] White on black at minimum_contrast 1.0 passes through unchanged
- [ ] Dark gray on black at minimum_contrast 4.5 adjusts fg to lighter color
- [ ] Light gray on white at minimum_contrast 4.5 adjusts fg to darker color

---

## 24.4 HiDPI & Display Scaling

Render correctly on high-DPI displays and handle multi-monitor DPI transitions.

**File:** `oriterm/src/app.rs` (DPI tracking, WindowDrag), `oriterm/src/gpu/pipeline.rs` (sRGB)

- [ ] Track `scale_factor: f64` on `App` struct
  - [ ] Initial value: 1.0, updated on first window creation
- [ ] Handle `ScaleFactorChanged` event:
  - [ ] Update `self.scale_factor`
  - [ ] Re-rasterize fonts at `config.font.size * scale_factor`
  - [ ] Trigger font set rebuild (atlas clear + re-render)
  - [ ] Recalculate grid layout with new cell dimensions
- [ ] Font size scaling:
  - [ ] Font rasterized at `font_size * scale_factor`
  - [ ] Zoom operations account for scale factor
  - [ ] `reset_font_size()` resets to `config.font.size * scale_factor`
- [ ] Multi-monitor DPI handling:
  - [ ] Manual window drag replaces native `drag_window()` to prevent `WM_DPICHANGED` oscillation at per-monitor DPI boundaries
  - [ ] `WindowDrag` struct tracks screen-space cursor and window positions
  - [ ] Periodic scale factor check during drag
- [ ] sRGB-correct rendering pipeline:
  - [ ] GPU pipeline uses sRGB surface format for gamma-correct blending
  - [ ] Luminance-based alpha correction option (`AlphaBlending::LinearCorrected`)
  - [ ] Config: `colors.alpha_blending = "linear" | "linear_corrected"` (default: linear_corrected)

**Tests:**
- [ ] Scale factor change triggers font re-rasterization
- [ ] Grid dimensions recalculated after DPI change
- [ ] Window drag across monitors with different DPI works without oscillation

---

## 24.5 Smooth Scrolling

Pixel-level smooth scrolling instead of line-level jumps. Mouse wheel currently scrolls 3 lines per tick (instant jump). All scrolling is line-quantized via `Grid.display_offset`.

**File:** `oriterm/src/app.rs` (scroll input), `oriterm/src/gpu/renderer.rs` (render offset)

- [ ] Fractional scroll tracking:
  - [ ] Add `scroll_pixel_offset: f32` to rendering state (0.0 to cell_height)
  - [ ] Mouse wheel: accumulate pixel deltas from `MouseScrollDelta::PixelDelta` or convert `LineDelta` to pixels
  - [ ] When pixel offset accumulates past `cell_height`, decrement `display_offset` and subtract `cell_height` from pixel offset
- [ ] Render with sub-line offset:
  - [ ] Shift all cell y-positions by `scroll_pixel_offset`
  - [ ] Top and bottom rows may be partially visible (clip at grid boundaries)
  - [ ] Render one extra row above and below for partial visibility
- [ ] Kinetic scroll (optional):
  - [ ] On mouse wheel release, continue scrolling with decelerating velocity
  - [ ] Friction coefficient: velocity *= 0.95 per frame
  - [ ] Snap to exact line when velocity drops below threshold
- [ ] Keyboard scroll: instant jump (no animation) -- keep current behavior
  - [ ] Or: short animation (100ms ease-out) for page jumps
- [ ] Touchpad support: honor precise pixel deltas from trackpad gestures
- [ ] Config: `behavior.smooth_scroll = true | false` (default: true)

**Tests:**
- [ ] Pixel delta accumulation triggers line scroll at cell_height boundary
- [ ] Sub-line offset renders partial rows correctly
- [ ] Kinetic scroll decelerates and snaps to whole line
- [ ] Keyboard scroll bypasses smooth scrolling

---

## 24.6 Background Images

Display a background image behind the terminal grid.

**File:** `oriterm/src/gpu/renderer.rs` (render pass), `oriterm/src/gpu/pipeline.rs` (shader), `oriterm/src/config.rs` (config)

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

**Tests:**
- [ ] Image loads from valid path, returns error for missing path
- [ ] Position mode center computes correct UV coordinates
- [ ] Position mode fill maintains aspect ratio
- [ ] Opacity multiplier applied correctly in shader
- [ ] Config reload swaps background image without restart

---

## 24.7 Section Completion

- [ ] All 24.1-24.6 items complete
- [ ] Cursor blinks at configured rate for blinking styles
- [ ] Cursor blink resets on keypress
- [ ] Mouse cursor hides when typing, reappears on move
- [ ] Minimum contrast enforces readable text (WCAG 2.0 in shader)
- [ ] HiDPI displays render crisp text at correct scale
- [ ] Moving between monitors with different DPI works
- [ ] Smooth scrolling feels natural with mouse wheel
- [ ] Background images render behind terminal content
- [ ] All features configurable and hot-reloadable

**Exit Criteria:** Terminal feels visually polished at first launch -- cursor blinks, text is readable, HiDPI works, scrolling is smooth.
