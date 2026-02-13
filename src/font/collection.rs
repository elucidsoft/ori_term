//! Font data collection — owns raw bytes, fontdue rasterizers, and creates rustybuzz faces.

#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::render::font_discovery::FontFamily;
#[cfg(target_os = "windows")]
use crate::render::font_discovery::{
    resolve_fallback_paths_dwrite, resolve_family_paths_dwrite, DWRITE_FAMILIES,
};
#[cfg(not(target_os = "windows"))]
use crate::render::font_discovery::{
    build_font_index, find_font_variant_path, resolve_fallback_paths,
};
use crate::render::font_discovery::FONT_FAMILIES;
use crate::render::{FontStyle, MAX_FONT_SIZE, MIN_FONT_SIZE, parse_font};

use super::FaceIdx;

/// Per-face data: raw bytes + fontdue rasterizer.
struct FaceData {
    /// Raw font file bytes (kept alive for rustybuzz `Face` borrowing).
    bytes: Arc<Vec<u8>>,
    /// fontdue font for rasterization (owns its parsed data).
    raster: fontdue::Font,
    /// Index within .ttc collection file (0 for single-font files).
    face_index: u32,
}

/// Font collection for grid rendering: stores font data and provides rasterization.
///
/// Primary faces (4 style variants) and fallback fonts are loaded from disk.
/// Raw bytes are kept in `Arc<Vec<u8>>` so rustybuzz faces can borrow them
/// transiently during frame building.
pub struct FontCollection {
    /// Primary font faces (Regular/Bold/Italic/BoldItalic).
    /// Regular (index 0) is always `Some`.
    primary: [Option<FaceData>; 4],
    /// Whether a real (non-Regular) font exists for each style.
    has_variant: [bool; 4],
    /// Fallback font faces (in priority order).
    fallbacks: Vec<FaceData>,
    /// File paths for deferred loading of font variants.
    font_paths: [Option<PathBuf>; 4],
    /// File paths for deferred loading of fallback fonts.
    fallback_paths: Vec<PathBuf>,
    /// Whether fallback fonts have been loaded.
    fallbacks_loaded: bool,
    /// OpenType features to apply during shaping.
    pub features: Vec<rustybuzz::Feature>,
    /// Font size in points.
    pub size: f32,
    /// Cell width in pixels (from primary Regular font).
    pub cell_width: usize,
    /// Cell height in pixels (from primary Regular font).
    pub cell_height: usize,
    /// Baseline offset from top of cell.
    pub baseline: usize,
}

impl FontCollection {
    /// Build a `FontCollection` from a loaded Regular face and its paths.
    fn new_from_regular(
        bytes: Vec<u8>,
        raster: fontdue::Font,
        face_index: u32,
        size: f32,
        font_paths: [Option<PathBuf>; 4],
        has_variant: [bool; 4],
        fallback_paths: Vec<PathBuf>,
        features: Vec<rustybuzz::Feature>,
    ) -> Self {
        let (cell_width, cell_height, baseline) = compute_metrics(&raster, size);
        Self {
            primary: [
                Some(FaceData {
                    bytes: Arc::new(bytes),
                    raster,
                    face_index,
                }),
                None,
                None,
                None,
            ],
            has_variant,
            fallbacks: Vec::new(),
            font_paths,
            fallback_paths,
            fallbacks_loaded: false,
            features,
            size,
            cell_width,
            cell_height,
            baseline,
        }
    }

    /// Parse feature strings into rustybuzz features.
    ///
    /// Each string is a 4-char OpenType tag, optionally prefixed with `-` to
    /// disable. Examples: `"calt"` (enable), `"-dlig"` (disable).
    pub fn parse_features(strings: &[String]) -> Vec<rustybuzz::Feature> {
        strings
            .iter()
            .filter_map(|s| {
                let (tag_str, value) = if let Some(rest) = s.strip_prefix('-') {
                    (rest, 0)
                } else {
                    (s.as_str(), 1)
                };
                let bytes = tag_str.as_bytes();
                if bytes.len() != 4 {
                    crate::log(&format!("font_collection: ignoring invalid feature tag: {s}"));
                    return None;
                }
                let tag = rustybuzz::ttf_parser::Tag::from_bytes(
                    bytes.try_into().expect("checked length"),
                );
                Some(rustybuzz::Feature::new(tag, value, ..))
            })
            .collect()
    }

    /// Load a font collection at the given size, trying font families in priority order.
    pub fn load(size: f32, family: Option<&str>, features: &[rustybuzz::Feature]) -> Self {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        #[cfg(not(target_os = "windows"))]
        let font_index = build_font_index();

        if let Some(name) = family {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_by_name(name, size, &font_index, features);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_by_name(name, size, features);

            if let Some(fc) = result {
                return fc;
            }
        }

        #[cfg(target_os = "windows")]
        for name in DWRITE_FAMILIES {
            if let Some(fc) = Self::try_load_family_dwrite(name, size, features) {
                return fc;
            }
        }

        for fam in FONT_FAMILIES {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_family(fam, size, &font_index, features);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_family(fam, size, features);

            if let Some(fc) = result {
                return fc;
            }
        }
        panic!("no suitable monospace font found");
    }

    /// Rebuild the font collection at a new size, preserving the same font files.
    #[must_use]
    pub fn resize(&self, new_size: f32) -> Self {
        let new_size = new_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        // Re-load from paths at the new size to get correct metrics.
        // Font data is re-parsed to update fontdue's internal metrics cache.
        let regular_path = self.font_paths[0]
            .as_ref()
            .expect("Regular font path must exist");
        let bytes = std::fs::read(regular_path).expect("failed to re-read Regular font");
        let raster = parse_font(&bytes).expect("failed to re-parse Regular font");
        let face_index = self.primary[0]
            .as_ref()
            .map_or(0, |fd| fd.face_index);

        Self::new_from_regular(
            bytes,
            raster,
            face_index,
            new_size,
            self.font_paths.clone(),
            self.has_variant,
            self.fallback_paths.clone(),
            self.features.clone(),
        )
    }

    /// Whether bold needs synthetic rendering (no real bold font available).
    pub fn needs_synthetic_bold(&self) -> bool {
        !self.has_variant[FontStyle::Bold as usize]
    }

    /// Get the advance width of a single character in pixels (for width measurement).
    pub fn char_advance(&self, ch: char) -> f32 {
        if let Some(ref fd) = self.primary[0] {
            let (m, _) = fd.raster.rasterize(ch, self.size);
            m.advance_width.ceil()
        } else {
            self.cell_width as f32
        }
    }

    /// Eagerly load all font variants and fallbacks.
    ///
    /// Call before `create_shaping_faces()` + `find_face_loaded()` to ensure
    /// all faces are available without `&mut self`.
    pub fn ensure_all_loaded(&mut self) {
        for idx in 0..4 {
            self.ensure_primary_loaded(idx);
        }
        self.ensure_fallbacks_loaded();
    }

    /// Find which face covers a given character for the given style.
    pub fn find_face_for_char(&mut self, ch: char, style: FontStyle) -> FaceIdx {
        let idx = style as usize;

        // 1. Try requested style
        self.ensure_primary_loaded(idx);
        if let Some(ref fd) = self.primary[idx] {
            if fd.raster.lookup_glyph_index(ch) != 0 {
                return FaceIdx(idx as u16);
            }
        }

        // 2. Try Regular (style fallback)
        if style != FontStyle::Regular {
            if let Some(ref fd) = self.primary[0] {
                if fd.raster.lookup_glyph_index(ch) != 0 {
                    return FaceIdx(0);
                }
            }
        }

        // 3. Try fallback fonts
        self.ensure_fallbacks_loaded();
        for (i, fb) in self.fallbacks.iter().enumerate() {
            if fb.raster.lookup_glyph_index(ch) != 0 {
                return FaceIdx((4 + i) as u16);
            }
        }

        // 4. Return primary Regular (.notdef)
        FaceIdx(0)
    }

    /// Find which face covers a character (immutable — requires prior `ensure_all_loaded()`).
    ///
    /// Same logic as `find_face_for_char` but takes `&self`. Only valid after
    /// all font variants and fallbacks have been loaded.
    pub fn find_face_loaded(&self, ch: char, style: FontStyle) -> FaceIdx {
        let idx = style as usize;

        if let Some(ref fd) = self.primary[idx] {
            if fd.raster.lookup_glyph_index(ch) != 0 {
                return FaceIdx(idx as u16);
            }
        }

        if style != FontStyle::Regular {
            if let Some(ref fd) = self.primary[0] {
                if fd.raster.lookup_glyph_index(ch) != 0 {
                    return FaceIdx(0);
                }
            }
        }

        for (i, fb) in self.fallbacks.iter().enumerate() {
            if fb.raster.lookup_glyph_index(ch) != 0 {
                return FaceIdx((4 + i) as u16);
            }
        }

        FaceIdx(0)
    }

    /// Rasterize a glyph by its glyph ID from the specified face.
    pub fn rasterize_glyph(
        &self,
        face_idx: FaceIdx,
        glyph_id: u16,
    ) -> Option<(fontdue::Metrics, Vec<u8>)> {
        let fd = self.face(face_idx)?;
        let (m, bmp) = fd.raster.rasterize_indexed(glyph_id, self.size);
        Some((m, bmp))
    }

    /// Create transient rustybuzz faces that borrow from stored bytes.
    ///
    /// Returns a vec parallel to the face indices: primary[0..4] then fallbacks.
    /// Faces that aren't loaded yet are `None`.
    pub fn create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face<'_>>> {
        let total = 4 + self.fallbacks.len();
        let mut faces = Vec::with_capacity(total);

        for slot in &self.primary {
            faces.push(slot.as_ref().and_then(|fd| {
                rustybuzz::Face::from_slice(&fd.bytes, fd.face_index)
            }));
        }
        for fd in &self.fallbacks {
            faces.push(rustybuzz::Face::from_slice(&fd.bytes, fd.face_index));
        }

        faces
    }

    /// Returns the units-per-em for the given face (needed to scale shaper output).
    pub fn units_per_em(&self, face_idx: FaceIdx) -> u16 {
        self.face(face_idx)
            .and_then(|fd| {
                rustybuzz::Face::from_slice(&fd.bytes, fd.face_index)
                    .map(|f| f.units_per_em() as u16)
            })
            .unwrap_or(1000)
    }

    /// Access a face by index (0–3 = primary, 4+ = fallback).
    fn face(&self, idx: FaceIdx) -> Option<&FaceData> {
        let i = idx.0 as usize;
        if i < 4 {
            self.primary[i].as_ref()
        } else {
            self.fallbacks.get(i - 4)
        }
    }

    /// Lazily load a primary font variant from its stored path.
    fn ensure_primary_loaded(&mut self, idx: usize) {
        if self.primary[idx].is_some() {
            return;
        }
        if let Some(path) = self.font_paths[idx].as_ref() {
            if let Ok(data) = std::fs::read(path) {
                if let Some(raster) = parse_font(&data) {
                    self.primary[idx] = Some(FaceData {
                        bytes: Arc::new(data),
                        raster,
                        face_index: 0,
                    });
                    return;
                }
            }
            crate::log(&format!(
                "font_collection: failed to load variant {idx} from {}",
                path.display()
            ));
            self.has_variant[idx] = false;
            self.font_paths[idx] = None;
        }
    }

    /// Lazily load all fallback fonts from their stored paths.
    fn ensure_fallbacks_loaded(&mut self) {
        if self.fallbacks_loaded {
            return;
        }
        self.fallbacks_loaded = true;
        for path in &self.fallback_paths {
            if let Ok(data) = std::fs::read(path) {
                let face_index = detect_face_index(path);
                let settings = fontdue::FontSettings {
                    collection_index: face_index,
                    ..fontdue::FontSettings::default()
                };
                if let Ok(raster) = fontdue::Font::from_bytes(data.as_slice(), settings) {
                    self.fallbacks.push(FaceData {
                        bytes: Arc::new(data),
                        raster,
                        face_index,
                    });
                }
            }
        }
    }

    // Platform-specific font loading methods

    /// Try to load a font family via DirectWrite by family name.
    #[cfg(target_os = "windows")]
    fn try_load_family_dwrite(
        family_name: &str,
        size: f32,
        features: &[rustybuzz::Feature],
    ) -> Option<Self> {
        let paths = resolve_family_paths_dwrite(family_name)?;
        let regular_path = paths[0].as_ref()?;
        let bytes = std::fs::read(regular_path).ok()?;
        let raster = parse_font(&bytes)?;

        let has_variant = [
            true,
            paths[1].is_some(),
            paths[2].is_some(),
            paths[3].is_some(),
        ];
        let fallback_paths = resolve_fallback_paths_dwrite();

        crate::log(&format!(
            "font_collection: loaded {family_name:?} via DirectWrite ({} fallbacks)",
            fallback_paths.len()
        ));

        Some(Self::new_from_regular(
            bytes,
            raster,
            0,
            size,
            paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
        ))
    }

    /// Try to load a font family from hardcoded file paths (Windows fallback).
    #[cfg(target_os = "windows")]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        features: &[rustybuzz::Feature],
    ) -> Option<Self> {
        let regular_path = family
            .regular
            .iter()
            .map(|p| PathBuf::from(*p))
            .find(|p| p.exists())?;
        let bytes = std::fs::read(&regular_path).ok()?;
        let raster = parse_font(&bytes)?;

        let mut font_paths: [Option<PathBuf>; 4] = [Some(regular_path), None, None, None];
        let mut has_variant = [true, false, false, false];

        for (idx, candidates) in [
            (1usize, family.bold),
            (2, family.italic),
            (3, family.bold_italic),
        ] {
            for path_str in candidates {
                let path = PathBuf::from(*path_str);
                if path.exists() {
                    font_paths[idx] = Some(path);
                    has_variant[idx] = true;
                    break;
                }
            }
        }

        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            bytes,
            raster,
            0,
            size,
            font_paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
        ))
    }

    /// Try to load a font by user-specified name or path (Windows).
    #[cfg(target_os = "windows")]
    fn try_load_by_name(
        name: &str,
        size: f32,
        features: &[rustybuzz::Feature],
    ) -> Option<Self> {
        if let Some(fc) = Self::try_load_family_dwrite(name, size, features) {
            return Some(fc);
        }

        let path = if std::path::Path::new(name).is_absolute() {
            PathBuf::from(name)
        } else {
            PathBuf::from(r"C:\Windows\Fonts").join(name)
        };
        let bytes = std::fs::read(&path).ok()?;
        let raster = parse_font(&bytes)?;
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            bytes,
            raster,
            0,
            size,
            [Some(path), None, None, None],
            [true, false, false, false],
            fallback_paths,
            features.to_vec(),
        ))
    }

    /// Try to load a font family from the pre-built font index (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
        features: &[rustybuzz::Feature],
    ) -> Option<Self> {
        let regular_path = find_font_variant_path(family.regular, font_index)?;
        let bytes = std::fs::read(&regular_path).ok()?;
        let raster = parse_font(&bytes)?;

        let mut font_paths: [Option<PathBuf>; 4] = [Some(regular_path), None, None, None];
        let mut has_variant = [true, false, false, false];

        for (idx, names) in [
            (1usize, family.bold),
            (2, family.italic),
            (3, family.bold_italic),
        ] {
            if let Some(path) = find_font_variant_path(names, font_index) {
                font_paths[idx] = Some(path);
                has_variant[idx] = true;
            }
        }

        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self::new_from_regular(
            bytes,
            raster,
            0,
            size,
            font_paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
        ))
    }

    /// Try to load a font by user-specified name or path (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_by_name(
        name: &str,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
        features: &[rustybuzz::Feature],
    ) -> Option<Self> {
        let (bytes, regular_path) = if std::path::Path::new(name).is_absolute() {
            let path = PathBuf::from(name);
            let data = std::fs::read(&path).ok()?;
            (data, path)
        } else {
            let path = font_index.get(name)?.clone();
            let data = std::fs::read(&path).ok()?;
            (data, path)
        };
        let raster = parse_font(&bytes)?;
        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self::new_from_regular(
            bytes,
            raster,
            0,
            size,
            [Some(regular_path), None, None, None],
            [true, false, false, false],
            fallback_paths,
            features.to_vec(),
        ))
    }
}

/// Compute cell metrics from a fontdue font at the given size.
fn compute_metrics(font: &fontdue::Font, size: f32) -> (usize, usize, usize) {
    let lm = font
        .horizontal_line_metrics(size)
        .expect("no line metrics");
    let cell_height = (lm.ascent - lm.descent).ceil() as usize;
    let baseline = lm.ascent.ceil() as usize;
    let (m, _) = font.rasterize('M', size);
    let cell_width = m.advance_width.ceil() as usize;
    (cell_width, cell_height, baseline)
}

/// Detect the face index for a font file (always 0 — uses first face in collections).
fn detect_face_index(_path: &std::path::Path) -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::FONT_SIZE;

    #[test]
    fn default_features() {
        let features = vec![
            rustybuzz::Feature::new(
                rustybuzz::ttf_parser::Tag::from_bytes(b"calt"),
                1,
                ..,
            ),
            rustybuzz::Feature::new(
                rustybuzz::ttf_parser::Tag::from_bytes(b"liga"),
                1,
                ..,
            ),
        ];
        let fc = FontCollection::load(FONT_SIZE, None, &features);
        assert!(fc.cell_width > 0);
        assert!(fc.cell_height > 0);
        assert!(fc.baseline > 0);
    }

    #[test]
    fn find_face_for_ascii() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[]);
        let idx = fc.find_face_for_char('A', FontStyle::Regular);
        assert_eq!(idx, FaceIdx(0), "ASCII 'A' should be in primary Regular");
    }

    #[test]
    fn rasterize_glyph_by_id() {
        let fc = FontCollection::load(FONT_SIZE, None, &[]);
        // Lookup glyph ID for 'A' in primary Regular
        let glyph_id = fc.primary[0]
            .as_ref()
            .map(|fd| fd.raster.lookup_glyph_index('A'))
            .expect("Regular font must be loaded");
        assert_ne!(glyph_id, 0, "'A' must have a non-zero glyph ID");

        let result = fc.rasterize_glyph(FaceIdx(0), glyph_id);
        assert!(result.is_some(), "rasterize_glyph should succeed for 'A'");
        let (metrics, bitmap) = result.unwrap();
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
        assert!(!bitmap.is_empty());
    }

    #[test]
    fn create_shaping_faces_has_regular() {
        let fc = FontCollection::load(FONT_SIZE, None, &[]);
        let faces = fc.create_shaping_faces();
        assert!(faces[0].is_some(), "Regular face must be created");
    }

    #[test]
    fn resize_preserves_features() {
        let features = vec![
            rustybuzz::Feature::new(
                rustybuzz::ttf_parser::Tag::from_bytes(b"calt"),
                1,
                ..,
            ),
        ];
        let fc = FontCollection::load(FONT_SIZE, None, &features);
        let fc2 = fc.resize(20.0);
        assert_eq!(fc2.features.len(), 1);
        assert!(fc2.cell_width > 0);
    }
}
