//! Color conversion utilities and UI color constants for GPU rendering.
//!
//! All colors are in linear space unless noted otherwise.
//! Constants use `rgb_const` for compile-time sRGB-to-linear approximation.

use crate::palette::Palette;

// Close button hover (red background, white icon)
pub(super) const CONTROL_CLOSE_HOVER_BG: [f32; 4] = rgb_const(0xc4, 0x2b, 0x1c);
pub(super) const CONTROL_CLOSE_HOVER_FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

// Default UI colors (non-themed) — used for context menus, popups, panels.
// All values in linear space (compile-time sRGB-to-linear via rgb_const).
pub(super) const UI_BG: [f32; 4] = rgb_const(0x29, 0x29, 0x29);
pub(super) const UI_BG_HOVER: [f32; 4] = rgb_const(0x3D, 0x3D, 0x3D);
pub(super) const UI_SEPARATOR: [f32; 4] = rgb_const(0x3D, 0x3F, 0x43);
pub(super) const UI_TEXT: [f32; 4] = rgb_const(0xE8, 0xEB, 0xED);
pub(super) const UI_TEXT_DIM: [f32; 4] = rgb_const(0xA6, 0xA8, 0xAB);

/// Tab bar colors derived dynamically from the palette.
///
/// The bar is dark, inactive tabs are slightly lighter (solid, fully opaque),
/// the active tab matches the content area exactly.  All colors are alpha 1.0 —
/// no transparency anywhere in the tab bar.
pub(super) struct TabBarColors {
    pub(super) bar_bg: [f32; 4],
    pub(super) active_bg: [f32; 4],
    pub(super) inactive_bg: [f32; 4],
    pub(super) tab_hover_bg: [f32; 4],
    pub(super) button_hover_bg: [f32; 4],
    pub(super) separator: [f32; 4],
    pub(super) text_fg: [f32; 4],
    pub(super) inactive_text: [f32; 4],
    pub(super) close_fg: [f32; 4],
    pub(super) control_fg: [f32; 4],
    pub(super) control_fg_dim: [f32; 4],
    pub(super) control_hover_bg: [f32; 4],
}

impl TabBarColors {
    pub(super) fn from_palette(palette: &Palette) -> Self {
        let base = vte_rgb_to_rgba(palette.default_bg());
        let fg = vte_rgb_to_rgba(palette.default_fg());

        // Derive tab bar colors using OKLab for colored themes (preserves hue),
        // neutral gray shading for near-black/achromatic themes.
        let lab = to_oklab(base);
        let oklab_chroma = lab[1].hypot(lab[2]);
        let active_bg = base;

        let (bar_bg, inactive_bg, tab_hover_bg, button_hover_bg, separator, control_hover_bg);
        if oklab_chroma > 0.01 {
            // Colored theme — shift lightness in OKLab, preserving hue exactly
            bar_bg = oklab_shift(base, -0.06);
            inactive_bg = oklab_shift(base, -0.04);
            tab_hover_bg = oklab_shift(base, 0.04);
            button_hover_bg = oklab_shift(bar_bg, 0.06);
            separator = oklab_shift(bar_bg, 0.03);
            control_hover_bg = oklab_shift(bar_bg, 0.06);
        } else {
            // Near-black/achromatic — use neutral gray shading
            let gray = [0.04, 0.04, 0.04, 1.0];
            bar_bg = darken(gray, 0.35);
            inactive_bg = darken(gray, 0.15);
            tab_hover_bg = lighten(gray, 0.08);
            button_hover_bg = lighten(bar_bg, 0.15);
            separator = lighten(bar_bg, 0.06);
            control_hover_bg = lighten(bar_bg, 0.15);
        }

        let inactive_text = lerp_color(fg, bar_bg, 0.40);
        let close_fg = inactive_text;
        let control_fg = fg;
        let control_fg_dim = lerp_color(fg, bar_bg, 0.50);

        Self {
            bar_bg,
            active_bg,
            inactive_bg,
            tab_hover_bg,
            button_hover_bg,
            separator,
            text_fg: fg,
            inactive_text,
            close_fg,
            control_fg,
            control_fg_dim,
            control_hover_bg,
        }
    }
}

/// Convert an sRGB u8 triplet to linear RGBA at compile time (approximate).
/// Uses a simple gamma-2.2 power curve (close enough for UI constants).
pub(super) const fn rgb_const(r: u8, g: u8, b: u8) -> [f32; 4] {
    // Approximate sRGB-to-linear via x^2.2.  We need const fn so we use a
    // piecewise quadratic approximation: (x/255)^2.2 ~ pow22(x).
    // For exact conversion at runtime, see `srgb_to_linear`.
    const fn pow22(v: u8) -> f32 {
        let x = v as f32 / 255.0;
        // x^2.2 ~ x^2 * x^0.2;  x^0.2 ~ 1 - 0.8*(1-x) for x>0.04
        // Simpler: just use x*x as a rough gamma-2.0 approximation which
        // is close enough for compile-time UI element colors.
        x * x
    }
    [pow22(r), pow22(g), pow22(b), 1.0]
}

/// Convert an sRGB component (0.0-1.0) to linear light.
pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert VTE RGB to [f32; 4] RGBA in **linear** space (alpha=1.0).
pub(super) fn vte_rgb_to_rgba(rgb: vte::ansi::Rgb) -> [f32; 4] {
    [
        srgb_to_linear(f32::from(rgb.r) / 255.0),
        srgb_to_linear(f32::from(rgb.g) / 255.0),
        srgb_to_linear(f32::from(rgb.b) / 255.0),
        1.0,
    ]
}

/// Convert a u32 color (0x00RRGGBB) to [f32; 4] RGBA in **linear** space.
#[cfg(target_os = "windows")]
pub(super) fn u32_to_rgba(c: u32) -> [f32; 4] {
    [
        srgb_to_linear(((c >> 16) & 0xFF) as f32 / 255.0),
        srgb_to_linear(((c >> 8) & 0xFF) as f32 / 255.0),
        srgb_to_linear((c & 0xFF) as f32 / 255.0),
        1.0,
    ]
}

/// Build an orthographic projection matrix (pixels to NDC) as 64 bytes.
/// Maps (0,0)-(w,h) to (-1,1)-(1,-1), column-major for WGSL mat4x4.
pub(super) fn ortho_projection(w: f32, h: f32) -> [u8; 64] {
    let proj: [f32; 16] = [
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        -1.0,
        1.0,
        0.0,
        1.0,
    ];

    let mut bytes = [0u8; 64];
    for (i, &v) in proj.iter().enumerate() {
        let b = v.to_ne_bytes();
        bytes[i * 4..i * 4 + 4].copy_from_slice(&b);
    }
    bytes
}

/// Darken a color by a factor (0.0 = unchanged, 1.0 = black).
pub(super) fn darken(c: [f32; 4], amount: f32) -> [f32; 4] {
    let f = 1.0 - amount;
    [c[0] * f, c[1] * f, c[2] * f, c[3]]
}

/// Lighten a color by a factor (0.0 = unchanged, 1.0 = white).
pub(super) fn lighten(c: [f32; 4], amount: f32) -> [f32; 4] {
    [
        c[0] + (1.0 - c[0]) * amount,
        c[1] + (1.0 - c[1]) * amount,
        c[2] + (1.0 - c[2]) * amount,
        c[3],
    ]
}

/// Linear interpolation between two colors.  t=0 -> a, t=1 -> b.  Alpha always 1.0.
pub(super) fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        1.0,
    ]
}

/// Convert linear RGB to `OKLab` (L, a, b).
#[expect(clippy::excessive_precision, reason = "OKLab matrix coefficients require full precision")]
fn to_oklab(c: [f32; 4]) -> [f32; 3] {
    let l_ = (0.4122214708 * c[0] + 0.5363325363 * c[1] + 0.0514459929 * c[2]).cbrt();
    let m_ = (0.2119034982 * c[0] + 0.6806995451 * c[1] + 0.1073969566 * c[2]).cbrt();
    let s_ = (0.0883024619 * c[0] + 0.2817188376 * c[1] + 0.6299787005 * c[2]).cbrt();
    [
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    ]
}

/// Convert `OKLab` (L, a, b) back to linear RGB with alpha=1, clamped to sRGB gamut.
#[expect(clippy::excessive_precision, reason = "OKLab matrix coefficients require full precision")]
fn from_oklab(lab: [f32; 3]) -> [f32; 4] {
    let l_ = lab[0] + 0.3963377774 * lab[1] + 0.2158037573 * lab[2];
    let m_ = lab[0] - 0.1055613458 * lab[1] - 0.0638541728 * lab[2];
    let s_ = lab[0] - 0.0894841775 * lab[1] - 1.2914855480 * lab[2];
    let l_ = l_ * l_ * l_;
    let m_ = m_ * m_ * m_;
    let s_ = s_ * s_ * s_;
    [
        (4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_).clamp(0.0, 1.0),
        (-1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_).clamp(0.0, 1.0),
        (-0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_).clamp(0.0, 1.0),
        1.0,
    ]
}

/// Adjust a color's perceptual lightness in `OKLab` space while preserving hue and chroma.
/// Positive delta = lighter, negative delta = darker.
fn oklab_shift(c: [f32; 4], delta_l: f32) -> [f32; 4] {
    let mut lab = to_oklab(c);
    lab[0] = (lab[0] + delta_l).clamp(0.0, 1.0);
    from_oklab(lab)
}
