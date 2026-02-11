---
section: "18"
title: Visual Polish
status: not-started
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
    status: not-started
  - id: "18.4"
    title: HiDPI & Display Scaling
    status: not-started
  - id: "18.5"
    title: Smooth Scrolling
    status: not-started
  - id: "18.6"
    title: Background Images
    status: not-started
  - id: "18.7"
    title: Completion Checklist
    status: not-started
---

# Section 18: Visual Polish

**Status:** Not Started
**Goal:** Small visual features that collectively create a polished, modern feel.
Each is low-to-medium effort but highly visible.

**Why this matters:** These are the details people notice in the first 5 minutes.
Ghostty's "just works" reputation comes from getting dozens of small things right.
Missing cursor blink, broken HiDPI, or unreadable colors are dealbreakers.

---

## 18.1 Cursor Blinking

Toggle cursor visibility on a timer.

- [ ] Blink timer: default 530ms on / 530ms off (configurable)
- [ ] Only blink when DECSCUSR sets a blinking style (odd values: 1, 3, 5)
- [ ] Reset blink to visible state on:
  - [ ] Any keypress
  - [ ] Cursor movement (handler sets cursor position)
  - [ ] PTY output that moves cursor
- [ ] Timer implementation:
  - [ ] Use `winit` `ControlFlow::WaitUntil` for next blink deadline
  - [ ] Or track blink state in draw_frame based on elapsed time
- [ ] Config: `cursor_blink = true | false` (default: true)
- [ ] When terminal loses focus, show steady cursor (no blink)

---

## 18.2 Hide Cursor While Typing

Mouse cursor disappears when typing, reappears on mouse move.

- [ ] On `KeyboardInput` event (non-modifier keys): `window.set_cursor_visible(false)`
- [ ] On `CursorMoved` event: `window.set_cursor_visible(true)`
- [ ] Config: `hide_mouse_when_typing = true | false` (default: true)
- [ ] Don't hide during mouse reporting mode (application is using the mouse)

---

## 18.3 Minimum Contrast

Ensure text is always readable regardless of color scheme.

- [ ] Calculate contrast ratio between fg and bg colors (WCAG formula)
- [ ] If contrast ratio < threshold (default 4.5:1), adjust fg color:
  - [ ] Lighten fg on dark bg, darken fg on light bg
  - [ ] Preserve hue, only adjust luminance
- [ ] Config: `minimum_contrast = 1.0` to `21.0` (default: 1.0 = disabled)
  - [ ] 4.5 = WCAG AA for normal text
  - [ ] 7.0 = WCAG AAA
- [ ] Apply during color resolution in palette.rs `resolve_fg()`
- [ ] Skip for cells that are part of images or colored backgrounds that are intentional

**Ref:** Ghostty's minimum contrast feature, iTerm2's minimum contrast slider

---

## 18.4 HiDPI & Display Scaling

Render correctly on high-DPI displays.

- [ ] Read `window.scale_factor()` from winit
- [ ] Scale grid dimensions: `cell_width * scale`, `cell_height * scale`
- [ ] Scale font size: rasterize at `font_size * scale_factor` points
- [ ] Scale all pixel measurements:
  - [ ] Tab bar height, padding, border width
  - [ ] Resize hit zones, drag thresholds
  - [ ] Cursor width (bar/underline)
  - [ ] Split divider width
- [ ] Handle `ScaleFactorChanged` event:
  - [ ] Re-rasterize all glyphs at new size
  - [ ] Rebuild glyph atlas
  - [ ] Recalculate grid layout
  - [ ] Resize PTY to new columns/rows
- [ ] Multi-monitor: handle moving window between displays with different DPI
- [ ] Config: `dpi_override = auto | <number>` (default: auto)

---

## 18.5 Smooth Scrolling

Pixel-level smooth scrolling instead of line-level jumps.

- [ ] Track fractional scroll offset (pixels within a row)
- [ ] Mouse wheel: accumulate pixel delta, scroll by pixels
- [ ] Animate scroll: when jumping to position, ease with deceleration curve
- [ ] Keyboard scroll (Shift+PgUp): instant jump (no animation) or configurable
- [ ] Render: offset all cell y-positions by fractional scroll amount
- [ ] Snap: when scroll delta is near a line boundary, snap to exact line
- [ ] Config: `smooth_scroll = true | false` (default: true)

---

## 18.6 Background Images

Display a background image behind the terminal grid.

- [ ] Config: `background_image = /path/to/image.png`
- [ ] Config: `background_image_opacity = 0.0..1.0` (default: 0.1)
- [ ] Config: `background_image_position = center | stretch | tile | fill`
- [ ] Load image at startup and on config reload
- [ ] Render as first layer before cell backgrounds
- [ ] Cell backgrounds blend over the image (using opacity)
- [ ] Decode formats: PNG, JPEG, BMP (via `image` crate or minimal decoder)
- [ ] Handle image larger/smaller than window (scale/crop per position setting)

---

## 18.7 Completion Checklist

- [ ] Cursor blinks at configured rate for blinking styles
- [ ] Cursor blink resets on keypress
- [ ] Mouse cursor hides when typing, reappears on move
- [ ] Minimum contrast enforces readable text
- [ ] HiDPI displays render crisp text at correct scale
- [ ] Moving between monitors with different DPI works
- [ ] Smooth scrolling feels natural with mouse wheel
- [ ] Background images render behind terminal content

**Exit Criteria:** Terminal feels visually polished at first launch — cursor blinks,
text is readable, HiDPI works, scrolling is smooth.
