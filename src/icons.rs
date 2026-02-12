//! Vector icon rasterizer.
//!
//! Each icon is defined as line segments and rasterized to an alpha bitmap
//! at any pixel size using signed-distance anti-aliasing. Icons are cached
//! in the glyph atlas and rendered through the existing textured-quad pipeline.

/// Available icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Close,
    ChevronDown,
    Plus,
    Checkmark,
}

/// Rasterized icon bitmap (R8 alpha channel, row-major).
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Icon {
    /// Rasterize this icon into an alpha bitmap at the given pixel size.
    pub fn rasterize(self, size: u32) -> IconBitmap {
        match self {
            Self::Close => rasterize_close(size),
            Self::ChevronDown => rasterize_chevron_down(size),
            Self::Plus => rasterize_plus(size),
            Self::Checkmark => rasterize_checkmark(size),
        }
    }
}

/// Distance from point (px, py) to the nearest point on segment (ax,ay)→(bx,by).
fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let nx = ax + t * dx;
    let ny = ay + t * dy;
    (px - nx).hypot(py - ny)
}

/// Rasterize line segments into an alpha bitmap using SDF anti-aliasing.
fn rasterize_segments(
    w: u32,
    h: u32,
    segments: &[(f32, f32, f32, f32)],
    half_thickness: f32,
) -> IconBitmap {
    let mut data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut alpha = 0.0_f32;
            for &(ax, ay, bx, by) in segments {
                let d = dist_to_segment(px, py, ax, ay, bx, by);
                // Smooth falloff: full coverage inside, linear ramp over ~1px at edge
                let a = (half_thickness + 0.5 - d).clamp(0.0, 1.0);
                alpha = alpha.max(a);
            }
            data[(y * w + x) as usize] = (alpha * 255.0).round() as u8;
        }
    }
    IconBitmap { width: w, height: h, data }
}

/// Close icon (×): two diagonal lines.
fn rasterize_close(size: u32) -> IconBitmap {
    let s = size as f32;
    let pad = (s * 0.15).max(1.0);
    let thickness = (s * 0.18).max(1.4);
    let segments = [
        (pad, pad, s - pad, s - pad),
        (s - pad, pad, pad, s - pad),
    ];
    rasterize_segments(size, size, &segments, thickness / 2.0)
}

/// Chevron-down icon (▾): two lines forming a V.
fn rasterize_chevron_down(size: u32) -> IconBitmap {
    let s = size as f32;
    let pad_x = s * 0.12;
    let pad_y = s * 0.30;
    let mid_x = s / 2.0;
    let thickness = (s * 0.18).max(1.4);
    let segments = [
        (pad_x, pad_y, mid_x, s - pad_y),
        (s - pad_x, pad_y, mid_x, s - pad_y),
    ];
    rasterize_segments(size, size, &segments, thickness / 2.0)
}

/// Plus icon (+): horizontal and vertical cross.
fn rasterize_plus(size: u32) -> IconBitmap {
    let s = size as f32;
    let pad = (s * 0.15).max(1.0);
    let mid = s / 2.0;
    let thickness = (s * 0.18).max(1.4);
    let segments = [
        (pad, mid, s - pad, mid),
        (mid, pad, mid, s - pad),
    ];
    rasterize_segments(size, size, &segments, thickness / 2.0)
}

/// Checkmark icon (✓): short left arm + long right arm.
fn rasterize_checkmark(size: u32) -> IconBitmap {
    let s = size as f32;
    let thickness = (s * 0.18).max(1.4);
    // Vertex at bottom-left-ish, short arm goes up-left, long arm goes up-right
    let vx = s * 0.30;
    let vy = s * 0.75;
    let segments = [
        (s * 0.10, s * 0.50, vx, vy),        // short left arm
        (vx, vy, s * 0.88, s * 0.18),         // long right arm
    ];
    rasterize_segments(size, size, &segments, thickness / 2.0)
}
