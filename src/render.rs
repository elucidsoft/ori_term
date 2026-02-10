use std::collections::HashMap;

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::palette::{Palette, rgb_to_u32};
use crate::selection::Selection;
use crate::term_mode::TermMode;

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
        match (flags.contains(CellFlags::BOLD), flags.contains(CellFlags::ITALIC)) {
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
        regular: &["JetBrainsMono-Regular.ttf", "JetBrainsMonoNerdFont-Regular.ttf"],
        bold: &["JetBrainsMono-Bold.ttf", "JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &["JetBrainsMono-Italic.ttf", "JetBrainsMonoNerdFont-Italic.ttf"],
        bold_italic: &["JetBrainsMono-BoldItalic.ttf", "JetBrainsMonoNerdFont-BoldItalic.ttf"],
    },
    FontFamily {
        regular: &["UbuntuMono-Regular.ttf", "UbuntuMonoNerdFont-Regular.ttf"],
        bold: &["UbuntuMono-Bold.ttf", "UbuntuMonoNerdFont-Bold.ttf"],
        italic: &["UbuntuMono-Italic.ttf", "UbuntuMonoNerdFont-Italic.ttf"],
        bold_italic: &["UbuntuMono-BoldItalic.ttf", "UbuntuMonoNerdFont-BoldItalic.ttf"],
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
    r"C:\Windows\Fonts\seguisym.ttf",  // Segoe UI Symbol
    r"C:\Windows\Fonts\msgothic.ttc",  // MS Gothic (CJK)
    r"C:\Windows\Fonts\segoeui.ttf",   // Segoe UI
];

#[cfg(not(target_os = "windows"))]
const FALLBACK_FONT_NAMES: &[&str] = &[
    "NotoSansMono-Regular.ttf",
    "NotoSansSymbols2-Regular.ttf",
    "NotoSansCJK-Regular.ttc",
    "DejaVuSans.ttf",
];

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

#[cfg(not(target_os = "windows"))]
fn find_font_file(name: &str) -> Option<Vec<u8>> {
    for dir in linux_font_dirs() {
        if let Some(data) = find_font_in_dir(&dir, name) {
            return Some(data);
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn find_font_in_dir(dir: &std::path::Path, name: &str) -> Option<Vec<u8>> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(data) = find_font_in_dir(&path, name) {
                return Some(data);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return std::fs::read(&path).ok();
        } else {
            // Not a match, continue scanning
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn load_font_from_paths(paths: &[&str]) -> Option<Vec<u8>> {
    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn load_font_variant(names: &[&str]) -> Option<Vec<u8>> {
    for name in names {
        if let Some(data) = find_font_file(name) {
            return Some(data);
        }
    }
    None
}

fn parse_font(data: &[u8]) -> Option<fontdue::Font> {
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
}

// --- FontSet ---

pub struct FontSet {
    fonts: [fontdue::Font; 4],
    has_variant: [bool; 4],
    fallback_fonts: Vec<fontdue::Font>,
    pub size: f32,
    pub cell_width: usize,
    pub cell_height: usize,
    pub baseline: usize,
    cache: HashMap<(char, FontStyle), (fontdue::Metrics, Vec<u8>)>,
}

impl FontSet {
    /// Load a font set at the given size, trying font families in priority order.
    pub fn load(size: f32) -> Self {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        for family in FONT_FAMILIES {
            if let Some(fs) = Self::try_load_family(family, size) {
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
        let fallback_fonts = self.fallback_fonts.clone();

        let (cell_width, cell_height, baseline) = Self::compute_metrics(&fonts[0], new_size);

        let mut fs = Self {
            fonts,
            has_variant,
            fallback_fonts,
            size: new_size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        };
        fs.precache_ascii();
        fs
    }

    fn try_load_family(family: &FontFamily, size: f32) -> Option<Self> {
        #[cfg(target_os = "windows")]
        let regular_data = load_font_from_paths(family.regular)?;
        #[cfg(not(target_os = "windows"))]
        let regular_data = load_font_variant(family.regular)?;

        let regular = parse_font(&regular_data)?;

        let mut fonts = [regular.clone(), regular.clone(), regular.clone(), regular];
        let mut has_variant = [true, false, false, false];

        // Try loading each variant
        let variant_specs: [(usize, &[&str]); 3] = [
            (1, family.bold),
            (2, family.italic),
            (3, family.bold_italic),
        ];

        for (idx, paths) in variant_specs {
            #[cfg(target_os = "windows")]
            let data = load_font_from_paths(paths);
            #[cfg(not(target_os = "windows"))]
            let data = load_font_variant(paths);

            if let Some(data) = data {
                if let Some(font) = parse_font(&data) {
                    fonts[idx] = font;
                    has_variant[idx] = true;
                }
            }
        }

        let (cell_width, cell_height, baseline) = Self::compute_metrics(&fonts[0], size);

        let fallback_fonts = Self::load_fallback_fonts();

        let mut fs = Self {
            fonts,
            has_variant,
            fallback_fonts,
            size,
            cell_width,
            cell_height,
            baseline,
            cache: HashMap::new(),
        };
        fs.precache_ascii();
        Some(fs)
    }

    fn compute_metrics(font: &fontdue::Font, size: f32) -> (usize, usize, usize) {
        let lm = font.horizontal_line_metrics(size).expect("no line metrics");
        let cell_height = (lm.ascent - lm.descent).ceil() as usize;
        let baseline = lm.ascent.ceil() as usize;
        let (m, _) = font.rasterize('M', size);
        let cell_width = m.advance_width.ceil() as usize;
        (cell_width, cell_height, baseline)
    }

    fn load_fallback_fonts() -> Vec<fontdue::Font> {
        let mut fallbacks = Vec::new();

        #[cfg(target_os = "windows")]
        {
            for path in FALLBACK_FONT_PATHS {
                if let Ok(data) = std::fs::read(path) {
                    if let Some(font) = parse_font(&data) {
                        fallbacks.push(font);
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            for name in FALLBACK_FONT_NAMES {
                if let Some(data) = find_font_file(name) {
                    if let Some(font) = parse_font(&data) {
                        fallbacks.push(font);
                    }
                }
            }
        }

        fallbacks
    }

    fn precache_ascii(&mut self) {
        for ch in ' '..='~' {
            self.ensure(ch, FontStyle::Regular);
        }
    }

    /// Rasterize a glyph with the fallback chain.
    fn rasterize_with_fallback(&self, ch: char, style: FontStyle) -> (fontdue::Metrics, Vec<u8>) {
        let idx = style as usize;

        // 1. Try requested style font
        if self.fonts[idx].has_glyph(ch) {
            return self.fonts[idx].rasterize(ch, self.size);
        }

        // 2. Try Regular font (style fallback)
        if style != FontStyle::Regular && self.fonts[0].has_glyph(ch) {
            return self.fonts[0].rasterize(ch, self.size);
        }

        // 3. Try fallback fonts
        for fb in &self.fallback_fonts {
            if fb.has_glyph(ch) {
                return fb.rasterize(ch, self.size);
            }
        }

        // 4. Replacement character
        if self.fonts[0].has_glyph('\u{FFFD}') {
            return self.fonts[0].rasterize('\u{FFFD}', self.size);
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

    /// Whether bold needs synthetic rendering (no real bold font loaded).
    pub fn needs_synthetic_bold(&self) -> bool {
        !self.has_variant[FontStyle::Bold as usize]
    }
}

// --- Drawing helpers ---

fn set_pixel(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, color: u32) {
    if x < buf_w {
        let idx = y * buf_w + x;
        if idx < buffer.len() {
            buffer[idx] = color;
        }
    }
}

fn draw_hline(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, color: u32) {
    for dx in 0..w {
        set_pixel(buffer, buf_w, x + dx, y, color);
    }
}

fn draw_dotted_hline(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, color: u32) {
    for dx in (0..w).step_by(2) {
        set_pixel(buffer, buf_w, x + dx, y, color);
    }
}

fn draw_dashed_hline(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, color: u32) {
    for dx in 0..w {
        // 3px on, 2px off pattern
        if dx % 5 < 3 {
            set_pixel(buffer, buf_w, x + dx, y, color);
        }
    }
}

fn draw_undercurl(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, color: u32) {
    // Sine wave approximation: 2px amplitude, period = cell width
    // Use a lookup pattern for a smooth curl across the cell width
    for dx in 0..w {
        let phase = (dx as f32 / w as f32) * std::f32::consts::TAU;
        let offset = (phase.sin() * 2.0).round() as i32;
        let py = y as i32 + offset;
        if py >= 0 {
            set_pixel(buffer, buf_w, x + dx, py as usize, color);
        }
    }
}

/// Alpha-blend a glyph pixel onto the buffer.
#[inline]
fn blend_pixel(buffer: &mut [u32], pidx: usize, alpha: u32, draw_r: u32, draw_g: u32, draw_b: u32, draw_u32: u32) {
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

// --- Grid rendering ---

/// Render a grid into a pixel buffer at the given offsets (in pixels).
/// The buffer is assumed to be `buf_w` pixels wide.
#[allow(clippy::too_many_arguments)]
pub fn render_grid(
    glyphs: &mut FontSet,
    grid: &Grid,
    palette: &Palette,
    mode: TermMode,
    selection: Option<&Selection>,
    buffer: &mut [u32],
    buf_w: usize,
    buf_h: usize,
    x_offset: usize,
    y_offset: usize,
) {
    let cw = glyphs.cell_width;
    let cell_h = glyphs.cell_height;
    let baseline = glyphs.baseline;
    let synthetic_bold = glyphs.needs_synthetic_bold();

    // Pre-cache visible chars with correct font style
    for line in 0..grid.lines {
        let row = grid.visible_row(line);
        for col in 0..grid.cols {
            let cell = &row[col];
            if cell.c != ' ' && cell.c != '\0' && !cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                let style = FontStyle::from_cell_flags(cell.flags);
                glyphs.ensure(cell.c, style);
            }
        }
    }

    let default_bg_u32 = rgb_to_u32(palette.default_bg());

    for line in 0..grid.lines {
        let row = grid.visible_row(line);
        for col in 0..grid.cols {
            let cell = &row[col];
            let x0 = col * cw + x_offset;
            let y0 = line * cell_h + y_offset;

            if x0 >= buf_w || y0 >= buf_h {
                continue;
            }

            // Skip wide char spacer cells
            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }

            // Resolve colors
            let mut fg_rgb = palette.resolve_fg(cell.fg, cell.bg, cell.flags);
            let mut bg_rgb = palette.resolve_bg(cell.fg, cell.bg, cell.flags);

            // Selection highlight: invert fg/bg for selected cells
            let is_selected = selection.is_some_and(|sel| {
                let abs_row = grid.scrollback.len()
                    .saturating_sub(grid.display_offset)
                    + line;
                sel.contains(abs_row, col)
            });
            if is_selected {
                std::mem::swap(&mut fg_rgb, &mut bg_rgb);
            }

            let bg_u32 = rgb_to_u32(bg_rgb);
            let fg_u32 = rgb_to_u32(fg_rgb);

            // Draw cell background if non-default or selected
            let cell_w = if cell.flags.contains(CellFlags::WIDE_CHAR) { cw * 2 } else { cw };
            if bg_u32 != default_bg_u32 || is_selected {
                for dy in 0..cell_h.min(buf_h - y0) {
                    for dx in 0..cell_w.min(buf_w - x0) {
                        buffer[(y0 + dy) * buf_w + (x0 + dx)] = bg_u32;
                    }
                }
            }

            // Draw cursor block (only on live viewport)
            if grid.display_offset == 0
                && mode.contains(TermMode::SHOW_CURSOR)
                && line == grid.cursor.row
                && col == grid.cursor.col
            {
                let cursor_u32 = rgb_to_u32(palette.cursor_color());
                for dy in 0..cell_h.min(buf_h - y0) {
                    for dx in 0..cw.min(buf_w - x0) {
                        buffer[(y0 + dy) * buf_w + (x0 + dx)] = cursor_u32;
                    }
                }
            }

            // Resolve underline color (SGR 58 override or fg)
            let ul_u32 = if let Some(ul_color) = cell.underline_color() {
                rgb_to_u32(palette.resolve(ul_color, CellFlags::empty()))
            } else {
                fg_u32
            };

            // Draw underline decorations (before glyph so glyph renders on top for undercurl)
            if cell.flags.intersects(CellFlags::ANY_UNDERLINE) {
                let underline_y = y0 + cell_h.saturating_sub(2);
                if underline_y < buf_h {
                    let draw_w = cell_w.min(buf_w.saturating_sub(x0));
                    if cell.flags.contains(CellFlags::UNDERCURL) {
                        draw_undercurl(buffer, buf_w, x0, underline_y, draw_w, ul_u32);
                    } else if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                        draw_hline(buffer, buf_w, x0, underline_y, draw_w, ul_u32);
                        if underline_y >= 2 {
                            draw_hline(buffer, buf_w, x0, underline_y - 2, draw_w, ul_u32);
                        }
                    } else if cell.flags.contains(CellFlags::DOTTED_UNDERLINE) {
                        draw_dotted_hline(buffer, buf_w, x0, underline_y, draw_w, ul_u32);
                    } else if cell.flags.contains(CellFlags::DASHED_UNDERLINE) {
                        draw_dashed_hline(buffer, buf_w, x0, underline_y, draw_w, ul_u32);
                    } else if cell.flags.contains(CellFlags::UNDERLINE) {
                        draw_hline(buffer, buf_w, x0, underline_y, draw_w, ul_u32);
                    } else {
                        // No recognized underline variant — shouldn't reach here
                    }
                }
            }

            // Draw strikethrough
            if cell.flags.contains(CellFlags::STRIKEOUT) {
                let strike_y = y0 + cell_h / 2;
                if strike_y < buf_h {
                    let draw_w = cell_w.min(buf_w.saturating_sub(x0));
                    draw_hline(buffer, buf_w, x0, strike_y, draw_w, fg_u32);
                }
            }

            if cell.c == ' ' || cell.c == '\0' {
                continue;
            }

            let style = FontStyle::from_cell_flags(cell.flags);
            if let Some((metrics, bitmap)) = glyphs.get(cell.c, style) {
                let gx = x0 as i32 + metrics.xmin;
                let gy = y0 as i32 + baseline as i32 - metrics.height as i32 - metrics.ymin;

                // If cursor is on this cell, use dark text for contrast
                let (draw_r, draw_g, draw_b, draw_u32) =
                    if grid.display_offset == 0
                        && mode.contains(TermMode::SHOW_CURSOR)
                        && line == grid.cursor.row
                        && col == grid.cursor.col
                    {
                        let dark = palette.default_bg();
                        let du32 = rgb_to_u32(dark);
                        ((du32 >> 16) & 0xFF, (du32 >> 8) & 0xFF, du32 & 0xFF, du32)
                    } else {
                        ((fg_u32 >> 16) & 0xFF, (fg_u32 >> 8) & 0xFF, fg_u32 & 0xFF, fg_u32)
                    };

                let use_synthetic = synthetic_bold
                    && (style == FontStyle::Bold || style == FontStyle::BoldItalic);
                render_glyph(
                    buffer, buf_w, buf_h, metrics, bitmap,
                    gx, gy, draw_r, draw_g, draw_b, draw_u32,
                    use_synthetic,
                );
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
        if cx + cw > buf_w {
            break;
        }
        glyphs.ensure(ch, FontStyle::Regular);
        if let Some((metrics, bitmap)) = glyphs.get(ch, FontStyle::Regular) {
            let gx = cx as i32 + metrics.xmin;
            let gy = y as i32 + baseline as i32 - metrics.height as i32 - metrics.ymin;

            render_glyph(
                buffer, buf_w, buf_h, metrics, bitmap,
                gx, gy, fg_r, fg_g, fg_b, fg,
                false,
            );
        }
        cx += cw;
    }
}
