use std::collections::HashMap;

use unicode_width::UnicodeWidthChar;

use crate::cell::CellFlags;

pub const FONT_SIZE: f32 = 16.0;
const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 32.0;

// --- Font style types ---

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

// --- Font family definitions ---

struct FontFamily {
    regular: &'static [&'static str],
    bold: &'static [&'static str],
    italic: &'static [&'static str],
    bold_italic: &'static [&'static str],
}

#[cfg(target_os = "windows")]
const FONT_FAMILIES: &[FontFamily] = &[
    FontFamily {
        regular: &[r"C:\Windows\Fonts\JetBrainsMono-Regular.ttf"],
        bold: &[r"C:\Windows\Fonts\JetBrainsMono-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\JetBrainsMono-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\JetBrainsMono-BoldItalic.ttf"],
    },
    FontFamily {
        regular: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Regular.ttf"],
        bold: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-BoldItalic.ttf"],
    },
    FontFamily {
        regular: &[r"C:\Windows\Fonts\CascadiaMonoNF.ttf"],
        bold: &[r"C:\Windows\Fonts\CascadiaMonoNF-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\CascadiaMonoNF-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\CascadiaMonoNF-BoldItalic.ttf"],
    },
    FontFamily {
        regular: &[r"C:\Windows\Fonts\CascadiaMono.ttf"],
        bold: &[r"C:\Windows\Fonts\CascadiaMono-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\CascadiaMono-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\CascadiaMono-BoldItalic.ttf"],
    },
    FontFamily {
        regular: &[r"C:\Windows\Fonts\consola.ttf"],
        bold: &[r"C:\Windows\Fonts\consolab.ttf"],
        italic: &[r"C:\Windows\Fonts\consolai.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\consolaz.ttf"],
    },
    FontFamily {
        regular: &[r"C:\Windows\Fonts\cour.ttf"],
        bold: &[r"C:\Windows\Fonts\courbd.ttf"],
        italic: &[r"C:\Windows\Fonts\couri.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\courbi.ttf"],
    },
];

#[cfg(not(target_os = "windows"))]
const FONT_FAMILIES: &[FontFamily] = &[
    FontFamily {
        regular: &[
            "JetBrainsMono-Regular.ttf",
            "JetBrainsMonoNerdFont-Regular.ttf",
        ],
        bold: &["JetBrainsMono-Bold.ttf", "JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &[
            "JetBrainsMono-Italic.ttf",
            "JetBrainsMonoNerdFont-Italic.ttf",
        ],
        bold_italic: &[
            "JetBrainsMono-BoldItalic.ttf",
            "JetBrainsMonoNerdFont-BoldItalic.ttf",
        ],
    },
    FontFamily {
        regular: &["UbuntuMono-Regular.ttf", "UbuntuMonoNerdFont-Regular.ttf"],
        bold: &["UbuntuMono-Bold.ttf", "UbuntuMonoNerdFont-Bold.ttf"],
        italic: &["UbuntuMono-Italic.ttf", "UbuntuMonoNerdFont-Italic.ttf"],
        bold_italic: &[
            "UbuntuMono-BoldItalic.ttf",
            "UbuntuMonoNerdFont-BoldItalic.ttf",
        ],
    },
    FontFamily {
        regular: &["DejaVuSansMono.ttf"],
        bold: &["DejaVuSansMono-Bold.ttf"],
        italic: &["DejaVuSansMono-Oblique.ttf"],
        bold_italic: &["DejaVuSansMono-BoldOblique.ttf"],
    },
    FontFamily {
        regular: &["LiberationMono-Regular.ttf"],
        bold: &["LiberationMono-Bold.ttf"],
        italic: &["LiberationMono-Italic.ttf"],
        bold_italic: &["LiberationMono-BoldItalic.ttf"],
    },
];

/// Fallback fonts for missing glyphs (symbols, CJK, etc.).
#[cfg(target_os = "windows")]
const FALLBACK_FONT_PATHS: &[&str] = &[
    r"C:\Windows\Fonts\seguisym.ttf", // Segoe UI Symbol
    r"C:\Windows\Fonts\msgothic.ttc", // MS Gothic (CJK)
    r"C:\Windows\Fonts\segoeui.ttf",  // Segoe UI
];

#[cfg(not(target_os = "windows"))]
const FALLBACK_FONT_NAMES: &[&str] = &[
    "NotoSansMono-Regular.ttf",
    "NotoSansSymbols2-Regular.ttf",
    "NotoSansCJK-Regular.ttc",
    "DejaVuSans.ttf",
];

// --- DirectWrite font resolution (Windows) ---

/// Font family names to try via DirectWrite, in priority order.
#[cfg(target_os = "windows")]
const DWRITE_FAMILIES: &[&str] = &[
    "JetBrains Mono",
    "JetBrainsMono Nerd Font",
    "Cascadia Mono NF",
    "Cascadia Mono",
    "Consolas",
    "Courier New",
];

/// Fallback font family names for DirectWrite resolution.
#[cfg(target_os = "windows")]
const DWRITE_FALLBACK_FAMILIES: &[&str] = &["Segoe UI Symbol", "MS Gothic", "Segoe UI"];

/// UI font family names to try on Windows (proportional).
#[cfg(target_os = "windows")]
const DWRITE_UI_FAMILIES: &[&str] = &["Segoe UI", "Tahoma", "Arial"];

/// Detect the OS system UI font family name.
///
/// On Windows, queries `SystemParametersInfo(SPI_GETNONCLIENTMETRICS)` to get
/// the message font (used for dialogs and general UI text).
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn detect_system_ui_font_name() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SystemParametersInfoW,
    };

    // SAFETY: We pass a properly sized and zeroed NONCLIENTMETRICSW buffer to
    // SystemParametersInfoW, which is a standard Win32 API call that fills the
    // struct with system font metrics.
    let mut metrics = unsafe { std::mem::zeroed::<NONCLIENTMETRICSW>() };
    metrics.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;

    let success = unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            (&raw mut metrics).cast::<std::ffi::c_void>(),
            0,
        )
    };

    if success == 0 {
        return None;
    }

    // lfMessageFont is the font used for message boxes and general dialog UI.
    let face_name = &metrics.lfMessageFont.lfFaceName;
    let len = face_name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(face_name.len());
    String::from_utf16(&face_name[..len]).ok()
}

/// UI font filenames to try on Linux (proportional, SemiBold preferred).
#[cfg(not(target_os = "windows"))]
const UI_FONT_NAMES: &[&str] = &[
    "Cantarell-Bold.otf",
    "Ubuntu-M.ttf",
    "NotoSans-SemiBold.ttf",
    "NotoSans-SemiBold.otf",
    "DejaVuSans-Bold.ttf",
    "LiberationSans-Bold.ttf",
    // Regular fallbacks
    "Cantarell-Regular.otf",
    "Ubuntu-R.ttf",
    "NotoSans-Regular.ttf",
    "NotoSans-Regular.otf",
    "DejaVuSans.ttf",
    "LiberationSans-Regular.ttf",
];

/// Resolve a single font variant via DirectWrite by family name + weight + style.
#[cfg(target_os = "windows")]
fn resolve_font_dwrite(
    family_name: &str,
    weight: dwrote::FontWeight,
    style: dwrote::FontStyle,
) -> Option<std::path::PathBuf> {
    let collection = dwrote::FontCollection::system();
    let descriptor = dwrote::FontDescriptor {
        family_name: family_name.to_string(),
        weight,
        stretch: dwrote::FontStretch::Normal,
        style,
    };
    let font = collection
        .font_from_descriptor(&descriptor)
        .ok()
        .flatten()?;
    let face = font.create_font_face();
    let files = face.files().ok()?;
    let file = files.first()?;
    file.font_file_path().ok()
}

/// Resolve all 4 variant paths (Regular/Bold/Italic/BoldItalic) for a font family
/// via DirectWrite. Returns `None` if the family doesn't exist (Regular not found).
/// Bold/Italic/BoldItalic paths are filtered: if DirectWrite returns the same file
/// as Regular (fuzzy fallback), the variant is treated as unavailable.
#[cfg(target_os = "windows")]
fn resolve_family_paths_dwrite(family_name: &str) -> Option<[Option<std::path::PathBuf>; 4]> {
    let regular = resolve_font_dwrite(
        family_name,
        dwrote::FontWeight::Regular,
        dwrote::FontStyle::Normal,
    )?;

    let bold = resolve_font_dwrite(
        family_name,
        dwrote::FontWeight::Bold,
        dwrote::FontStyle::Normal,
    )
    .filter(|p| *p != regular);

    let italic = resolve_font_dwrite(
        family_name,
        dwrote::FontWeight::Regular,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    let bold_italic = resolve_font_dwrite(
        family_name,
        dwrote::FontWeight::Bold,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    Some([Some(regular), bold, italic, bold_italic])
}

/// Resolve fallback font paths via DirectWrite, with static paths as additional fallback.
#[cfg(target_os = "windows")]
fn resolve_fallback_paths_dwrite() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for name in DWRITE_FALLBACK_FAMILIES {
        if let Some(path) =
            resolve_font_dwrite(name, dwrote::FontWeight::Regular, dwrote::FontStyle::Normal)
        {
            paths.push(path);
        }
    }
    // Also try static paths not already found via DirectWrite
    for p in FALLBACK_FONT_PATHS {
        let path = std::path::PathBuf::from(*p);
        if path.exists() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

// --- Font discovery helpers ---

#[cfg(not(target_os = "windows"))]
fn linux_font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(std::path::PathBuf::from(home).join(".local/share/fonts"));
    }
    dirs.push(std::path::PathBuf::from("/usr/share/fonts"));
    dirs.push(std::path::PathBuf::from("/usr/local/share/fonts"));
    dirs
}

/// Build a filename → full path index by scanning all font directories once.
#[cfg(not(target_os = "windows"))]
fn build_font_index() -> HashMap<String, std::path::PathBuf> {
    let mut index = HashMap::new();
    for dir in linux_font_dirs() {
        index_font_dir(&dir, &mut index);
    }
    index
}

#[cfg(not(target_os = "windows"))]
fn index_font_dir(dir: &std::path::Path, index: &mut HashMap<String, std::path::PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            index_font_dir(&path, index);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            index.entry(name.to_owned()).or_insert(path);
        } else {
            // Non-UTF-8 filename — skip
        }
    }
}

/// Look up a font variant path by trying multiple candidate filenames.
#[cfg(not(target_os = "windows"))]
fn find_font_variant_path(
    names: &[&str],
    index: &HashMap<String, std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    for name in names {
        if let Some(path) = index.get(*name) {
            return Some(path.clone());
        }
    }
    None
}

/// Resolve fallback font paths from the pre-built index.
#[cfg(not(target_os = "windows"))]
fn resolve_fallback_paths(
    font_index: &HashMap<String, std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    FALLBACK_FONT_NAMES
        .iter()
        .filter_map(|name| font_index.get(*name).cloned())
        .collect()
}

fn parse_font(data: &[u8]) -> Option<fontdue::Font> {
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
}

// --- FontSet ---

pub struct FontSet {
    /// Loaded font objects. Index 0 (Regular) is always `Some`.
    /// Bold/Italic/BoldItalic are loaded lazily on first use.
    fonts: [Option<fontdue::Font>; 4],
    /// True if a real (non-Regular-fallback) font exists for this variant.
    /// Set during font discovery based on path availability.
    has_variant: [bool; 4],
    /// File paths for deferred loading of font variants.
    font_paths: [Option<std::path::PathBuf>; 4],
    /// Loaded fallback font objects (populated lazily).
    fallback_fonts: Vec<fontdue::Font>,
    /// File paths for deferred loading of fallback fonts.
    fallback_paths: Vec<std::path::PathBuf>,
    /// Whether fallback fonts have been loaded from `fallback_paths`.
    fallbacks_loaded: bool,
    pub size: f32,
    pub cell_width: usize,
    pub cell_height: usize,
    pub baseline: usize,
    cache: HashMap<(char, FontStyle), (fontdue::Metrics, Vec<u8>)>,
}

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

    /// Rebuild the font set at a new size, preserving the same font files.
    #[must_use]
    pub fn resize(&self, new_size: f32) -> Self {
        let new_size = new_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        let fonts = self.fonts.clone();
        let has_variant = self.has_variant;
        let font_paths = self.font_paths.clone();
        let fallback_fonts = self.fallback_fonts.clone();
        let fallback_paths = self.fallback_paths.clone();
        let fallbacks_loaded = self.fallbacks_loaded;

        let regular = fonts[0]
            .as_ref()
            .expect("Regular font must always be loaded");
        let (cell_width, cell_height, baseline) = Self::compute_metrics(regular, new_size);

        Self {
            fonts,
            has_variant,
            font_paths,
            fallback_fonts,
            fallback_paths,
            fallbacks_loaded,
            size: new_size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        }
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
        if let Some(name) = detect_system_ui_font_name() {
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
        let (cell_width, cell_height, baseline) = Self::compute_metrics(&font, size);
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self {
            fonts: [Some(font), None, None, None],
            has_variant: [true, false, false, false],
            font_paths: [Some(path), None, None, None],
            fallback_fonts: Vec::new(),
            fallback_paths,
            fallbacks_loaded: false,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        })
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
                        let (cell_width, cell_height, baseline) =
                            Self::compute_metrics(&font, size);
                        let fallback_paths = resolve_fallback_paths(&font_index);
                        crate::log(&format!("ui font: loaded {name:?}"));
                        return Some(Self {
                            fonts: [Some(font), None, None, None],
                            has_variant: [true, false, false, false],
                            font_paths: [Some(path.clone()), None, None, None],
                            fallback_fonts: Vec::new(),
                            fallback_paths,
                            fallbacks_loaded: false,
                            size,
                            cell_width,
                            cell_height,
                            baseline,
                            cache: HashMap::new(),
                        });
                    }
                }
            }
        }

        None
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
    pub fn truncate_to_pixel_width(&mut self, text: &str, max_width: f32) -> String {
        // Check if the full text fits — no truncation needed
        if self.text_advance(text) <= max_width {
            return text.to_string();
        }

        // Need to truncate: find the cut point leaving room for ellipsis
        let ellipsis = '\u{2026}';
        let ellipsis_w = self.char_advance(ellipsis);
        let target = (max_width - ellipsis_w).max(0.0);
        let mut width = 0.0f32;
        let mut result = String::new();

        for ch in text.chars() {
            let cw = self.char_advance(ch);
            if width + cw > target {
                result.push(ellipsis);
                return result;
            }
            width += cw;
            result.push(ch);
        }
        result
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

        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);

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

        Some(Self {
            fonts: [Some(regular), None, None, None],
            has_variant,
            font_paths: paths,
            fallback_fonts: Vec::new(),
            fallback_paths,
            fallbacks_loaded: false,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        })
    }

    /// Try to load a font family from hardcoded file paths (Windows fallback).
    /// Only loads the Regular variant eagerly; other variants are lazy-loaded.
    #[cfg(target_os = "windows")]
    fn try_load_family(family: &FontFamily, size: f32) -> Option<Self> {
        // Find and load Regular font
        let regular_path = family
            .regular
            .iter()
            .map(|p| std::path::PathBuf::from(*p))
            .find(|p| p.exists())?;
        let regular_data = std::fs::read(&regular_path).ok()?;
        let regular = parse_font(&regular_data)?;

        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);

        // Resolve variant paths without loading
        let mut font_paths: [Option<std::path::PathBuf>; 4] =
            [Some(regular_path), None, None, None];
        let mut has_variant = [true, false, false, false];

        for (idx, candidates) in [
            (1usize, family.bold),
            (2, family.italic),
            (3, family.bold_italic),
        ] {
            for path_str in candidates {
                let path = std::path::PathBuf::from(*path_str);
                if path.exists() {
                    font_paths[idx] = Some(path);
                    has_variant[idx] = true;
                    break;
                }
            }
        }

        // Resolve fallback paths (prefer DirectWrite, fall back to static paths)
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self {
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
        })
    }

    /// Try to load a font family from the pre-built font index (Linux).
    /// Only loads the Regular variant eagerly; other variants are lazy-loaded.
    #[cfg(not(target_os = "windows"))]
    fn try_load_family(
        family: &FontFamily,
        size: f32,
        font_index: &HashMap<String, std::path::PathBuf>,
    ) -> Option<Self> {
        // Find and load Regular font
        let regular_path = find_font_variant_path(family.regular, font_index)?;
        let regular_data = std::fs::read(&regular_path).ok()?;
        let regular = parse_font(&regular_data)?;

        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);

        // Resolve variant paths without loading
        let mut font_paths: [Option<std::path::PathBuf>; 4] =
            [Some(regular_path), None, None, None];
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

        Some(Self {
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
        })
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
            std::path::PathBuf::from(name)
        } else {
            std::path::PathBuf::from(r"C:\Windows\Fonts").join(name)
        };
        let regular_data = std::fs::read(&path).ok()?;
        let regular = parse_font(&regular_data)?;
        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);
        let fallback_paths = resolve_fallback_paths_dwrite();

        Some(Self {
            fonts: [Some(regular), None, None, None],
            has_variant: [true, false, false, false],
            font_paths: [Some(path), None, None, None],
            fallback_fonts: Vec::new(),
            fallback_paths,
            fallbacks_loaded: false,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        })
    }

    /// Try to load a font by a user-specified name or path (Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_load_by_name(
        name: &str,
        size: f32,
        font_index: &HashMap<String, std::path::PathBuf>,
    ) -> Option<Self> {
        let (regular_data, regular_path) = if std::path::Path::new(name).is_absolute() {
            let path = std::path::PathBuf::from(name);
            let data = std::fs::read(&path).ok()?;
            (data, path)
        } else {
            let path = font_index.get(name)?.clone();
            let data = std::fs::read(&path).ok()?;
            (data, path)
        };
        let regular = parse_font(&regular_data)?;
        let (cell_width, cell_height, baseline) = Self::compute_metrics(&regular, size);
        let fallback_paths = resolve_fallback_paths(font_index);

        Some(Self {
            fonts: [Some(regular), None, None, None],
            has_variant: [true, false, false, false],
            font_paths: [Some(regular_path), None, None, None],
            fallback_fonts: Vec::new(),
            fallback_paths,
            fallbacks_loaded: false,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        })
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

/// Alpha-blend a glyph pixel onto the buffer.
#[inline]
fn blend_pixel(
    buffer: &mut [u32],
    pidx: usize,
    alpha: u32,
    draw_r: u32,
    draw_g: u32,
    draw_b: u32,
    draw_u32: u32,
) {
    if alpha == 255 {
        buffer[pidx] = draw_u32;
    } else {
        let bg_val = buffer[pidx];
        let inv = 255 - alpha;
        let r = (draw_r * alpha + ((bg_val >> 16) & 0xFF) * inv) / 255;
        let g = (draw_g * alpha + ((bg_val >> 8) & 0xFF) * inv) / 255;
        let b = (draw_b * alpha + (bg_val & 0xFF) * inv) / 255;
        buffer[pidx] = (r << 16) | (g << 8) | b;
    }
}

/// Render a glyph bitmap at the given position with alpha blending.
/// If `synthetic_bold` is true, renders a second pass at gx+1 for double-strike.
#[allow(clippy::too_many_arguments)]
fn render_glyph(
    buffer: &mut [u32],
    buf_w: usize,
    buf_h: usize,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
    gx: i32,
    gy: i32,
    draw_r: u32,
    draw_g: u32,
    draw_b: u32,
    draw_u32: u32,
    synthetic_bold: bool,
) {
    for by in 0..metrics.height {
        for bx in 0..metrics.width {
            let alpha = bitmap[by * metrics.width + bx] as u32;
            if alpha == 0 {
                continue;
            }
            let px = gx + bx as i32;
            let py = gy + by as i32;
            if px < 0 || py < 0 || px as usize >= buf_w || py as usize >= buf_h {
                continue;
            }
            let pidx = py as usize * buf_w + px as usize;
            blend_pixel(buffer, pidx, alpha, draw_r, draw_g, draw_b, draw_u32);

            // Synthetic bold: draw again 1px to the right
            if synthetic_bold {
                let px2 = px + 1;
                if px2 >= 0 && (px2 as usize) < buf_w {
                    let pidx2 = py as usize * buf_w + px2 as usize;
                    blend_pixel(buffer, pidx2, alpha, draw_r, draw_g, draw_b, draw_u32);
                }
            }
        }
    }
}

/// Render a single text string into the buffer at pixel position (x, y).
/// Used for tab bar labels, etc. Always uses Regular style.
#[allow(clippy::many_single_char_names)]
#[allow(clippy::too_many_arguments)]
pub fn render_text(
    glyphs: &mut FontSet,
    text: &str,
    fg: u32,
    buffer: &mut [u32],
    buf_w: usize,
    buf_h: usize,
    x: usize,
    y: usize,
) {
    let baseline = glyphs.baseline;
    let cw = glyphs.cell_width;
    let mut cx = x;

    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;

    for ch in text.chars() {
        let char_cells = UnicodeWidthChar::width(ch).unwrap_or(1).max(1);
        let advance = cw * char_cells;
        if cx + advance > buf_w {
            break;
        }
        glyphs.ensure(ch, FontStyle::Regular);
        if let Some((metrics, bitmap)) = glyphs.get(ch, FontStyle::Regular) {
            let gx = cx as i32 + metrics.xmin;
            let gy = y as i32 + baseline as i32 - metrics.height as i32 - metrics.ymin;

            render_glyph(
                buffer, buf_w, buf_h, metrics, bitmap, gx, gy, fg_r, fg_g, fg_b, fg, false,
            );
        }
        cx += advance;
    }
}
