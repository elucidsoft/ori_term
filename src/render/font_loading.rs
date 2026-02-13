//! Font loading — platform-specific discovery, family resolution, lazy variant loading.

#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;

use super::font_discovery::FontFamily;
#[cfg(target_os = "windows")]
use super::font_discovery::{
    resolve_fallback_paths_dwrite, resolve_family_paths_dwrite, resolve_font_dwrite,
    DWRITE_FAMILIES, DWRITE_UI_FAMILIES,
};
#[cfg(not(target_os = "windows"))]
use super::font_discovery::{
    build_font_index, find_font_variant_path, resolve_fallback_paths, UI_FONT_NAMES,
};
use super::font_discovery::FONT_FAMILIES;
use super::{FontSet, MAX_FONT_SIZE, MIN_FONT_SIZE, parse_font};

impl FontSet {
    /// Load a font set at the given size, trying font families in priority order.
    /// If `family` is specified, attempt to load that font first before the
    /// platform auto-detect list.
    pub fn load(size: f32, family: Option<&str>) -> Self {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        // Build the font directory index once on Linux (avoids 20-36+ recursive walks)
        #[cfg(not(target_os = "windows"))]
        let font_index = build_font_index();

        if let Some(name) = family {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_by_name(name, size, &font_index);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_by_name(name, size);

            if let Some(fs) = result {
                return fs;
            }
            crate::log(&format!(
                "font: custom family {name:?} not found, using platform default"
            ));
        }

        // On Windows: try DirectWrite families first (instant OS-indexed lookup)
        #[cfg(target_os = "windows")]
        for name in DWRITE_FAMILIES {
            if let Some(fs) = Self::try_load_family_dwrite(name, size) {
                return fs;
            }
        }

        for fam in FONT_FAMILIES {
            #[cfg(not(target_os = "windows"))]
            let result = Self::try_load_family(fam, size, &font_index);
            #[cfg(target_os = "windows")]
            let result = Self::try_load_family(fam, size);

            if let Some(fs) = result {
                return fs;
            }
        }
        panic!("no suitable monospace font found");
    }

    /// Load the OS system UI font at the given size (semi-bold weight).
    ///
    /// On Windows, queries `SystemParametersInfo` for the message font name
    /// and resolves it via DirectWrite at semi-bold weight for crisp tab labels.
    /// Returns `None` if no UI font is found (caller should fall back to
    /// resizing the monospace font).
    #[cfg(target_os = "windows")]
    pub fn load_ui(size: f32) -> Option<Self> {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        // Try the user's configured system font first
        if let Some(name) = super::font_discovery::detect_system_ui_font_name() {
            if let Some(fs) = Self::try_load_ui_font_dwrite(&name, size) {
                crate::log(&format!(
                    "ui font: loaded system font {name:?} (SemiBold) via DirectWrite"
                ));
                return Some(fs);
            }
        }

        // Fall back to common UI font families
        for name in DWRITE_UI_FAMILIES {
            if let Some(fs) = Self::try_load_ui_font_dwrite(name, size) {
                crate::log(&format!(
                    "ui font: loaded {name:?} (SemiBold) via DirectWrite"
                ));
                return Some(fs);
            }
        }

        None
    }

    /// Load a single UI font at semi-bold weight via DirectWrite.
    #[cfg(target_os = "windows")]
    fn try_load_ui_font_dwrite(family_name: &str, size: f32) -> Option<Self> {
        // Try SemiBold first, fall back to Bold, then Regular
        let path = resolve_font_dwrite(
            family_name,
            dwrote::FontWeight::SemiBold,
            dwrote::FontStyle::Normal,
        )
        .or_else(|| {
            resolve_font_dwrite(
                family_name,
                dwrote::FontWeight::Bold,
                dwrote::FontStyle::Normal,
            )
        })
        .or_else(|| {
            resolve_font_dwrite(
                family_name,
                dwrote::FontWeight::Regular,
                dwrote::FontStyle::Normal,
            )
        })?;

        let data = std::fs::read(&path).ok()?;
        let font = parse_font(&data)?;
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            font,
            size,
            [Some(path), None, None, None],
            [true, false, false, false],
            fallback_paths,
        ))
    }

    /// Load the OS system UI font at the given size (Linux).
    #[cfg(not(target_os = "windows"))]
    pub fn load_ui(size: f32) -> Option<Self> {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let font_index = build_font_index();

        for name in UI_FONT_NAMES {
            if let Some(path) = font_index.get(*name) {
                if let Ok(data) = std::fs::read(path) {
                    if let Some(font) = parse_font(&data) {
                        let fallback_paths = resolve_fallback_paths(&font_index);
                        crate::log(&format!("ui font: loaded {name:?}"));
                        return Some(Self::new_from_regular(
                            font,
                            size,
                            [Some(path.clone()), None, None, None],
                            [true, false, false, false],
                            fallback_paths,
                        ));
                    }
                }
            }
        }

        None
    }

    /// Try to load a font family via DirectWrite by family name.
    /// Only loads the Regular variant eagerly; Bold/Italic/BoldItalic paths are
    /// stored for lazy loading on first use.
    #[cfg(target_os = "windows")]
    fn try_load_family_dwrite(family_name: &str, size: f32) -> Option<Self> {
        let paths = resolve_family_paths_dwrite(family_name)?;

        // Load Regular font immediately
        let regular_path = paths[0].as_ref()?;
        let regular_data = std::fs::read(regular_path).ok()?;
        let regular = parse_font(&regular_data)?;

        let has_variant = [
            true,
            paths[1].is_some(),
            paths[2].is_some(),
            paths[3].is_some(),
        ];

        let fallback_paths = resolve_fallback_paths_dwrite();

        crate::log(&format!(
            "font: loaded {:?} via DirectWrite ({} fallback paths)",
            family_name,
            fallback_paths.len()
        ));

        Some(Self::new_from_regular(
            regular,
            size,
            paths,
            has_variant,
            fallback_paths,
        ))
    }

    /// Try to load a font family from hardcoded file paths (Windows fallback).
    /// Only loads the Regular variant eagerly; other variants are lazy-loaded.
    #[cfg(target_os = "windows")]
    fn try_load_family(family: &FontFamily, size: f32) -> Option<Self> {
        // Find and load Regular font
        let regular_path = family
            .regular
            .iter()
            .map(|p| PathBuf::from(*p))
            .find(|p| p.exists())?;
        let regular_data = std::fs::read(&regular_path).ok()?;
        let regular = parse_font(&regular_data)?;

        // Resolve variant paths without loading
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

        // Resolve fallback paths (prefer DirectWrite, fall back to static paths)
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            regular,
            size,
            font_paths,
            has_variant,
            fallback_paths,
        ))
    }

    /// Try to load a font family from the pre-built font index (Linux).
    /// Only loads the Regular variant eagerly; other variants are lazy-loaded.
    #[cfg(not(target_os = "windows"))]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
    ) -> Option<Self> {
        // Find and load Regular font
        let regular_path = find_font_variant_path(family.regular, font_index)?;
        let regular_data = std::fs::read(&regular_path).ok()?;
        let regular = parse_font(&regular_data)?;

        // Resolve variant paths without loading
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

        // Resolve fallback paths without loading
        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self::new_from_regular(
            regular,
            size,
            font_paths,
            has_variant,
            fallback_paths,
        ))
    }

    /// Try to load a font by a user-specified name or path.
    /// On Windows, tries DirectWrite by family name first, then falls back to
    /// treating the name as a filename under `C:\Windows\Fonts\`.
    #[cfg(target_os = "windows")]
    fn try_load_by_name(name: &str, size: f32) -> Option<Self> {
        // First try DirectWrite by family name (supports "Fira Code", "Consolas", etc.)
        if let Some(fs) = Self::try_load_family_dwrite(name, size) {
            return Some(fs);
        }

        // Fall back to file path
        let path = if std::path::Path::new(name).is_absolute() {
            PathBuf::from(name)
        } else {
            PathBuf::from(r"C:\Windows\Fonts").join(name)
        };
        let regular_data = std::fs::read(&path).ok()?;
        let regular = parse_font(&regular_data)?;
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self::new_from_regular(
            regular,
            size,
            [Some(path), None, None, None],
            [true, false, false, false],
            fallback_paths,
        ))
    }

    /// Try to load a font by a user-specified name or path (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_by_name(
        name: &str,
        size: f32,
        font_index: &HashMap<String, PathBuf>,
    ) -> Option<Self> {
        let (regular_data, regular_path) = if std::path::Path::new(name).is_absolute() {
            let path = PathBuf::from(name);
            let data = std::fs::read(&path).ok()?;
            (data, path)
        } else {
            let path = font_index.get(name)?.clone();
            let data = std::fs::read(&path).ok()?;
            (data, path)
        };
        let regular = parse_font(&regular_data)?;
        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self::new_from_regular(
            regular,
            size,
            [Some(regular_path), None, None, None],
            [true, false, false, false],
            fallback_paths,
        ))
    }
}
