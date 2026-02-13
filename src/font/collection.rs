//! Font data collection — owns raw bytes, swash cache keys, and creates rustybuzz faces.

use std::borrow::Cow;
#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use swash::scale::{Render, ScaleContext, Source};
use swash::{CacheKey, FontRef};
use swash::zeno::Format;

use crate::config::FallbackFontConfig;
use crate::gpu::atlas::GlyphBitmap;
use crate::render::font_discovery::FontFamily;
#[cfg(target_os = "windows")]
use crate::render::font_discovery::{
    resolve_fallback_paths_dwrite, resolve_family_paths_dwrite, resolve_user_fallback,
    DWRITE_FAMILIES,
};
#[cfg(not(target_os = "windows"))]
use crate::render::font_discovery::{
    build_font_index, find_font_variant_path, resolve_fallback_paths, resolve_user_fallback,
};
use crate::render::font_discovery::FONT_FAMILIES;
use crate::render::{FontStyle, MAX_FONT_SIZE, MIN_FONT_SIZE};

use super::FaceIdx;

/// Per-fallback metadata: features, cap-height scale, and size offset.
struct FallbackMeta {
    /// OpenType features specific to this fallback font.
    features: Vec<rustybuzz::Feature>,
    /// Cap-height normalization: `primary_cap_height / fallback_cap_height`.
    scale_factor: f32,
    /// User-configured size offset in points (0.0 if unset).
    size_offset: f32,
}

/// Per-face data: raw bytes + swash identifiers for transient `FontRef` creation.
struct FaceData {
    /// Raw font file bytes (kept alive for rustybuzz `Face` and swash `FontRef` borrowing).
    bytes: Arc<Vec<u8>>,
    /// Index within .ttc collection file (0 for single-font files).
    face_index: u32,
    /// Byte offset to the font table directory (from `FontRef::from_index`).
    offset: u32,
    /// Unique cache key for `ScaleContext` reuse across frames.
    cache_key: CacheKey,
}

/// Create a transient swash `FontRef` from stored face data.
fn font_ref(fd: &FaceData) -> FontRef<'_> {
    FontRef {
        data: &fd.bytes,
        offset: fd.offset,
        key: fd.cache_key,
    }
}

/// Check whether a face covers a given character.
fn has_glyph(fd: &FaceData, ch: char) -> bool {
    font_ref(fd).charmap().map(ch) != 0
}

/// Validate font bytes and extract swash metadata.
///
/// Returns `(offset, cache_key)` on success.
fn validate_font(data: &[u8], face_index: u32) -> Option<(u32, CacheKey)> {
    let fr = FontRef::from_index(data, face_index as usize)?;
    Some((fr.offset, fr.key))
}

/// Font collection for grid rendering: stores font data and provides rasterization.
///
/// Primary faces (4 style variants) and fallback fonts are loaded from disk.
/// Raw bytes are kept in `Arc<Vec<u8>>` so rustybuzz faces and swash `FontRef`s
/// can borrow them transiently during frame building.
pub struct FontCollection {
    /// Primary font faces (Regular/Bold/Italic/BoldItalic).
    /// Regular (index 0) is always `Some`.
    primary: [Option<FaceData>; 4],
    /// Whether a real (non-Regular) font exists for each style.
    has_variant: [bool; 4],
    /// Fallback font faces (in priority order).
    fallbacks: Vec<FaceData>,
    /// Per-fallback metadata (parallel to `fallbacks`).
    fallback_meta: Vec<FallbackMeta>,
    /// File paths for deferred loading of font variants.
    font_paths: [Option<PathBuf>; 4],
    /// File paths for deferred loading of fallback fonts.
    fallback_paths: Vec<PathBuf>,
    /// User-configured fallback font entries (resolved at load time).
    user_fallback_config: Vec<FallbackFontConfig>,
    /// Whether fallback fonts have been loaded.
    fallbacks_loaded: bool,
    /// OpenType features to apply during shaping.
    pub features: Vec<rustybuzz::Feature>,
    /// Font size in points.
    pub size: f32,
    /// Cap height of the primary Regular font in pixels.
    primary_cap_height_px: f32,
    /// Cell width in pixels (from primary Regular font).
    pub cell_width: usize,
    /// Cell height in pixels (from primary Regular font).
    pub cell_height: usize,
    /// Baseline offset from top of cell.
    pub baseline: usize,
    /// CSS-style font weight (100–900) used during font discovery.
    weight: u16,
    /// Reusable scale context for swash rasterization.
    scale_context: ScaleContext,
}

impl FontCollection {
    /// Build a `FontCollection` from validated Regular face data and its paths.
    fn new_from_regular(
        bytes: Vec<u8>,
        face_index: u32,
        offset: u32,
        cache_key: CacheKey,
        size: f32,
        font_paths: [Option<PathBuf>; 4],
        has_variant: [bool; 4],
        fallback_paths: Vec<PathBuf>,
        features: Vec<rustybuzz::Feature>,
        user_fallback_config: Vec<FallbackFontConfig>,
        weight: u16,
    ) -> Self {
        let (cell_width, cell_height, baseline) = compute_metrics(&bytes, face_index, size);
        let arc_bytes = Arc::new(bytes);
        let primary_cap_height_px = cap_height_px(&arc_bytes, face_index, size);
        Self {
            primary: [
                Some(FaceData {
                    bytes: arc_bytes,
                    face_index,
                    offset,
                    cache_key,
                }),
                None,
                None,
                None,
            ],
            has_variant,
            fallbacks: Vec::new(),
            fallback_meta: Vec::new(),
            font_paths,
            fallback_paths,
            user_fallback_config,
            fallbacks_loaded: false,
            features,
            size,
            primary_cap_height_px,
            cell_width,
            cell_height,
            baseline,
            weight,
            scale_context: ScaleContext::new(),
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
    ///
    /// `weight` is the CSS-style font weight (100–900) for the Regular slot.
    pub fn load(
        size: f32,
        family: Option<&str>,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
    ) -> Self {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let fc_vec = fallback_config.to_vec();

        #[cfg(not(target_os = "windows"))]
        let font_index = build_font_index();

        if let Some(name) = family {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_by_name(name, size, &font_index, features, &fc_vec, weight);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_by_name(name, size, features, &fc_vec, weight);

            if let Some(fc) = result {
                return fc;
            }
        }

        #[cfg(target_os = "windows")]
        for name in DWRITE_FAMILIES {
            if let Some(fc) = Self::try_load_family_dwrite(name, size, features, &fc_vec, weight) {
                return fc;
            }
        }

        for fam in FONT_FAMILIES {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_family(fam, size, &font_index, features, &fc_vec, weight);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_family(fam, size, features, &fc_vec, weight);

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
        let regular_path = self.font_paths[0]
            .as_ref()
            .expect("Regular font path must exist");
        let bytes = std::fs::read(regular_path).expect("failed to re-read Regular font");
        let face_index = self.primary[0]
            .as_ref()
            .map_or(0, |fd| fd.face_index);
        let (offset, cache_key) =
            validate_font(&bytes, face_index).expect("failed to re-validate Regular font");

        Self::new_from_regular(
            bytes,
            face_index,
            offset,
            cache_key,
            new_size,
            self.font_paths.clone(),
            self.has_variant,
            self.fallback_paths.clone(),
            self.features.clone(),
            self.user_fallback_config.clone(),
            self.weight,
        )
    }

    /// Whether bold needs synthetic rendering (no real bold font available).
    pub fn needs_synthetic_bold(&self) -> bool {
        !self.has_variant[FontStyle::Bold as usize]
    }

    /// Get the advance width of a single character in pixels (for width measurement).
    pub fn char_advance(&self, ch: char) -> f32 {
        if let Some(ref fd) = self.primary[0] {
            let fr = font_ref(fd);
            let gid = fr.charmap().map(ch);
            fr.glyph_metrics(&[]).scale(self.size).advance_width(gid).ceil()
        } else {
            self.cell_width as f32
        }
    }

    /// Get the total advance width of a string in pixels.
    pub fn text_advance(&self, text: &str) -> f32 {
        text.chars().map(|ch| self.char_advance(ch)).sum()
    }

    /// Truncate a string to fit within `max_width` pixels, appending an
    /// ellipsis only if the full text overflows. Uses per-glyph advance
    /// widths for accuracy with proportional fonts.
    pub fn truncate_to_pixel_width<'t>(&self, text: &'t str, max_width: f32) -> Cow<'t, str> {
        if self.text_advance(text) <= max_width {
            return Cow::Borrowed(text);
        }

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

    /// Returns true if the primary Regular font has a `wght` variation axis.
    pub fn has_wght_axis(&self) -> bool {
        self.primary[0]
            .as_ref()
            .is_some_and(|fd| font_ref(fd).variations().any(|v| v.tag() == swash::tag_from_bytes(b"wght")))
    }

    /// Returns the OpenType features to use for the given face.
    ///
    /// Primary faces use the collection-wide features. Fallback faces use
    /// their per-font override (if configured), otherwise the primary features.
    pub fn features_for_face(&self, face_idx: FaceIdx) -> &[rustybuzz::Feature] {
        if face_idx.is_fallback() {
            let fb_i = face_idx.0 as usize - 4;
            if let Some(meta) = self.fallback_meta.get(fb_i) {
                if !meta.features.is_empty() {
                    return &meta.features;
                }
            }
        }
        &self.features
    }

    /// Returns the effective font size for the given face.
    ///
    /// Primary faces use the base size. Fallback faces are scaled by their
    /// cap-height normalization factor and adjusted by any user `size_offset`.
    pub fn effective_size(&self, face_idx: FaceIdx) -> f32 {
        effective_size_for(face_idx, self.size, &self.fallback_meta)
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
            if has_glyph(fd, ch) {
                return FaceIdx(idx as u16);
            }
        }

        // 2. Try Regular (style fallback)
        if style != FontStyle::Regular {
            if let Some(ref fd) = self.primary[0] {
                if has_glyph(fd, ch) {
                    return FaceIdx(0);
                }
            }
        }

        // 3. Try fallback fonts
        self.ensure_fallbacks_loaded();
        for (i, fb) in self.fallbacks.iter().enumerate() {
            if has_glyph(fb, ch) {
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
            if has_glyph(fd, ch) {
                return FaceIdx(idx as u16);
            }
        }

        if style != FontStyle::Regular {
            if let Some(ref fd) = self.primary[0] {
                if has_glyph(fd, ch) {
                    return FaceIdx(0);
                }
            }
        }

        for (i, fb) in self.fallbacks.iter().enumerate() {
            if has_glyph(fb, ch) {
                return FaceIdx((4 + i) as u16);
            }
        }

        FaceIdx(0)
    }

    /// Rasterize a glyph by its glyph ID from the specified face.
    pub fn rasterize_glyph(
        &mut self,
        face_idx: FaceIdx,
        glyph_id: u16,
    ) -> Option<GlyphBitmap> {
        // Access face data via direct field indexing (not self.face()) to allow
        // disjoint borrow of self.scale_context.
        let i = face_idx.0 as usize;
        let fd = if i < 4 {
            self.primary[i].as_ref()?
        } else {
            self.fallbacks.get(i - 4)?
        };
        let size = effective_size_for(face_idx, self.size, &self.fallback_meta);
        let wght = weight_variation_for(face_idx, self.weight);
        rasterize_from_face(fd, glyph_id, size, wght, &mut self.scale_context)
    }

    /// Rasterize a glyph using an external `ScaleContext`.
    ///
    /// Takes `&self` so it can be called while rustybuzz faces borrow the
    /// collection's font bytes. The caller provides a mutable `ScaleContext`
    /// (typically owned by the renderer).
    pub fn rasterize_glyph_with(
        &self,
        face_idx: FaceIdx,
        glyph_id: u16,
        scale_ctx: &mut ScaleContext,
    ) -> Option<GlyphBitmap> {
        let fd = self.face(face_idx)?;
        let size = self.effective_size(face_idx);
        let wght = weight_variation_for(face_idx, self.weight);
        rasterize_from_face(fd, glyph_id, size, wght, scale_ctx)
    }

    /// Create transient rustybuzz faces that borrow from stored bytes.
    ///
    /// Returns a vec parallel to the face indices: primary[0..4] then fallbacks.
    /// Faces that aren't loaded yet are `None`. Primary faces have the `wght`
    /// variation axis set according to the configured weight.
    pub fn create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face<'_>>> {
        let total = 4 + self.fallbacks.len();
        let mut faces = Vec::with_capacity(total);

        for (i, slot) in self.primary.iter().enumerate() {
            faces.push(slot.as_ref().and_then(|fd| {
                let mut face = rustybuzz::Face::from_slice(&fd.bytes, fd.face_index)?;
                if let Some(w) = weight_variation_for(FaceIdx(i as u16), self.weight) {
                    face.set_variations(&[rustybuzz::Variation {
                        tag: rustybuzz::ttf_parser::Tag::from_bytes(b"wght"),
                        value: w,
                    }]);
                }
                Some(face)
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
                if let Some((offset, cache_key)) = validate_font(&data, 0) {
                    self.primary[idx] = Some(FaceData {
                        bytes: Arc::new(data),
                        face_index: 0,
                        offset,
                        cache_key,
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
    ///
    /// User-configured fallbacks are loaded first (with per-font features and
    /// cap-height normalization), followed by system-discovered fallbacks.
    fn ensure_fallbacks_loaded(&mut self) {
        if self.fallbacks_loaded {
            return;
        }
        self.fallbacks_loaded = true;

        // Phase 1: user-configured fallbacks (from config)
        let user_configs: Vec<FallbackFontConfig> = self.user_fallback_config.clone();
        for cfg in &user_configs {
            let path = {
                #[cfg(target_os = "windows")]
                { resolve_user_fallback(&cfg.family) }
                #[cfg(not(target_os = "windows"))]
                {
                    let idx = build_font_index();
                    resolve_user_fallback(&cfg.family, &idx)
                }
            };
            let Some(path) = path else {
                crate::log(&format!(
                    "font_collection: user fallback {:?} not found, skipping",
                    cfg.family
                ));
                continue;
            };
            if let Some((fd, meta)) = self.load_fallback_with_meta(&path, cfg) {
                self.fallbacks.push(fd);
                self.fallback_meta.push(meta);
            }
        }

        // Phase 2: system-discovered fallbacks
        let system_paths: Vec<PathBuf> = self.fallback_paths.clone();
        for path in &system_paths {
            if let Some((fd, meta)) = self.load_system_fallback(path) {
                self.fallbacks.push(fd);
                self.fallback_meta.push(meta);
            }
        }
    }

    /// Load a single user-configured fallback font and compute its metadata.
    fn load_fallback_with_meta(
        &self,
        path: &std::path::Path,
        cfg: &FallbackFontConfig,
    ) -> Option<(FaceData, FallbackMeta)> {
        let data = std::fs::read(path).ok()?;
        let face_index = detect_face_index(path);
        let (offset, cache_key) = validate_font(&data, face_index)?;
        let arc = Arc::new(data);

        let fb_cap = cap_height_px(&arc, face_index, self.size);
        let scale_factor = if fb_cap > 0.0 && self.primary_cap_height_px > 0.0 {
            self.primary_cap_height_px / fb_cap
        } else {
            1.0
        };

        let features = if let Some(ref feat_strings) = cfg.features {
            Self::parse_features(feat_strings)
        } else {
            Vec::new()
        };
        let size_offset = cfg.size_offset.unwrap_or(0.0);

        Some((
            FaceData { bytes: arc, face_index, offset, cache_key },
            FallbackMeta { features, scale_factor, size_offset },
        ))
    }

    /// Load a single system-discovered fallback font with auto cap-height normalization.
    fn load_system_fallback(
        &self,
        path: &std::path::Path,
    ) -> Option<(FaceData, FallbackMeta)> {
        let data = std::fs::read(path).ok()?;
        let face_index = detect_face_index(path);
        let (offset, cache_key) = validate_font(&data, face_index)?;
        let arc = Arc::new(data);

        let fb_cap = cap_height_px(&arc, face_index, self.size);
        let scale_factor = if fb_cap > 0.0 && self.primary_cap_height_px > 0.0 {
            self.primary_cap_height_px / fb_cap
        } else {
            1.0
        };

        Some((
            FaceData { bytes: arc, face_index, offset, cache_key },
            FallbackMeta { features: Vec::new(), scale_factor, size_offset: 0.0 },
        ))
    }

    // Platform-specific font loading methods

    /// Try to load a font family via DirectWrite by family name.
    #[cfg(target_os = "windows")]
    fn try_load_family_dwrite(
        family_name: &str,
        size: f32,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
    ) -> Option<Self> {
        let paths = resolve_family_paths_dwrite(family_name, weight)?;
        let regular_path = paths[0].as_ref()?;
        let bytes = std::fs::read(regular_path).ok()?;
        let (offset, cache_key) = validate_font(&bytes, 0)?;

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
            0,
            offset,
            cache_key,
            size,
            paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
            fallback_config.to_vec(),
            weight,
        ))
    }

    /// Try to load a font family from hardcoded file paths (Windows fallback).
    #[cfg(target_os = "windows")]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
    ) -> Option<Self> {
        let regular_path = family
            .regular
            .iter()
            .map(|p| PathBuf::from(*p))
            .find(|p| p.exists())?;
        let bytes = std::fs::read(&regular_path).ok()?;
        let (offset, cache_key) = validate_font(&bytes, 0)?;

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
            0,
            offset,
            cache_key,
            size,
            font_paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
            fallback_config.to_vec(),
            weight,
        ))
    }

    /// Try to load a font by user-specified name or path (Windows).
    #[cfg(target_os = "windows")]
    fn try_load_by_name(
        name: &str,
        size: f32,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
    ) -> Option<Self> {
        if let Some(fc) = Self::try_load_family_dwrite(name, size, features, fallback_config, weight) {
            return Some(fc);
        }

        let path = if std::path::Path::new(name).is_absolute() {
            PathBuf::from(name)
        } else {
            PathBuf::from(r"C:\Windows\Fonts").join(name)
        };
        let bytes = std::fs::read(&path).ok()?;
        let (offset, cache_key) = validate_font(&bytes, 0)?;
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            bytes,
            0,
            offset,
            cache_key,
            size,
            [Some(path), None, None, None],
            [true, false, false, false],
            fallback_paths,
            features.to_vec(),
            fallback_config.to_vec(),
            weight,
        ))
    }

    /// Try to load a font family from the pre-built font index (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
    ) -> Option<Self> {
        let regular_path = find_font_variant_path(family.regular, font_index)?;
        let bytes = std::fs::read(&regular_path).ok()?;
        let (offset, cache_key) = validate_font(&bytes, 0)?;

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
            0,
            offset,
            cache_key,
            size,
            font_paths,
            has_variant,
            fallback_paths,
            features.to_vec(),
            fallback_config.to_vec(),
            weight,
        ))
    }

    /// Try to load a font by user-specified name or path (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_by_name(
        name: &str,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
        features: &[rustybuzz::Feature],
        fallback_config: &[FallbackFontConfig],
        weight: u16,
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
        let (offset, cache_key) = validate_font(&bytes, 0)?;
        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self::new_from_regular(
            bytes,
            0,
            offset,
            cache_key,
            size,
            [Some(regular_path), None, None, None],
            [true, false, false, false],
            fallback_paths,
            features.to_vec(),
            fallback_config.to_vec(),
            weight,
        ))
    }
}

/// Compute the `wght` variation value for a face index.
///
/// Primary faces use the configured weight (Regular/Italic) or bold-derived
/// weight (Bold/BoldItalic). Fallback faces return `None` (default weight).
fn weight_variation_for(face_idx: FaceIdx, weight: u16) -> Option<f32> {
    let i = face_idx.0 as usize;
    if i < 4 {
        let w = if i == 1 || i == 3 {
            // Bold / BoldItalic
            (weight + 300).min(900)
        } else {
            // Regular / Italic
            weight
        };
        Some(w as f32)
    } else {
        None
    }
}

/// Compute effective font size for a face index.
fn effective_size_for(face_idx: FaceIdx, base_size: f32, fallback_meta: &[FallbackMeta]) -> f32 {
    if face_idx.is_fallback() {
        let fb_i = face_idx.0 as usize - 4;
        if let Some(meta) = fallback_meta.get(fb_i) {
            return (base_size * meta.scale_factor + meta.size_offset)
                .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        }
    }
    base_size
}

/// Rasterize a glyph from face data using the given scale context.
///
/// `wght` sets the weight variation axis for variable fonts. Pass `None`
/// for fallback faces or non-variable fonts (uses font default).
fn rasterize_from_face(
    fd: &FaceData,
    glyph_id: u16,
    size: f32,
    wght: Option<f32>,
    scale_ctx: &mut ScaleContext,
) -> Option<GlyphBitmap> {
    let fr = font_ref(fd);
    let advance_width = fr.glyph_metrics(&[]).scale(size).advance_width(glyph_id);

    let builder = scale_ctx
        .builder(fr)
        .size(size)
        .hint(true);
    let mut scaler = if let Some(w) = wght {
        builder.variations(&[("wght", w)]).build()
    } else {
        builder.build()
    };
    let image = Render::new(&[Source::Outline])
        .format(Format::Alpha)
        .render(&mut scaler, glyph_id)?;

    Some(GlyphBitmap {
        width: image.placement.width as usize,
        height: image.placement.height as usize,
        left: image.placement.left,
        top: image.placement.top,
        advance_width,
        data: image.data,
    })
}

/// Compute cell metrics from font bytes at the given size via swash.
fn compute_metrics(bytes: &[u8], face_index: u32, size: f32) -> (usize, usize, usize) {
    let fr = FontRef::from_index(bytes, face_index as usize).expect("valid font");
    let metrics = fr.metrics(&[]).scale(size);
    let cell_height = (metrics.ascent + metrics.descent.abs()).ceil() as usize;
    let baseline = metrics.ascent.ceil() as usize;
    let gid = fr.charmap().map('M');
    let cell_width = fr.glyph_metrics(&[]).scale(size).advance_width(gid).ceil() as usize;
    (cell_width, cell_height, baseline)
}

/// Compute the cap height in pixels for a font at the given size.
///
/// Reads `capital_height` from the OS/2 table via `ttf-parser`. Falls back to
/// `0.75 * ascender` when the metric is missing.
fn cap_height_px(bytes: &[u8], face_index: u32, size: f32) -> f32 {
    let Some(face) = rustybuzz::Face::from_slice(bytes, face_index) else {
        return 0.0;
    };
    let upem = face.units_per_em() as f32;
    if upem == 0.0 {
        return 0.0;
    }
    let cap_units = face
        .tables()
        .os2
        .and_then(|os2| {
            let h = os2.capital_height()?;
            Some(h as f32)
        })
        .unwrap_or_else(|| face.ascender() as f32 * 0.75);
    cap_units / upem * size
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
        let fc = FontCollection::load(FONT_SIZE, None, &features, &[], 400);
        assert!(fc.cell_width > 0);
        assert!(fc.cell_height > 0);
        assert!(fc.baseline > 0);
    }

    #[test]
    fn find_face_for_ascii() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let idx = fc.find_face_for_char('A', FontStyle::Regular);
        assert_eq!(idx, FaceIdx(0), "ASCII 'A' should be in primary Regular");
    }

    #[test]
    fn rasterize_glyph_by_id() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        // Lookup glyph ID for 'A' in primary Regular via swash charmap
        let glyph_id = fc.primary[0]
            .as_ref()
            .map(|fd| font_ref(fd).charmap().map('A'))
            .expect("Regular font must be loaded");
        assert_ne!(glyph_id, 0, "'A' must have a non-zero glyph ID");

        let result = fc.rasterize_glyph(FaceIdx(0), glyph_id);
        assert!(result.is_some(), "rasterize_glyph should succeed for 'A'");
        let bitmap = result.expect("checked above");
        assert!(bitmap.width > 0);
        assert!(bitmap.height > 0);
        assert!(!bitmap.data.is_empty());
    }

    #[test]
    fn create_shaping_faces_has_regular() {
        let fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
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
        let fc = FontCollection::load(FONT_SIZE, None, &features, &[], 400);
        let fc2 = fc.resize(20.0);
        assert_eq!(fc2.features.len(), 1);
        assert!(fc2.cell_width > 0);
    }

    #[test]
    fn primary_face_uses_collection_features() {
        let features = vec![rustybuzz::Feature::new(
            rustybuzz::ttf_parser::Tag::from_bytes(b"calt"),
            1,
            ..,
        )];
        let fc = FontCollection::load(FONT_SIZE, None, &features, &[], 400);
        let got = fc.features_for_face(FaceIdx(0));
        assert_eq!(got.len(), 1, "primary face should use collection features");
    }

    #[test]
    fn primary_effective_size_equals_base() {
        let fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let size = fc.effective_size(FaceIdx(0));
        assert!(
            (size - fc.size).abs() < f32::EPSILON,
            "primary face effective_size should equal base size"
        );
    }

    #[test]
    fn fallback_without_meta_uses_base_features() {
        let features = vec![rustybuzz::Feature::new(
            rustybuzz::ttf_parser::Tag::from_bytes(b"liga"),
            1,
            ..,
        )];
        let fc = FontCollection::load(FONT_SIZE, None, &features, &[], 400);
        // FaceIdx(10) is an out-of-range fallback — should fall back to primary features
        let got = fc.features_for_face(FaceIdx(10));
        assert_eq!(
            got.len(),
            features.len(),
            "out-of-range fallback should use primary features"
        );
    }

    #[test]
    fn cap_height_px_returns_positive() {
        let fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        assert!(
            fc.primary_cap_height_px > 0.0,
            "primary cap height should be positive"
        );
    }

    #[test]
    fn resize_preserves_fallback_config() {
        let fallback = vec![FallbackFontConfig {
            family: "NonExistent-Test-Font".into(),
            features: Some(vec!["-dlig".into()]),
            size_offset: Some(-1.0),
        }];
        let fc = FontCollection::load(FONT_SIZE, None, &[], &fallback, 400);
        let fc2 = fc.resize(20.0);
        assert_eq!(
            fc2.user_fallback_config.len(),
            1,
            "resize should preserve user_fallback_config"
        );
        assert_eq!(fc2.user_fallback_config[0].family, "NonExistent-Test-Font");
    }

    #[test]
    fn hinted_rasterization_produces_nonzero_bitmap() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let gid = fc.primary[0]
            .as_ref()
            .map(|fd| font_ref(fd).charmap().map('H'))
            .expect("Regular font must be loaded");
        let bitmap = fc.rasterize_glyph(FaceIdx(0), gid)
            .expect("'H' must rasterize");
        assert!(bitmap.width > 0 && bitmap.height > 0);
        assert!(bitmap.data.iter().any(|&b| b > 0), "bitmap should have non-zero pixels");
        assert!(bitmap.top > 0, "top bearing should be positive for uppercase");
    }

    #[test]
    fn weight_variation_changes_rasterized_output() {
        // Find a variable font with wght axis on this system
        let variable_paths = [
            "/mnt/c/WINDOWS/FONTS/CASCADIAMONO.TTF",
            "/usr/share/fonts/truetype/ubuntu/UbuntuSansMono[wght].ttf",
            "/usr/share/fonts/truetype/ubuntu/UbuntuMono[wght].ttf",
        ];
        let font_bytes = variable_paths.iter()
            .find_map(|p| std::fs::read(p).ok());
        let Some(bytes) = font_bytes else {
            eprintln!("no variable font with wght axis found — skipping test");
            return;
        };
        let fr = FontRef::from_index(&bytes, 0).expect("valid font");
        let has_wght = fr.variations().any(|v| v.tag() == swash::tag_from_bytes(b"wght"));
        assert!(has_wght, "test font must have wght axis");

        // Print axis details
        for v in fr.variations() {
            let tag_bytes = v.tag().to_be_bytes();
            let tag_str = std::str::from_utf8(&tag_bytes).unwrap_or("????");
            eprintln!("  axis {tag_str}: min={} default={} max={}",
                v.min_value(), v.default_value(), v.max_value());
        }

        let gid = fr.charmap().map('H');
        assert_ne!(gid, 0, "'H' must have a non-zero glyph ID");

        let mut ctx = ScaleContext::new();

        // Test at multiple sizes (12pt UI, 16pt grid, 20pt HiDPI)
        for size in [12.0, 15.0, 16.0, 20.0] {
            // Rasterize at weight 400 (regular/default)
            let mut scaler_regular = ctx.builder(fr)
                .size(size)
                .hint(true)
                .variations(&[("wght", 400.0)])
                .build();
            let img_regular = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut scaler_regular, gid)
                .expect("rasterize at 400");

            // Rasterize at weight 500 (grid)
            let mut scaler_500 = ctx.builder(fr)
                .size(size)
                .hint(true)
                .variations(&[("wght", 500.0)])
                .build();
            let img_500 = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut scaler_500, gid)
                .expect("rasterize at 500");

            // Rasterize at weight 700 (max for Cascadia)
            let mut scaler_700 = ctx.builder(fr)
                .size(size)
                .hint(true)
                .variations(&[("wght", 700.0)])
                .build();
            let img_700 = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut scaler_700, gid)
                .expect("rasterize at 700");

            // Rasterize at weight 900 (above max — should clamp)
            let mut scaler_900 = ctx.builder(fr)
                .size(size)
                .hint(true)
                .variations(&[("wght", 900.0)])
                .build();
            let img_900 = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut scaler_900, gid)
                .expect("rasterize at 900");

            let sum = |img: &swash::scale::image::Image| -> u64 {
                img.data.iter().map(|&b| b as u64).sum()
            };
            let s400 = sum(&img_regular);
            let s500 = sum(&img_500);
            let s700 = sum(&img_700);
            let s900 = sum(&img_900);
            eprintln!("size={size:.0}: w400={s400} w500={s500} w700={s700} w900={s900}");
        }

        // Final assertion: at 16pt, weight 700 should have more ink than 400
        let mut scaler_light = ctx.builder(fr)
            .size(16.0)
            .hint(true)
            .variations(&[("wght", 400.0)])
            .build();
        let img_light = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler_light, gid)
            .expect("rasterize at 400");
        let mut scaler_heavy = ctx.builder(fr)
            .size(16.0)
            .hint(true)
            .variations(&[("wght", 700.0)])
            .build();
        let img_heavy = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler_heavy, gid)
            .expect("rasterize at 700");

        let sum_light: u64 = img_light.data.iter().map(|&b| b as u64).sum();
        let sum_heavy: u64 = img_heavy.data.iter().map(|&b| b as u64).sum();
        assert!(
            sum_heavy > sum_light,
            "weight 700 should have more ink than 400: heavy={sum_heavy} light={sum_light}"
        );
    }
}
