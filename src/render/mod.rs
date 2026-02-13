//! Font loading, fallback chain, glyph cache, and text rendering for grid and UI.

mod font_discovery;
mod font_loading;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cell::CellFlags;

pub const FONT_SIZE: f32 = 16.0;
const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 32.0;

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

fn parse_font(data: &[u8]) -> Option<fontdue::Font> {
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
}

/// Font set with lazy variant loading, fallback chain, and glyph cache.
pub struct FontSet {
    /// Loaded font objects. Index 0 (Regular) is always `Some`.
    /// Bold/Italic/BoldItalic are loaded lazily on first use.
    fonts: [Option<fontdue::Font>; 4],
    /// True if a real (non-Regular-fallback) font exists for this variant.
    /// Set during font discovery based on path availability.
    has_variant: [bool; 4],
    /// File paths for deferred loading of font variants.
    font_paths: [Option<PathBuf>; 4],
    /// Loaded fallback font objects (populated lazily).
    fallback_fonts: Vec<fontdue::Font>,
    /// File paths for deferred loading of fallback fonts.
    fallback_paths: Vec<PathBuf>,
    /// Whether fallback fonts have been loaded from `fallback_paths`.
    fallbacks_loaded: bool,
    pub size: f32,
    pub cell_width: usize,
    pub cell_height: usize,
    pub baseline: usize,
    cache: HashMap<(char, FontStyle), (fontdue::Metrics, Vec<u8>)>,
}

impl FontSet {
    /// Build a `FontSet` from a loaded Regular font, computing metrics and
    /// initializing empty cache/fallbacks.
    fn new_from_regular(
        regular: fontdue::Font,
        size: f32,
        font_paths: [Option<PathBuf>; 4],
        has_variant: [bool; 4],
        fallback_paths: Vec<PathBuf>,
    ) -> Self {
        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);
        Self {
            fonts: [Some(regular), None, None, None],
            has_variant,
            font_paths,
            fallback_fonts: Vec::new(),
            fallback_paths,
            fallbacks_loaded: false,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        }
    }

    /// Rebuild the font set at a new size, preserving the same font files.
    #[must_use]
    pub fn resize(&self, new_size: f32) -> Self {
        let new_size = new_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let regular = self.fonts[0]
            .as_ref()
            .expect("Regular font must always be loaded");
        let (cell_width, cell_height, baseline) = Self::compute_metrics(regular, new_size);

        Self {
            fonts: self.fonts.clone(),
            has_variant: self.has_variant,
            font_paths: self.font_paths.clone(),
            fallback_fonts: self.fallback_fonts.clone(),
            fallback_paths: self.fallback_paths.clone(),
            fallbacks_loaded: self.fallbacks_loaded,
            size: new_size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        }
    }

    /// Get the advance width of a single character in pixels.
    pub fn char_advance(&mut self, ch: char) -> f32 {
        self.ensure(ch, FontStyle::Regular);
        self.get(ch, FontStyle::Regular)
            .map_or(self.cell_width as f32, |(m, _)| m.advance_width.ceil())
    }

    /// Get the total advance width of a string in pixels.
    pub fn text_advance(&mut self, text: &str) -> f32 {
        text.chars().map(|ch| self.char_advance(ch)).sum()
    }

    /// Truncate a string to fit within `max_width` pixels, appending an
    /// ellipsis only if the full text overflows. Uses per-glyph advance
    /// widths for accuracy with proportional fonts.
    pub fn truncate_to_pixel_width<'t>(&mut self, text: &'t str, max_width: f32) -> Cow<'t, str> {
        // Check if the full text fits — no truncation needed.
        if self.text_advance(text) <= max_width {
            return Cow::Borrowed(text);
        }

        // Need to truncate: find the cut point leaving room for ellipsis.
        let ellipsis = '\u{2026}';
        let ellipsis_w = self.char_advance(ellipsis);
        let target = (max_width - ellipsis_w).max(0.0);
        let mut width = 0.0f32;
        let mut result = String::new();

        for ch in text.chars() {
            let cw = self.char_advance(ch);
            if width + cw > target {
                result.push(ellipsis);
                return Cow::Owned(result);
            }
            width += cw;
            result.push(ch);
        }
        Cow::Owned(result)
    }

    fn compute_metrics(font: &fontdue::Font, size: f32) -> (usize, usize, usize) {
        let lm = font.horizontal_line_metrics(size).expect("no line metrics");
        let cell_height = (lm.ascent - lm.descent).ceil() as usize;
        let baseline = lm.ascent.ceil() as usize;
        let (m, _) = font.rasterize('M', size);
        let cell_width = m.advance_width.ceil() as usize;
        (cell_width, cell_height, baseline)
    }

    /// Lazily load a font variant from its stored path.
    fn ensure_font_loaded(&mut self, idx: usize) {
        if self.fonts[idx].is_some() {
            return;
        }
        if let Some(path) = self.font_paths[idx].as_ref() {
            if let Ok(data) = std::fs::read(path) {
                if let Some(font) = parse_font(&data) {
                    self.fonts[idx] = Some(font);
                    return;
                }
            }
            // Loading failed — mark variant unavailable
            crate::log(&format!(
                "font: failed to load variant {} from {:?}",
                idx, self.font_paths[idx]
            ));
            self.has_variant[idx] = false;
            self.font_paths[idx] = None;
        }
    }

    /// Lazily load fallback fonts from their stored paths.
    fn ensure_fallbacks_loaded(&mut self) {
        if self.fallbacks_loaded {
            return;
        }
        self.fallbacks_loaded = true;
        for path in &self.fallback_paths {
            if let Ok(data) = std::fs::read(path) {
                if let Some(font) = parse_font(&data) {
                    self.fallback_fonts.push(font);
                }
            }
        }
    }

    /// Rasterize a glyph with the fallback chain, lazy-loading fonts as needed.
    fn rasterize_with_fallback(
        &mut self,
        ch: char,
        style: FontStyle,
    ) -> (fontdue::Metrics, Vec<u8>) {
        let idx = style as usize;

        // 1. Try requested style font (lazy-load if needed)
        self.ensure_font_loaded(idx);
        if let Some(ref font) = self.fonts[idx] {
            if font.has_glyph(ch) {
                return font.rasterize(ch, self.size);
            }
        }

        // 2. Try Regular font (style fallback — always loaded)
        if style != FontStyle::Regular {
            if let Some(ref font) = self.fonts[0] {
                if font.has_glyph(ch) {
                    return font.rasterize(ch, self.size);
                }
            }
        }

        // 3. Try fallback fonts (lazy-load if needed)
        self.ensure_fallbacks_loaded();
        for fb in &self.fallback_fonts {
            if fb.has_glyph(ch) {
                return fb.rasterize(ch, self.size);
            }
        }

        // 4. Replacement character
        if let Some(ref font) = self.fonts[0] {
            if font.has_glyph('\u{FFFD}') {
                return font.rasterize('\u{FFFD}', self.size);
            }
        }

        // 5. Last resort: return empty glyph
        (fontdue::Metrics::default(), Vec::new())
    }

    /// Ensure a glyph is cached for the given style.
    pub fn ensure(&mut self, ch: char, style: FontStyle) {
        let key = (ch, style);
        if !self.cache.contains_key(&key) {
            let result = self.rasterize_with_fallback(ch, style);
            self.cache.insert(key, result);
        }
    }

    /// Get a cached glyph.
    pub fn get(&self, ch: char, style: FontStyle) -> Option<&(fontdue::Metrics, Vec<u8>)> {
        self.cache.get(&(ch, style))
    }

    /// Whether bold needs synthetic rendering (no real bold font available).
    pub fn needs_synthetic_bold(&self) -> bool {
        !self.has_variant[FontStyle::Bold as usize]
    }
}
