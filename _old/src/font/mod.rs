//! Font collection, text shaping, and glyph management for all text rendering.
//!
//! Provides `FontCollection` (font data + rasterization), `shape_line()`
//! (rustybuzz text shaping for grid cells), and `shape_text_string()` (UI text
//! shaping for tab titles, search bar, menus). All text — grid and UI — goes
//! through: `FontCollection` → rustybuzz shaping → swash rasterization → GPU atlas.

mod collection;
mod shaper;

pub use collection::FontCollection;
pub use shaper::{ShapingRun, UiShapedGlyph, prepare_line, shape_line, shape_prepared_runs, shape_text_string};

use crate::render::FontStyle;

/// Compact face index within a `FontCollection`.
///
/// 0–3 = primary styles (Regular/Bold/Italic/BoldItalic).
/// 4+ = fallback fonts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceIdx(pub u16);

impl FaceIdx {
    /// Returns the `FontStyle` if this is a primary face (0–3).
    pub fn style(self) -> Option<FontStyle> {
        match self.0 {
            0 => Some(FontStyle::Regular),
            1 => Some(FontStyle::Bold),
            2 => Some(FontStyle::Italic),
            3 => Some(FontStyle::BoldItalic),
            _ => None,
        }
    }

    /// Whether this face index refers to a fallback font (not primary).
    pub fn is_fallback(self) -> bool {
        self.0 >= 4
    }
}

/// A shaped glyph ready for atlas lookup and GPU rendering.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID within the font face (not a Unicode codepoint).
    pub glyph_id: u16,
    /// Which face this glyph comes from.
    pub face_idx: FaceIdx,
    /// Grid column where this glyph starts.
    pub col_start: usize,
    /// Number of grid columns this glyph spans (1 for normal, 2+ for ligatures/wide chars).
    pub col_span: usize,
    /// X pixel offset from the cell's left edge (from shaper positioning).
    pub x_offset: f32,
    /// Y pixel offset from the baseline (from shaper positioning).
    pub y_offset: f32,
}
