//! Grid cell instance building — characters, cursors, underlines, block elements.

use vte::ansi::CursorShape;

use crate::cell::CellFlags;
use crate::grid::{GRID_PADDING_LEFT, GRID_PADDING_TOP};
use crate::render::{FontSet, FontStyle};
use crate::search::MatchType;
use crate::tab_bar::TAB_BAR_HEIGHT;
use crate::term_mode::TermMode;
use super::color_util::vte_rgb_to_rgba;
use super::instance_writer::InstanceWriter;
use super::renderer::{FrameParams, GpuRenderer};

impl GpuRenderer {
    #[expect(clippy::too_many_lines, reason = "Flat per-cell rendering loop; further extraction would just scatter sequential logic")]
    pub(super) fn build_grid_instances(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
        default_bg: &[f32; 4],
    ) {
        let grid = params.grid;
        let palette = params.palette;
        let cw = glyphs.cell_width;
        let ch = glyphs.cell_height;
        let baseline = glyphs.baseline;
        let synthetic_bold = glyphs.needs_synthetic_bold();
        let sc = params.scale;
        let x_offset = (GRID_PADDING_LEFT as f32 * sc).round() as usize;
        let y_offset = ((TAB_BAR_HEIGHT + GRID_PADDING_TOP) as f32 * sc).round() as usize;

        let default_bg_u32 = crate::palette::rgb_to_u32(palette.default_bg());

        for line in 0..grid.lines {
            let row = grid.visible_row(line);
            for col in 0..grid.cols {
                let cell = &row[col];
                let x0 = (col * cw + x_offset) as f32;
                let y0 = (line * ch + y_offset) as f32;

                // Skip wide char spacers
                if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                    continue;
                }

                // Resolve colors
                let mut fg_rgb = palette.resolve_fg(cell.fg, cell.bg, cell.flags);
                let mut bg_rgb = palette.resolve_bg(cell.fg, cell.bg, cell.flags);

                // Compute absolute row for search/selection
                let abs_row = grid.scrollback.len().saturating_sub(grid.display_offset) + line;

                // Search match highlighting
                if let Some(search) = params.search {
                    match search.cell_match_type(abs_row, col) {
                        MatchType::FocusedMatch => {
                            bg_rgb = vte::ansi::Rgb {
                                r: 200,
                                g: 120,
                                b: 30,
                            };
                            fg_rgb = vte::ansi::Rgb { r: 0, g: 0, b: 0 };
                        }
                        MatchType::Match => {
                            bg_rgb = vte::ansi::Rgb {
                                r: 80,
                                g: 80,
                                b: 20,
                            };
                        }
                        MatchType::None => {}
                    }
                }

                // Selection highlight
                let is_selected = params
                    .selection
                    .is_some_and(|sel| sel.contains(abs_row, col));
                if is_selected {
                    let (sel_fg, sel_bg) = palette.selection_colors(fg_rgb, bg_rgb);
                    fg_rgb = sel_fg;
                    bg_rgb = sel_bg;
                }

                let bg_u32 = crate::palette::rgb_to_u32(bg_rgb);
                let fg_rgba = vte_rgb_to_rgba(fg_rgb);
                let bg_rgba = vte_rgb_to_rgba(bg_rgb);

                let cell_w = if cell.flags.contains(CellFlags::WIDE_CHAR) {
                    (cw * 2) as f32
                } else {
                    cw as f32
                };

                // Cell background (only if non-default or selected)
                if bg_u32 != default_bg_u32 || is_selected {
                    bg.push_rect(x0, y0, cell_w, ch as f32, bg_rgba);
                }

                // Cursor
                let is_cursor = grid.display_offset == 0
                    && params.mode.contains(TermMode::SHOW_CURSOR)
                    && line == grid.cursor.row
                    && col == grid.cursor.col;
                if is_cursor && params.cursor_visible {
                    let cursor_color = vte_rgb_to_rgba(palette.cursor_color());
                    match params.cursor_shape {
                        CursorShape::Beam => {
                            // 2px vertical bar at left edge
                            bg.push_rect(x0, y0, 2.0 * sc, ch as f32, cursor_color);
                        }
                        CursorShape::Underline => {
                            // 2px horizontal bar at bottom
                            bg.push_rect(
                                x0,
                                y0 + ch as f32 - 2.0 * sc,
                                cw as f32,
                                2.0 * sc,
                                cursor_color,
                            );
                        }
                        _ => {
                            // Block (default): filled rect
                            bg.push_rect(x0, y0, cw as f32, ch as f32, cursor_color);
                        }
                    }
                }

                // Underline and hyperlink decorations
                draw_underlines(
                    bg, cell, x0, y0, ch, cell_w, fg_rgba,
                    palette, params, abs_row, col,
                );

                // Strikethrough
                if cell.flags.contains(CellFlags::STRIKEOUT) {
                    let strike_y = y0 + ch as f32 / 2.0;
                    bg.push_rect(x0, strike_y, cell_w, 1.0, fg_rgba);
                }

                // Glyph (skip space/null)
                if cell.c == ' ' || cell.c == '\0' {
                    continue;
                }

                // Custom block character rendering (pixel-perfect, no font glyph)
                if draw_block_char(cell.c, x0, y0, cell_w, ch as f32, fg_rgba, bg) {
                    continue;
                }

                let style = FontStyle::from_cell_flags(cell.flags);
                let entry = self.atlas.get_or_insert(cell.c, style, glyphs, queue);

                if entry.metrics.width == 0 || entry.metrics.height == 0 {
                    continue;
                }

                // Glyph position
                let gx = x0 + entry.metrics.xmin as f32;
                let gy =
                    y0 + baseline as f32 - entry.metrics.height as f32 - entry.metrics.ymin as f32;

                // Only invert text color for block cursor (beam/underline don't cover the glyph)
                let is_block_cursor = is_cursor
                    && params.cursor_visible
                    && matches!(params.cursor_shape, CursorShape::Block);
                let glyph_fg = if is_block_cursor {
                    *default_bg
                } else {
                    fg_rgba
                };

                // Effective background behind this glyph (for contrast/correction)
                let glyph_bg = if is_block_cursor {
                    vte_rgb_to_rgba(palette.cursor_color())
                } else if bg_u32 != default_bg_u32 || is_selected {
                    bg_rgba
                } else {
                    *default_bg
                };

                fg.push_glyph(
                    gx,
                    gy,
                    entry.metrics.width as f32,
                    entry.metrics.height as f32,
                    entry.uv_pos,
                    entry.uv_size,
                    glyph_fg,
                    glyph_bg,
                );

                // Synthetic bold: render glyph again 1px to the right
                if synthetic_bold && (style == FontStyle::Bold || style == FontStyle::BoldItalic) {
                    fg.push_glyph(
                        gx + 1.0,
                        gy,
                        entry.metrics.width as f32,
                        entry.metrics.height as f32,
                        entry.uv_pos,
                        entry.uv_size,
                        glyph_fg,
                        glyph_bg,
                    );
                }

                // Overlay combining marks (zerowidth characters stored in CellExtra)
                for &zw in cell.zerowidth() {
                    let zw_entry = self.atlas.get_or_insert(zw, style, glyphs, queue);
                    if zw_entry.metrics.width == 0 || zw_entry.metrics.height == 0 {
                        continue;
                    }
                    let zx = x0 + zw_entry.metrics.xmin as f32;
                    let zy = y0 + baseline as f32
                        - zw_entry.metrics.height as f32
                        - zw_entry.metrics.ymin as f32;
                    fg.push_glyph(
                        zx,
                        zy,
                        zw_entry.metrics.width as f32,
                        zw_entry.metrics.height as f32,
                        zw_entry.uv_pos,
                        zw_entry.uv_size,
                        glyph_fg,
                        glyph_bg,
                    );
                }
            }
        }
    }
}

/// Draw a dotted underline: 1px rectangles every other pixel.
fn draw_dotted_line(bg: &mut InstanceWriter, x: f32, y: f32, w: f32, color: [f32; 4]) {
    let steps = w as usize;
    for dx in (0..steps).step_by(2) {
        bg.push_rect(x + dx as f32, y, 1.0, 1.0, color);
    }
}

/// Draw all underline decorations for a single cell: VTE underline styles,
/// OSC 8 hyperlink underlines, and implicit URL underlines.
#[expect(clippy::too_many_arguments, reason = "Cell decoration requires full cell context")]
fn draw_underlines(
    bg: &mut InstanceWriter,
    cell: &crate::cell::Cell,
    x0: f32,
    y0: f32,
    ch: usize,
    cell_w: f32,
    fg_rgba: [f32; 4],
    palette: &crate::palette::Palette,
    params: &FrameParams<'_>,
    abs_row: usize,
    col: usize,
) {
    let underline_y = y0 + ch as f32 - 2.0;

    // VTE underline decorations (UNDERLINE, DOUBLE, DOTTED, DASHED, UNDERCURL)
    if cell.flags.intersects(CellFlags::ANY_UNDERLINE) {
        let ul_color = if let Some(ul) = cell.underline_color() {
            vte_rgb_to_rgba(palette.resolve(ul, CellFlags::empty()))
        } else {
            fg_rgba
        };

        if cell.flags.contains(CellFlags::UNDERCURL) {
            let steps = cell_w as usize;
            for dx in 0..steps {
                let phase = (dx as f32 / cell_w) * std::f32::consts::TAU;
                let offset = (phase.sin() * 2.0).round();
                bg.push_rect(x0 + dx as f32, underline_y + offset, 1.0, 1.0, ul_color);
            }
        } else if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
            bg.push_rect(x0, underline_y, cell_w, 1.0, ul_color);
            bg.push_rect(x0, underline_y - 2.0, cell_w, 1.0, ul_color);
        } else if cell.flags.contains(CellFlags::DOTTED_UNDERLINE) {
            draw_dotted_line(bg, x0, underline_y, cell_w, ul_color);
        } else if cell.flags.contains(CellFlags::DASHED_UNDERLINE) {
            let steps = cell_w as usize;
            for dx in 0..steps {
                if dx % 5 < 3 {
                    bg.push_rect(x0 + dx as f32, underline_y, 1.0, 1.0, ul_color);
                }
            }
        } else if cell.flags.contains(CellFlags::UNDERLINE) {
            bg.push_rect(x0, underline_y, cell_w, 1.0, ul_color);
        } else {
            // No matching underline flag (shouldn't happen with ANY_UNDERLINE guard).
        }
        return; // VTE underline takes precedence — skip hyperlink/URL underlines.
    }

    // OSC 8 hyperlink underline
    if cell.hyperlink().is_some() {
        let is_hovered = params.hover_hyperlink.is_some_and(|hover_uri| {
            cell.hyperlink().is_some_and(|h| h.uri == hover_uri)
        });
        if is_hovered {
            bg.push_rect(x0, underline_y, cell_w, 1.0, fg_rgba);
        } else {
            draw_dotted_line(bg, x0, underline_y, cell_w, fg_rgba);
        }
        return; // Explicit hyperlink takes precedence over implicit URL.
    }

    // Implicit URL underline (Ctrl+hover detected URL, no OSC 8)
    if let Some(segments) = params.hover_url_range {
        let in_url = segments
            .iter()
            .any(|&(r, sc, ec)| abs_row == r && col >= sc && col <= ec);
        if in_url {
            bg.push_rect(x0, underline_y, cell_w, 1.0, fg_rgba);
        }
    }
}

/// Draw a Unicode block element (U+2580-U+259F) as pixel-perfect rectangles.
/// Returns `true` if the character was handled, `false` to fall through to the
/// normal glyph path.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_block_char(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    match c {
        // Upper half block
        '\u{2580}' => bg.push_rect(x, y, w, (h / 2.0).round(), fg),
        // Lower N/8 blocks (U+2581-U+2587)
        '\u{2581}'..='\u{2587}' => {
            let eighths = (c as u32 - 0x2580) as f32;
            let bh = (h * eighths / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Full block
        '\u{2588}' => bg.push_rect(x, y, w, h, fg),
        // Left N/8 blocks (U+2589-U+258F): 7/8 down to 1/8
        '\u{2589}'..='\u{258F}' => {
            let eighths = (0x2590 - c as u32) as f32;
            bg.push_rect(x, y, (w * eighths / 8.0).round(), h, fg);
        }
        // Right half
        '\u{2590}' => {
            let hw = (w / 2.0).round();
            bg.push_rect(x + w - hw, y, hw, h, fg);
        }
        // Shade blocks (25%, 50%, 75%)
        '\u{2591}'..='\u{2593}' => {
            let alpha = (c as u32 - 0x2590) as f32 * 0.25;
            bg.push_rect(x, y, w, h, [fg[0], fg[1], fg[2], fg[3] * alpha]);
        }
        // Upper 1/8
        '\u{2594}' => bg.push_rect(x, y, w, (h / 8.0).round(), fg),
        // Right 1/8
        '\u{2595}' => {
            let bw = (w / 8.0).round();
            bg.push_rect(x + w - bw, y, bw, h, fg);
        }
        // Quadrant block elements (U+2596-U+259F)
        '\u{2596}'..='\u{259F}' => draw_quadrant(c, x, y, w, h, fg, bg),
        _ => return false,
    }
    true
}

/// Draw a quadrant block element (U+2596-U+259F) from a bitmask.
///
/// Each quadrant char maps to a 4-bit mask: TL, TR, BL, BR.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_quadrant(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    // Bitmask per quadrant: bit 3=TL, bit 2=TR, bit 1=BL, bit 0=BR
    // Index 0 = U+2596, index 9 = U+259F
    const QUADRANT_MASKS: [u8; 10] = [
        0b0010, // U+2596: lower left
        0b0001, // U+2597: lower right
        0b1000, // U+2598: upper left
        0b1011, // U+2599: upper left + lower left + lower right
        0b1001, // U+259A: upper left + lower right
        0b1110, // U+259B: upper left + upper right + lower left
        0b1101, // U+259C: upper left + upper right + lower right
        0b0100, // U+259D: upper right
        0b0110, // U+259E: upper right + lower left
        0b0111, // U+259F: upper right + lower left + lower right
    ];

    let idx = (c as u32 - 0x2596) as usize;
    let mask = QUADRANT_MASKS[idx];
    let hw = (w / 2.0).round();
    let hh = (h / 2.0).round();

    if mask & 0b1000 != 0 { bg.push_rect(x,      y,      hw,      hh,      fg); } // TL
    if mask & 0b0100 != 0 { bg.push_rect(x + hw,  y,      w - hw,  hh,      fg); } // TR
    if mask & 0b0010 != 0 { bg.push_rect(x,       y + hh, hw,      h - hh,  fg); } // BL
    if mask & 0b0001 != 0 { bg.push_rect(x + hw,  y + hh, w - hw,  h - hh,  fg); } // BR
}
