//! Platform-specific font discovery — finding font files on disk.
//!
//! Handles DirectWrite resolution on Windows and directory scanning on Linux.
//! Pure discovery: no font loading, rasterizing, or caching.

#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::path::PathBuf;

// Font family definitions

pub(super) struct FontFamily {
    pub regular: &'static [&'static str],
    pub bold: &'static [&'static str],
    pub italic: &'static [&'static str],
    pub bold_italic: &'static [&'static str],
}

#[cfg(target_os = "windows")]
pub(super) const FONT_FAMILIES: &[FontFamily] = &[
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
pub(super) const FONT_FAMILIES: &[FontFamily] = &[
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
pub(super) const DWRITE_FAMILIES: &[&str] = &[
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
pub(super) const DWRITE_UI_FAMILIES: &[&str] = &["Segoe UI", "Tahoma", "Arial"];

/// Detect the OS system UI font family name.
///
/// On Windows, queries `SystemParametersInfo(SPI_GETNONCLIENTMETRICS)` to get
/// the message font (used for dialogs and general UI text).
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub(super) fn detect_system_ui_font_name() -> Option<String> {
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
pub(super) const UI_FONT_NAMES: &[&str] = &[
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
pub(super) fn resolve_font_dwrite(
    family_name: &str,
    weight: dwrote::FontWeight,
    style: dwrote::FontStyle,
) -> Option<PathBuf> {
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
pub(super) fn resolve_family_paths_dwrite(family_name: &str) -> Option<[Option<PathBuf>; 4]> {
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
pub(super) fn resolve_fallback_paths_dwrite() -> Vec<PathBuf> {
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
pub(super) fn build_font_index() -> HashMap<String, PathBuf> {
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
pub(super) fn find_font_variant_path(
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

/// Resolve fallback font paths from the pre-built index.
#[cfg(not(target_os = "windows"))]
pub(super) fn resolve_fallback_paths(font_index: &HashMap<String, PathBuf>) -> Vec<PathBuf> {
    FALLBACK_FONT_NAMES
        .iter()
        .filter_map(|name| font_index.get(*name).cloned())
        .collect()
}
