---
plan: "dpr_color-system_02122026"
title: "Design Pattern Review: Color System"
status: draft
---

# Design Pattern Review: Color System

## ori_term Today

ori_term's color system is a clean two-layer design. `Palette` (`palette.rs`) owns a flat `[Rgb; 270]` array covering ANSI 0-15, 6x6x6 color cube (16-231), 24-step grayscale (232-255), and 14 semantic named colors (Foreground, Background, Cursor, DimBlack..DimWhite, BrightForeground, DimForeground). A second `defaults` array of equal size enables per-slot `reset_color()` back to the scheme baseline. `ColorScheme` is a static `const` struct (16 ANSI + fg/bg/cursor) with 8 built-in schemes (Catppuccin, Solarized, Dracula, etc). On the GPU side, `gpu/color_util.rs` handles sRGB-to-linear conversion via `srgb_to_linear()` / `vte_rgb_to_rgba()`, OKLab transforms for perceptually-correct tab bar derivation (`TabBarColors::from_palette()`), and compile-time approximate gamma via `rgb_const()`. The WGSL shader (`pipeline.rs`) implements WCAG 2.0 minimum contrast enforcement per-vertex and Ghostty-style luminance-corrected alpha blending per-fragment.

What works well: the flat `[Rgb; 270]` table with integer indexing is fast and cache-friendly. Bold-as-bright promotion in `resolve()` correctly gates on `CellFlags::BOLD && idx < 8`. The dim derivation at 2/3 intensity matches the ANSI standard and is automatically recalculated in `apply_overrides()`. GPU-side minimum contrast enforcement (binary search toward white/black in WGSL) keeps the CPU color path simple while giving the shader per-pixel accuracy against actual background colors. The `dynamic_color_sequence()` correctly responds to OSC 10/11/12 queries in XParseColor format. Config overrides (`ColorConfig`) layer cleanly over schemes via `apply_overrides()`.

What's missing or fragile: (1) No change tracking -- `set_color()` mutates palette entries directly with no record of which slots were modified by escape sequences vs. config, making runtime theme switching lossy (user OSC 4 customizations are silently discarded by `set_scheme()`). (2) Selection color logic relies on "FG/BG swap" as default, but when a cell is already inverse, the double-swap cancels out -- this works by coincidence, not by design. (3) `parse_hex_color()` only handles `#RGB`/`#RRGGBB`, but applications send `rgb:RRRR/GGGG/BBBB` and `rgbi:f/f/f` via OSC 4/10/11/12 -- these are silently dropped. (4) The `minimum_contrast` config field is defined and passed to the GPU shader, but there is no CPU-side contrast calculation for scenarios like copy/paste text extraction or accessibility queries. (5) No "resolved color snapshot" API -- render_grid calls `palette.resolve_fg()` and `palette.resolve_bg()` separately per cell, computing the inverse/hidden/dim logic twice for cells that need both fg and bg (the bg call re-resolves fg just to check INVERSE).

## Prior Art

### Alacritty -- Immutable Display Palette

Alacritty separates color state into two distinct layers: the terminal layer stores `Colors([Option<Rgb>; 269])` where each slot is `Option` (unset slots inherit from config), and the display layer materializes a fully-resolved `List([Rgb; COUNT])` with explicit `fill_named()`, `fill_cube()`, `fill_gray_ramp()` methods. The display palette is rebuilt whenever the terminal palette changes -- it is never mutated in place during rendering. This makes the render path zero-decision: every color lookup is a direct array index, no `Option` unwrapping, no fallback logic. The dim colors are auto-derived at 0.66 factor unless the user provides explicit overrides.

This pattern is valuable because it guarantees that palette access during rendering is always safe and predictable. Config errors or malformed escape sequences cannot corrupt the render palette. The cost is slight memory overhead (~2x 270 entries = ~1.6KB, negligible) and a rebuild step on palette mutation. For ori_term, this approach would eliminate the risk of `set_color()` racing with `draw_frame()`.

### Ghostty -- Change-Tracked Mutable Palette

Ghostty's `DynamicPalette` stores `current`, `original`, and a `StaticBitSet` mask tracking which entries have been modified from defaults. When the user switches themes at runtime, `changeDefault()` applies the new base palette but re-applies user customizations by iterating the mask. This preserves OSC 4 color overrides across theme switches -- a real UX improvement over Alacritty's "theme switch resets everything" behavior.

The bitmask approach is lightweight (256 bits = 32 bytes) and O(1) per set/reset. For ori_term, where users can switch themes via the settings UI and applications can set colors via escape sequences, tracking modifications prevents data loss. The pattern also enables a clean `reset_all()` that restores only the unchanged slots to new defaults while preserving intentional customizations.

### WezTerm -- Paired Color Attributes

WezTerm's `ColorAttribute` enum stores `TrueColorWithPaletteFallback(SrgbaTuple, PaletteIndex)` in cells, recording both the exact TrueColor value and a fallback palette index at parse time. The renderer picks which to use based on terminal capability. In a multiplexer, the remote pane may support different color depths than the local frontend.

While ori_term is not a multiplexer, the pattern of recording the original palette index alongside the resolved color has a different value: it enables correct behavior when the palette changes after cells are written. Currently, a cell written with `Color::Indexed(196)` will pick up the new value of palette entry 196 if the scheme changes (correct for Named/Indexed) but a cell written with `Color::Spec(Rgb{...})` will not. This distinction is intentional in xterm semantics, but WezTerm's explicit pairing makes the intent self-documenting.

## Proposed Best-of-Breed Design

### Core Idea

Combine Alacritty's two-tier separation (mutable terminal palette + immutable render snapshot) with Ghostty's change-tracking bitmask, adapted for ori_term's single-process GPU architecture. The terminal-side `Palette` gains a `u256` bitmask tracking which slots were modified by escape sequences (OSC 4, OSC 10/11/12). When the user switches themes via the settings UI, the new scheme is applied to all *unmodified* slots while preserving application customizations. A new `ResolvedPalette` snapshot type is built once per frame from the live `Palette` and passed immutably to `build_grid_instances()`, eliminating any possibility of mid-frame palette mutation.

The GPU-side pipeline stays unchanged -- minimum contrast enforcement and luminance-corrected alpha blending already operate correctly in WGSL. The CPU-side change is purely in how palette state is managed and how colors are resolved before being sent to the GPU. This keeps the hot path (per-cell color resolution in `build_grid_instances`) as a flat array lookup while adding safety and theme-switch correctness at the structural level.

### Key Design Choices

1. **Two-tier palette (Alacritty pattern)**: Split current `Palette` into `Palette` (mutable, owned by `Tab`) and `ResolvedColors` (immutable snapshot, built per-frame). `ResolvedColors` is a flat `[Rgb; NUM_COLORS]` with pre-computed dim variants, passed by reference to the render path. Rationale: prevents mid-frame mutation races and makes the render path a pure function of its snapshot. ori_term's single-process architecture means this snapshot is cheap (memcpy 270 * 3 = 810 bytes).

2. **Change-tracking bitmask (Ghostty pattern)**: Add a `modified: [u64; 5]` field (320 bits, covers all 270 slots) to `Palette`. `set_color()` sets the corresponding bit; `reset_color()` clears it. `set_scheme()` only overwrites slots where the bit is clear. Cost: 40 bytes of additional state. Benefit: theme switching preserves application color customizations. This is the right fit because ori_term has a settings UI for live theme switching.

3. **Unified cell color resolution (novel for ori_term)**: Replace the separate `resolve_fg()` / `resolve_bg()` methods with a single `resolve_cell_colors(fg, bg, flags) -> CellColors` that returns `(fg: Rgb, bg: Rgb)` in one pass. This eliminates the redundant `resolve(fg, flags)` + `resolve(bg, empty)` that currently happens twice (once in `resolve_fg`, once in `resolve_bg`). The INVERSE and HIDDEN flags are applied once. Inspired by how all reference emulators resolve both colors together in the render loop.

4. **Extended hex parsing (Ghostty/xterm pattern)**: Extend `parse_hex_color()` to also accept `rgb:RR/GG/BB`, `rgb:RRRR/GGGG/BBBB`, and `rgbi:f.f/f.f/f.f` formats. These are used by OSC 4/10/11/12 color setting sequences that applications actually send (e.g., tmux, vim). Currently these are silently dropped, causing subtle color breakage. Every production emulator handles these formats.

5. **Flat array stays flat (cross-cutting consensus)**: Keep `[Rgb; 270]` as the core representation. No `HashMap`, no `Option<Rgb>` per slot (Alacritty's `Option` layer adds complexity ori_term doesn't need since we have explicit `defaults` + bitmask). No nested structs. Indexing by `usize` with bounds checks, never `u8` wrapping. This matches the consensus across Alacritty, Ghostty, and WezTerm.

6. **Selection colors resolved against final cell state**: Move selection color logic to operate on already-resolved (post-inverse, post-dim) RGB values. Current `selection_colors(fg, bg)` receives pre-resolution colors and the double-swap fragility is accidental. The new `resolve_cell_colors` returns final RGB, and selection applies on top. This matches Alacritty's approach where selection is a display-layer concern, not a palette concern.

7. **GPU-side contrast stays in shader (ori_term advantage)**: Keep minimum contrast enforcement in the WGSL vertex shader rather than moving it to the CPU. The shader already has the correct implementation (binary search toward white/black with WCAG 2.0 ratio). CPU-side contrast adjustment would require resolving against the actual rendered background, which may differ from the cell's logical bg (due to selection, search highlights, cursor). The GPU knows the final colors.

### What Makes ori_term's Approach Unique

ori_term renders through wgpu to a frameless window with a custom Chrome-style tab bar, meaning the color system must serve two distinct rendering domains in a single frame: the terminal grid (palette-driven, per-cell resolution) and the UI chrome (OKLab-derived, perceptually uniform). No reference emulator has this constraint -- Alacritty and Ghostty use platform-native window decorations; WezTerm has a tab bar but renders it through its own GUI framework.

The `TabBarColors::from_palette()` derivation using OKLab lightness shifts is a genuine innovation. By computing tab bar colors perceptually from the terminal palette, theme switches automatically produce a harmonious chrome without per-theme UI color definitions. The achromatic fallback (for near-black themes where OKLab hue becomes unstable) is a detail no reference emulator addresses because none derive UI colors from terminal palettes.

The WGSL shader pipeline gives ori_term a unique advantage for minimum contrast: enforcement happens per-vertex at no CPU cost, with binary search in parallel across thousands of cells per draw call. Alacritty does contrast adjustment on the CPU. Ghostty does it in Metal/OpenGL shaders but with different math. ori_term's WGSL implementation works across all wgpu backends (Vulkan, DX12, Metal) from a single shader source.

ConPTY on Windows means ori_term never needs to handle `TERM` environment variable detection for color capability (ConPTY always presents as a full-color terminal). This simplifies the color detection chain -- ori_term can assume TrueColor support and focus on palette correctness rather than capability negotiation.

### Concrete Types & Interfaces

```rust
// palette.rs — Core types

pub const NUM_COLORS: usize = 270;

/// Bitmask tracking which palette slots have been modified by escape sequences.
/// 5 x u64 = 320 bits, covering all 270 slots.
#[derive(Debug, Clone)]
struct ModifiedMask([u64; 5]);

impl ModifiedMask {
    const fn new() -> Self { Self([0; 5]) }

    fn set(&mut self, idx: usize) {
        debug_assert!(idx < NUM_COLORS);
        self.0[idx / 64] |= 1 << (idx % 64);
    }

    fn clear(&mut self, idx: usize) {
        debug_assert!(idx < NUM_COLORS);
        self.0[idx / 64] &= !(1 << (idx % 64));
    }

    fn is_set(&self, idx: usize) -> bool {
        debug_assert!(idx < NUM_COLORS);
        self.0[idx / 64] & (1 << (idx % 64)) != 0
    }

    fn clear_all(&mut self) { self.0 = [0; 5]; }
}

/// Mutable palette owned by each Tab. Tracks escape-sequence modifications.
#[derive(Debug, Clone)]
pub struct Palette {
    colors: [Rgb; NUM_COLORS],
    defaults: [Rgb; NUM_COLORS],
    modified: ModifiedMask,
    pub bold_is_bright: bool,
    pub selection_fg: Option<Rgb>,
    pub selection_bg: Option<Rgb>,
}

impl Palette {
    /// Apply a new color scheme, preserving escape-sequence modifications.
    /// Slots marked in `modified` keep their current value.
    pub fn set_scheme(&mut self, scheme: &ColorScheme) {
        let fresh = Self::from_scheme(scheme);
        for i in 0..NUM_COLORS {
            if !self.modified.is_set(i) {
                self.colors[i] = fresh.colors[i];
            }
            self.defaults[i] = fresh.defaults[i];
        }
        self.selection_fg = None;
        self.selection_bg = None;
    }

    /// Set a palette entry via escape sequence (OSC 4). Marks the slot as modified.
    pub fn set_color(&mut self, idx: usize, rgb: Rgb) {
        if idx < NUM_COLORS {
            self.colors[idx] = rgb;
            self.modified.set(idx);
        }
    }

    /// Reset a palette entry to its default. Clears the modified flag.
    pub fn reset_color(&mut self, idx: usize) {
        if idx < NUM_COLORS {
            self.colors[idx] = self.defaults[idx];
            self.modified.clear(idx);
        }
    }

    /// Reset the entire palette (RIS). Clears all modification flags.
    pub fn reset_all(&mut self) {
        self.colors = self.defaults;
        self.modified.clear_all();
    }

    /// Build an immutable snapshot for rendering.
    pub fn snapshot(&self) -> ResolvedColors {
        ResolvedColors {
            colors: self.colors,
            bold_is_bright: self.bold_is_bright,
            selection_fg: self.selection_fg,
            selection_bg: self.selection_bg,
        }
    }
}

/// Immutable color snapshot passed to the render path. Built once per frame.
/// All lookups are direct array indexing — no Options, no fallbacks.
#[derive(Debug, Clone)]
pub struct ResolvedColors {
    colors: [Rgb; NUM_COLORS],
    bold_is_bright: bool,
    selection_fg: Option<Rgb>,
    selection_bg: Option<Rgb>,
}

/// Resolved foreground and background for a single cell.
pub struct CellColors {
    pub fg: Rgb,
    pub bg: Rgb,
}

impl ResolvedColors {
    /// Resolve a Color enum to concrete Rgb.
    pub fn resolve(&self, color: Color, flags: CellFlags) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => self.colors[idx as usize],
            Color::Named(name) => {
                let idx = name as usize;
                if idx < NUM_COLORS {
                    if self.bold_is_bright && flags.contains(CellFlags::BOLD) && idx < 8 {
                        self.colors[idx + 8]
                    } else {
                        self.colors[idx]
                    }
                } else {
                    self.colors[NamedColor::Foreground as usize]
                }
            }
        }
    }

    /// Resolve both fg and bg for a cell in a single pass.
    /// Applies DIM, INVERSE, and HIDDEN flags exactly once.
    pub fn resolve_cell_colors(&self, fg: Color, bg: Color, flags: CellFlags) -> CellColors {
        let mut resolved_fg = self.resolve(fg, flags);
        let resolved_bg = self.resolve(bg, CellFlags::empty());

        if flags.contains(CellFlags::DIM) {
            resolved_fg = dim_color(resolved_fg);
        }

        if flags.contains(CellFlags::INVERSE) {
            CellColors { fg: resolved_bg, bg: resolved_fg }
        } else if flags.contains(CellFlags::HIDDEN) {
            CellColors { fg: resolved_bg, bg: resolved_bg }
        } else {
            CellColors { fg: resolved_fg, bg: resolved_bg }
        }
    }

    /// Selection colors applied on top of already-resolved cell colors.
    pub fn selection_colors(&self, cell_fg: Rgb, cell_bg: Rgb) -> (Rgb, Rgb) {
        (
            self.selection_fg.unwrap_or(cell_bg),
            self.selection_bg.unwrap_or(cell_fg),
        )
    }

    pub fn default_fg(&self) -> Rgb { self.colors[NamedColor::Foreground as usize] }
    pub fn default_bg(&self) -> Rgb { self.colors[NamedColor::Background as usize] }
    pub fn cursor_color(&self) -> Rgb { self.colors[NamedColor::Cursor as usize] }
}
```

```rust
// palette.rs — Extended hex parsing

/// Parse color strings in multiple formats:
/// - "#RGB", "#RRGGBB" (CSS hex)
/// - "rgb:RR/GG/BB", "rgb:RRRR/GGGG/BBBB" (XParseColor, used by OSC 4/10/11/12)
/// - "rgbi:f.f/f.f/f.f" (XParseColor intensity, floating-point 0.0-1.0)
pub fn parse_color_string(s: &str) -> Option<Rgb> {
    if let Some(rest) = s.strip_prefix("rgb:") {
        parse_xparse_rgb(rest)
    } else if let Some(rest) = s.strip_prefix("rgbi:") {
        parse_xparse_rgbi(rest)
    } else {
        parse_hex_color(s)
    }
}

/// Parse "RR/GG/BB" or "RRRR/GGGG/BBBB" (XParseColor rgb: format).
/// Each component is 1-4 hex digits, scaled to 8-bit.
fn parse_xparse_rgb(s: &str) -> Option<Rgb> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 { return None; }

    let r = parse_xparse_component(parts[0])?;
    let g = parse_xparse_component(parts[1])?;
    let b = parse_xparse_component(parts[2])?;
    Some(Rgb { r, g, b })
}

/// Parse a single XParseColor hex component (1-4 digits) and scale to u8.
fn parse_xparse_component(s: &str) -> Option<u8> {
    let len = s.len();
    if !(1..=4).contains(&len) { return None; }
    let val = u16::from_str_radix(s, 16).ok()?;
    // Scale: 1 digit = x * 17, 2 digits = x, 3 digits = x >> 4, 4 digits = x >> 8
    let scaled = match len {
        1 => val * 17,
        2 => val,
        3 => val >> 4,
        4 => val >> 8,
        _ => return None,
    };
    Some(scaled as u8)
}

/// Parse "f.f/f.f/f.f" (XParseColor rgbi: format, 0.0-1.0 per channel).
fn parse_xparse_rgbi(s: &str) -> Option<Rgb> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 { return None; }

    let r = parts[0].parse::<f32>().ok()?.clamp(0.0, 1.0);
    let g = parts[1].parse::<f32>().ok()?.clamp(0.0, 1.0);
    let b = parts[2].parse::<f32>().ok()?.clamp(0.0, 1.0);
    Some(Rgb {
        r: (r * 255.0).round() as u8,
        g: (g * 255.0).round() as u8,
        b: (b * 255.0).round() as u8,
    })
}
```

```rust
// gpu/render_grid.rs — Updated cell color resolution (sketch)

// Before (current):
//   let mut fg_rgb = palette.resolve_fg(cell.fg, cell.bg, cell.flags);
//   let mut bg_rgb = palette.resolve_bg(cell.fg, cell.bg, cell.flags);

// After:
//   let colors = resolved.resolve_cell_colors(cell.fg, cell.bg, cell.flags);
//   let mut fg_rgb = colors.fg;
//   let mut bg_rgb = colors.bg;

// FrameParams changes:
//   palette: &'a Palette,       // before
//   resolved: &'a ResolvedColors, // after
```

```rust
// gpu/color_util.rs — TabBarColors unchanged (already works with Palette methods)
// The from_palette() method would take &ResolvedColors instead of &Palette:
impl TabBarColors {
    pub(super) fn from_resolved(resolved: &ResolvedColors) -> Self {
        let base = vte_rgb_to_rgba(resolved.default_bg());
        let fg = vte_rgb_to_rgba(resolved.default_fg());
        // ... rest unchanged
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation
- [ ] Add `ModifiedMask` type to `palette.rs` with `set`/`clear`/`is_set`/`clear_all` methods and unit tests
- [ ] Add `modified: ModifiedMask` field to `Palette`; wire into `set_color()` and `reset_color()`
- [ ] Update `set_scheme()` to preserve modified slots (skip entries where `modified.is_set(i)`)
- [ ] Add `reset_all()` method that restores defaults and clears the entire bitmask
- [ ] Add `parse_xparse_rgb()`, `parse_xparse_rgbi()`, and `parse_color_string()` to `palette.rs` with comprehensive tests

### Phase 2: Core
- [ ] Create `ResolvedColors` struct with `resolve()`, `resolve_cell_colors()`, `selection_colors()`, `default_fg/bg/cursor`
- [ ] Add `Palette::snapshot() -> ResolvedColors` method
- [ ] Add `CellColors` struct and `resolve_cell_colors()` method to `ResolvedColors`
- [ ] Update `FrameParams` to hold `&ResolvedColors` instead of `&Palette`
- [ ] Update `build_grid_instances()` to use `resolved.resolve_cell_colors()` instead of separate `resolve_fg`/`resolve_bg`
- [ ] Update `TabBarColors::from_palette()` to accept `&ResolvedColors`
- [ ] Update `render_coord.rs` to build `ResolvedColors` snapshot and pass it to `FrameParams`

### Phase 3: Polish
- [ ] Wire `parse_color_string()` into `term_handler.rs` `set_color()` path for OSC 4 color setting
- [ ] Wire `parse_color_string()` into `dynamic_color_sequence()` for OSC 10/11/12 color setting (when parameter is not "?")
- [ ] Add integration tests: theme switch preserves OSC 4 overrides, reset_all clears them
- [ ] Add tests for selection color correctness with inverse cells
- [ ] Run `./clippy-all.sh` and `./test-all.sh` to verify no regressions
- [ ] Audit all `palette.resolve()` / `resolve_fg()` / `resolve_bg()` call sites and migrate to `ResolvedColors`

## References

- `src/palette.rs` -- Palette struct, ColorScheme definitions, resolve(), dim_color(), parse_hex_color()
- `src/gpu/color_util.rs` -- TabBarColors, srgb_to_linear(), vte_rgb_to_rgba(), OKLab transforms
- `src/gpu/render_grid.rs` -- build_grid_instances() cell loop with per-cell color resolution
- `src/gpu/renderer.rs` -- FrameParams struct, draw_frame(), prepare_frame()
- `src/gpu/pipeline.rs` -- WGSL shader: luminance(), contrast_ratio(), contrasted_color(), fs_main()
- `src/gpu/render_settings.rs` -- Settings window rendering, scheme picker color derivation
- `src/gpu/render_tab_bar.rs` -- Tab bar instance building, TabBarColors usage
- `src/cell.rs` -- Cell struct (fg/bg as Color enum), CellFlags, CellExtra
- `src/config.rs` -- ColorConfig, minimum_contrast, AlphaBlending, selection color overrides
- `src/term_handler.rs` -- terminal_attribute() (SGR), dynamic_color_sequence() (OSC 10/11/12), set_color() (OSC 4)
- `src/app/render_coord.rs` -- FrameParams construction, minimum_contrast passthrough
- Alacritty: `alacritty_terminal/src/term/color.rs` (terminal Colors), `alacritty/src/display/color.rs` (display List)
- Ghostty: `src/terminal/color.zig` (DynamicPalette, StaticBitSet mask, changeDefault)
- WezTerm: `term/src/color.rs` (ColorAttribute enum, TrueColorWithPaletteFallback)
- Ratatui: `ratatui-core/src/style/color.rs` (FromStr with alias normalization)
- Crossterm: `src/style/types/color.rs` (Color enum, parse_ansi_iter)
