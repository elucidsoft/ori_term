//! Font discovery, validation, and shared types for text rendering.

pub(crate) mod font_discovery;

use crate::cell::CellFlags;

pub const FONT_SIZE: f32 = 16.0;
pub(crate) const MIN_FONT_SIZE: f32 = 8.0;
pub(crate) const MAX_FONT_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Regular = 0,
    Bold = 1,
    Italic = 2,
    BoldItalic = 3,
}

impl FontStyle {
    /// Map cell flags to the appropriate font style.
    pub fn from_cell_flags(flags: CellFlags) -> Self {
        match (
            flags.contains(CellFlags::BOLD),
            flags.contains(CellFlags::ITALIC),
        ) {
            (true, true) => Self::BoldItalic,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (false, false) => Self::Regular,
        }
    }
}


