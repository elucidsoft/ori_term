use std::collections::HashMap;

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::palette::{Palette, rgb_to_u32};
use crate::term_mode::TermMode;

pub const FONT_SIZE: f32 = 16.0;

pub struct GlyphCache {
    pub font: fontdue::Font,
    pub size: f32,
    pub cell_width: usize,
    pub cell_height: usize,
    pub baseline: usize,
    map: HashMap<char, (fontdue::Metrics, Vec<u8>)>,
}

impl GlyphCache {
    pub fn new(font_data: &[u8], size: f32) -> Self {
        let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
            .expect("failed to parse font");

        let lm = font
            .horizontal_line_metrics(size)
            .expect("no line metrics");
        let cell_height = (lm.ascent - lm.descent).ceil() as usize;
        let baseline = lm.ascent.ceil() as usize;

        let (m, _) = font.rasterize('M', size);
        let cell_width = m.advance_width.ceil() as usize;

        Self {
            font,
            size,
            cell_width,
            cell_height,
            baseline,
            map: HashMap::new(),
        }
    }

    pub fn ensure(&mut self, ch: char) {
        if !self.map.contains_key(&ch) {
            let r = self.font.rasterize(ch, self.size);
            self.map.insert(ch, r);
        }
    }

    pub fn get(&self, ch: char) -> Option<&(fontdue::Metrics, Vec<u8>)> {
        self.map.get(&ch)
    }
}

/// Render a grid into a pixel buffer at the given offsets (in pixels).
/// The buffer is assumed to be `buf_w` pixels wide.
pub fn render_grid(
    glyphs: &mut GlyphCache,
    grid: &Grid,
    palette: &Palette,
    mode: TermMode,
    buffer: &mut [u32],
    buf_w: usize,
    buf_h: usize,
    x_offset: usize,
    y_offset: usize,
) {
    let cw = glyphs.cell_width;
    let cell_h = glyphs.cell_height;
    let baseline = glyphs.baseline;

    // Pre-cache visible chars
    for line in 0..grid.lines {
        let row = grid.visible_row(line);
        for col in 0..grid.cols {
            let cell = &row[col];
            if cell.c != ' ' && cell.c != '\0' && !cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                glyphs.ensure(cell.c);
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
            let fg_rgb = palette.resolve_fg(cell.fg, cell.bg, cell.flags);
            let bg_rgb = palette.resolve_bg(cell.fg, cell.bg, cell.flags);
            let bg_u32 = rgb_to_u32(bg_rgb);
            let fg_u32 = rgb_to_u32(fg_rgb);

            // Draw cell background if non-default
            let cell_w = if cell.flags.contains(CellFlags::WIDE_CHAR) { cw * 2 } else { cw };
            if bg_u32 != default_bg_u32 {
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

            if cell.c == ' ' || cell.c == '\0' {
                continue;
            }

            if let Some((metrics, bitmap)) = glyphs.get(cell.c) {
                let gx = x0 as i32 + metrics.xmin;
                let gy = y0 as i32 + baseline as i32 - metrics.height as i32 - metrics.ymin;

                let fg_r = (fg_u32 >> 16) & 0xFF;
                let fg_g = (fg_u32 >> 8) & 0xFF;
                let fg_b = fg_u32 & 0xFF;

                // If cursor is on this cell, use black text for contrast
                let (draw_r, draw_g, draw_b, draw_u32) =
                    if grid.display_offset == 0
                        && mode.contains(TermMode::SHOW_CURSOR)
                        && line == grid.cursor.row
                        && col == grid.cursor.col
                    {
                        // Use dark text over cursor
                        let dark = palette.default_bg();
                        let du32 = rgb_to_u32(dark);
                        ((du32 >> 16) & 0xFF, (du32 >> 8) & 0xFF, du32 & 0xFF, du32)
                    } else {
                        (fg_r, fg_g, fg_b, fg_u32)
                    };

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
                }
            }
        }
    }
}

/// Render a single text string into the buffer at pixel position (x, y).
/// Used for tab bar labels, etc.
#[allow(clippy::many_single_char_names)]
pub fn render_text(
    glyphs: &mut GlyphCache,
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

    for ch in text.chars() {
        if cx + cw > buf_w {
            break;
        }
        glyphs.ensure(ch);
        if let Some((metrics, bitmap)) = glyphs.get(ch) {
            let gx = cx as i32 + metrics.xmin;
            let gy = y as i32 + baseline as i32 - metrics.height as i32 - metrics.ymin;

            let fg_r = (fg >> 16) & 0xFF;
            let fg_g = (fg >> 8) & 0xFF;
            let fg_b = fg & 0xFF;

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
                    if alpha == 255 {
                        buffer[pidx] = fg;
                    } else {
                        let bg_val = buffer[pidx];
                        let inv = 255 - alpha;
                        let r = (fg_r * alpha + ((bg_val >> 16) & 0xFF) * inv) / 255;
                        let g = (fg_g * alpha + ((bg_val >> 8) & 0xFF) * inv) / 255;
                        let b = (fg_b * alpha + (bg_val & 0xFF) * inv) / 255;
                        buffer[pidx] = (r << 16) | (g << 8) | b;
                    }
                }
            }
        }
        cx += cw;
    }
}

pub fn load_font() -> Vec<u8> {
    let candidates = [
        r"C:\Windows\Fonts\CascadiaMono.ttf",
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\cour.ttf",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return data;
        }
    }
    panic!("no suitable monospace font found");
}
