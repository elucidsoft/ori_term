//! Platform-specific font discovery — finding font files on disk.
//!
//! Handles DirectWrite resolution on Windows and directory scanning on Linux.
//! Pure discovery: no font loading, rasterizing, or caching.

#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;

// Font family definitions

pub(crate) struct FontFamily {
    pub regular: &'static [&'static str],
    pub bold: &'static [&'static str],
    pub italic: &'static [&'static str],
    pub bold_italic: &'static [&'static str],
}

#[cfg(target_os = "windows")]
pub(crate) const FONT_FAMILIES: &[FontFamily] = &[
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
pub(crate) const FONT_FAMILIES: &[FontFamily] = &[
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

// DirectWrite font resolution (Windows)

/// Font family names to try via DirectWrite, in priority order.
#[cfg(target_os = "windows")]
pub(crate) const DWRITE_FAMILIES: &[&str] = &[
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

/// Resolve a single font variant via DirectWrite by family name + weight + style.
///
/// Uses `GetFirstMatchingFont` for best-match selection rather than requiring
/// an exact weight match. DirectWrite picks the closest available weight.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_font_dwrite(
    family_name: &str,
    weight: dwrote::FontWeight,
    style: dwrote::FontStyle,
) -> Option<PathBuf> {
    let collection = dwrote::FontCollection::system();
    let family = collection.font_family_by_name(family_name).ok().flatten()?;
    let font = family
        .first_matching_font(weight, dwrote::FontStretch::Normal, style)
        .ok()?;
    let face = font.create_font_face();
    let files = face.files().ok()?;
    let file = files.first()?;
    file.font_file_path().ok()
}

/// Resolve all 4 variant paths (Regular/Bold/Italic/BoldItalic) for a font family
/// via DirectWrite. Returns `None` if the family doesn't exist (Regular not found).
/// Bold/Italic/BoldItalic paths are filtered: if DirectWrite returns the same file
/// as Regular (fuzzy fallback), the variant is treated as unavailable.
///
/// `weight` is the CSS-style weight (100–900) for the Regular slot. Bold is
/// derived as `min(900, weight + 300)` per the CSS "bolder" algorithm.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_family_paths_dwrite(
    family_name: &str,
    weight: u16,
) -> Option<[Option<PathBuf>; 4]> {
    let regular_weight = dwrote::FontWeight::from_u32(weight as u32);
    let bold_weight = dwrote::FontWeight::from_u32((weight + 300).min(900) as u32);

    let regular = resolve_font_dwrite(
        family_name,
        regular_weight,
        dwrote::FontStyle::Normal,
    )?;

    let bold = resolve_font_dwrite(
        family_name,
        bold_weight,
        dwrote::FontStyle::Normal,
    )
    .filter(|p| *p != regular);

    let italic = resolve_font_dwrite(
        family_name,
        regular_weight,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    let bold_italic = resolve_font_dwrite(
        family_name,
        bold_weight,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    let path_str = |p: &Option<PathBuf>| -> String {
        p.as_ref().map_or_else(|| "none".into(), |p| p.display().to_string())
    };
    crate::log(&format!(
        "font_discovery: {family_name:?} weight={weight} bold_weight={} \
         regular={} bold={} italic={} bold_italic={}",
        (weight + 300).min(900),
        regular.display(),
        path_str(&bold),
        path_str(&italic),
        path_str(&bold_italic),
    ));

    Some([Some(regular), bold, italic, bold_italic])
}

/// Resolve a user-configured fallback font family name to a file path (Windows).
///
/// Tries DirectWrite first, then falls back to `C:\Windows\Fonts\{name}`.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_user_fallback(family: &str) -> Option<PathBuf> {
    if let Some(path) = resolve_font_dwrite(
        family,
        dwrote::FontWeight::Regular,
        dwrote::FontStyle::Normal,
    ) {
        return Some(path);
    }
    let path = if std::path::Path::new(family).is_absolute() {
        PathBuf::from(family)
    } else {
        PathBuf::from(r"C:\Windows\Fonts").join(family)
    };
    if path.exists() { Some(path) } else { None }
}

/// Resolve fallback font paths via DirectWrite, with static paths as additional fallback.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_fallback_paths_dwrite() -> Vec<PathBuf> {
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
        let path = PathBuf::from(*p);
        if path.exists() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

// Linux font discovery

#[cfg(not(target_os = "windows"))]
fn linux_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    dirs.push(PathBuf::from("/usr/share/fonts"));
    dirs.push(PathBuf::from("/usr/local/share/fonts"));
    dirs
}

/// Build a filename → full path index by scanning all font directories once.
#[cfg(not(target_os = "windows"))]
pub(crate) fn build_font_index() -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    for dir in linux_font_dirs() {
        index_font_dir(&dir, &mut index);
    }
    index
}

#[cfg(not(target_os = "windows"))]
fn index_font_dir(dir: &std::path::Path, index: &mut HashMap<String, PathBuf>) {
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
pub(crate) fn find_font_variant_path(
    names: &[&str],
    index: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    for name in names {
        if let Some(path) = index.get(*name) {
            return Some(path.clone());
        }
    }
    None
}

/// Resolve a user-configured fallback font family name to a file path (Linux).
///
/// Looks up in the pre-built font index first, then tries as an absolute path.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_user_fallback(family: &str, font_index: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    if let Some(path) = font_index.get(family) {
        return Some(path.clone());
    }
    let path = PathBuf::from(family);
    if path.is_absolute() && path.exists() {
        return Some(path);
    }
    None
}

/// Resolve fallback font paths from the pre-built index.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_fallback_paths(font_index: &HashMap<String, PathBuf>) -> Vec<PathBuf> {
    FALLBACK_FONT_NAMES
        .iter()
        .filter_map(|name| font_index.get(*name).cloned())
        .collect()
}
