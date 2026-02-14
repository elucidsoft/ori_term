---
section: "05"
title: Color System
status: complete
goal: Full color rendering with 256-color palette, truecolor, and configurable color schemes
sections:
  - id: "05.1"
    title: Color Palette
    status: complete
  - id: "05.2"
    title: Color Rendering
    status: complete
  - id: "05.3"
    title: Color Schemes
    status: complete
  - id: "05.4"
    title: Completion Checklist
    status: complete
---

# Section 05: Color System

**Status:** Complete
**Goal:** Render terminal output with full color support -- 16 named ANSI colors,
256-color indexed palette, and 24-bit truecolor -- with configurable color schemes.

**Inspired by:**
- Ghostty's 256-color palette construction (`terminal/color.zig`)
- Alacritty's extended 269-entry color array with semantic colors
- WezTerm's configurable color schemes

**Implemented in:** `src/palette.rs`, `src/render.rs` (palette-aware rendering)

**What was built:**
- 270-entry palette: 16 ANSI + 216 color cube + 24 grayscale + semantic slots
- `ColorScheme` struct with ANSI colors, fg, bg, cursor
- 7 built-in schemes: Catppuccin Mocha (default), Catppuccin Latte, One Dark, Solarized Dark, Solarized Light, Dracula, Tokyo Night
- `Palette::from_scheme()` constructor, `set_scheme()` for live switching
- `BUILTIN_SCHEMES` constant array for enumeration
- `resolve()`, `resolve_fg()`, `resolve_bg()` with bold-as-bright, DIM, INVERSE, HIDDEN
- `rgb_to_u32()` for softbuffer pixel buffer
- `set_color()`/`reset_color()` for OSC 4/104
- Per-cell color resolution in render_grid
- Cell backgrounds rendered for non-default colors
- Cursor uses palette cursor color with dark text for contrast
- 9 unit tests (including scheme switching tests)

---

## 05.1 Color Palette

Build the standard 256-color palette + semantic colors.

- [ ] Define the 256-color palette as `[Rgb; 256]`:
  - [ ] Colors 0-7: Standard ANSI (dark)
  - [ ] Colors 8-15: Bright ANSI
  - [ ] Colors 16-231: 6x6x6 color cube
    ```
    r = (index - 16) / 36, g = ((index - 16) % 36) / 6, b = (index - 16) % 6
    component = if val == 0 { 0 } else { val * 40 + 55 }
    ```
  - [ ] Colors 232-255: 24-step grayscale ramp
    ```
    value = (index - 232) * 10 + 8
    ```

- [ ] Semantic color slots (like Alacritty's extended array):
  - [ ] `default_fg`, `default_bg`
  - [ ] `cursor_color`, `cursor_text_color`
  - [ ] `selection_fg`, `selection_bg`
  - [ ] `dim_black..dim_white` (auto-generated dim variants)
  - [ ] `bright_black..bright_white` (aliases into palette 8-15)

- [ ] Default ANSI colors: start with Catppuccin Mocha (matching existing theme)
  ```
  black=#45475a, red=#f38ba8, green=#a6e3a1, yellow=#f9e2af
  blue=#89b4fa, magenta=#f5c2e7, cyan=#94e2d5, white=#bac2de
  bright_black=#585b70, bright_red=#f38ba8, ...
  ```

- [ ] Palette is mutable at runtime (OSC 4 can change indexed colors)
- [ ] OSC 104 resets palette to defaults

**Ref:** Ghostty `color.zig`, Alacritty `term/color.rs`

---

## 05.2 Color Rendering

Resolve Cell colors to actual RGB values for rendering.

- [ ] Color resolution function: `resolve_color(color: Color, palette: &Palette) -> Rgb`
  - [ ] `Color::Default` -> semantic default_fg or default_bg
  - [ ] `Color::Named(idx)` -> palette[idx] (respects bold -> bright mapping)
  - [ ] `Color::Indexed(idx)` -> palette[idx]
  - [ ] `Color::Rgb(r,g,b)` -> direct RGB

- [ ] Bold-as-bright: when cell has BOLD flag and foreground is Named(0-7),
  render as Named(8-15) instead. Configurable (some users prefer bold weight only).

- [ ] Dim attribute: when cell has DIM flag, multiply RGB by ~0.66

- [ ] Inverse attribute: when cell has INVERSE flag, swap fg and bg

- [ ] Hidden attribute: when cell has HIDDEN flag, set fg = bg

- [ ] Update `render_grid()` to:
  - [ ] Read cell fg/bg and resolve to RGB
  - [ ] Apply attribute transformations (bold-bright, dim, inverse, hidden)
  - [ ] Use resolved fg for glyph rendering
  - [ ] Use resolved bg for cell background fill

- [ ] Cursor rendering:
  - [ ] Block cursor: draw filled rectangle with cursor color, text in cursor_text_color
  - [ ] Bar cursor: draw thin vertical line
  - [ ] Underline cursor: draw line at cell bottom
  - [ ] Blinking: toggle visibility on timer (configurable rate)

**Ref:** Alacritty color resolution, Ghostty color handling

---

## 05.3 Color Schemes

Support loading different color schemes.

- [ ] Define `ColorScheme` struct holding all 256 palette colors + semantic colors
- [ ] Built-in schemes:
  - [ ] Catppuccin Mocha (default)
  - [ ] Catppuccin Latte (light)
  - [ ] One Dark
  - [ ] Solarized Dark / Light
  - [ ] Dracula
  - [ ] Tokyo Night

- [ ] Scheme loading (deferred to Section 13 for config file, but struct ready now)
- [ ] `set_scheme(scheme: &ColorScheme)` replaces palette and semantic colors
- [ ] Live scheme switching (change palette, request full redraw)

---

## 05.4 Completion Checklist

- [x] `ls --color` shows correct colors
- [x] 256-color test (`for i in {0..255}; do printf '\e[38;5;%dm%3d ' $i $i; done`) renders correctly
- [x] Truecolor test (`printf '\e[38;2;255;100;0mHello'`) renders orange text
- [x] Bold text uses bright colors (or configurable)
- [x] Dim text is visually dimmer
- [x] Inverse video swaps fg/bg
- [x] Cursor renders in configurable color
- [x] OSC 4 can change palette entries (set_color implemented)
- [x] OSC 104 resets palette (reset_color implemented)
- [x] Multiple color schemes built in (7 schemes: Catppuccin Mocha/Latte, One Dark, Solarized Dark/Light, Dracula, Tokyo Night)

**Exit Criteria:** Terminal output renders with full color fidelity. 256-color and
truecolor test scripts produce correct visual output.
