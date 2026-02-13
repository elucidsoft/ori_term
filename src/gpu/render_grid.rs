//! Grid cell instance building — characters, cursors, underlines, block elements.

use vte::ansi::CursorShape;

use crate::cell::CellFlags;
use crate::render::{FontSet, FontStyle};
use crate::search::MatchType;
use crate::tab_bar::{GRID_PADDING_LEFT, GRID_PADDING_TOP, TAB_BAR_HEIGHT};
use crate::term_mode::TermMode;
use super::color_util::vte_rgb_to_rgba;
use super::renderer::{FrameParams, GpuRenderer, InstanceWriter};

impl GpuRenderer {
    #[allow(clippy::too_many_lines)]
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

                // Underline decorations
                if cell.flags.intersects(CellFlags::ANY_UNDERLINE) {
                    let ul_color = if let Some(ul) = cell.underline_color() {
                        vte_rgb_to_rgba(palette.resolve(ul, CellFlags::empty()))
                    } else {
                        fg_rgba
                    };

                    let underline_y = y0 + ch as f32 - 2.0;
                    let draw_w = cell_w;

                    if cell.flags.contains(CellFlags::UNDERCURL) {
                        // Approximate undercurl with small rectangles at wave positions
                        let steps = draw_w as usize;
                        for dx in 0..steps {
                            let phase = (dx as f32 / draw_w) * std::f32::consts::TAU;
                            let offset = (phase.sin() * 2.0).round();
                            bg.push_rect(x0 + dx as f32, underline_y + offset, 1.0, 1.0, ul_color);
                        }
                    } else if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                        bg.push_rect(x0, underline_y, draw_w, 1.0, ul_color);
                        bg.push_rect(x0, underline_y - 2.0, draw_w, 1.0, ul_color);
                    } else if cell.flags.contains(CellFlags::DOTTED_UNDERLINE) {
                        // Dotted: every other pixel
                        let steps = draw_w as usize;
                        for dx in (0..steps).step_by(2) {
                            bg.push_rect(x0 + dx as f32, underline_y, 1.0, 1.0, ul_color);
                        }
                    } else if cell.flags.contains(CellFlags::DASHED_UNDERLINE) {
                        // Dashed: 3px on, 2px off
                        let steps = draw_w as usize;
                        for dx in 0..steps {
                            if dx % 5 < 3 {
                                bg.push_rect(x0 + dx as f32, underline_y, 1.0, 1.0, ul_color);
                            }
                        }
                    } else if cell.flags.contains(CellFlags::UNDERLINE) {
                        bg.push_rect(x0, underline_y, draw_w, 1.0, ul_color);
                    } else {
                        // No underline decoration
                    }
                }

                // Hyperlink underline (only when cell doesn't already have an underline)
                if cell.hyperlink().is_some() && !cell.flags.intersects(CellFlags::ANY_UNDERLINE) {
                    let underline_y = y0 + ch as f32 - 2.0;
                    let is_hovered = params.hover_hyperlink.is_some_and(|hover_uri| {
                        cell.hyperlink().is_some_and(|h| h.uri == hover_uri)
                    });
                    if is_hovered {
                        // Solid underline on hover
                        bg.push_rect(x0, underline_y, cell_w, 1.0, fg_rgba);
                    } else {
                        // Dotted underline (every other pixel)
                        let steps = cell_w as usize;
                        for dx in (0..steps).step_by(2) {
                            bg.push_rect(x0 + dx as f32, underline_y, 1.0, 1.0, fg_rgba);
                        }
                    }
                }

                // Implicit URL underline (when hovered via Ctrl, no OSC 8, no explicit underline)
                if let Some(segments) = params.hover_url_range {
                    let in_url = segments
                        .iter()
                        .any(|&(r, sc, ec)| abs_row == r && col >= sc && col <= ec);
                    if in_url
                        && cell.hyperlink().is_none()
                        && !cell.flags.intersects(CellFlags::ANY_UNDERLINE)
                    {
                        let underline_y = y0 + ch as f32 - 2.0;
                        bg.push_rect(x0, underline_y, cell_w, 1.0, fg_rgba);
                    }
                }

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

/// Draw a Unicode block element (U+2580-U+259F) as pixel-perfect rectangles.
/// Returns `true` if the character was handled, `false` to fall through to the
/// normal glyph path.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
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
        '\u{2580}' => {
            bg.push_rect(x, y, w, (h / 2.0).round(), fg);
        }
        // Lower 1/8
        '\u{2581}' => {
            let bh = (h / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower 1/4
        '\u{2582}' => {
            let bh = (h / 4.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower 3/8
        '\u{2583}' => {
            let bh = (h * 3.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower half
        '\u{2584}' => {
            let bh = (h / 2.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower 5/8
        '\u{2585}' => {
            let bh = (h * 5.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower 3/4
        '\u{2586}' => {
            let bh = (h * 3.0 / 4.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Lower 7/8
        '\u{2587}' => {
            let bh = (h * 7.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Full block
        '\u{2588}' => {
            bg.push_rect(x, y, w, h, fg);
        }
        // Left 7/8
        '\u{2589}' => {
            bg.push_rect(x, y, (w * 7.0 / 8.0).round(), h, fg);
        }
        // Left 3/4
        '\u{258A}' => {
            bg.push_rect(x, y, (w * 3.0 / 4.0).round(), h, fg);
        }
        // Left 5/8
        '\u{258B}' => {
            bg.push_rect(x, y, (w * 5.0 / 8.0).round(), h, fg);
        }
        // Left half
        '\u{258C}' => {
            bg.push_rect(x, y, (w / 2.0).round(), h, fg);
        }
        // Left 3/8
        '\u{258D}' => {
            bg.push_rect(x, y, (w * 3.0 / 8.0).round(), h, fg);
        }
        // Left 1/4
        '\u{258E}' => {
            bg.push_rect(x, y, (w / 4.0).round(), h, fg);
        }
        // Left 1/8
        '\u{258F}' => {
            bg.push_rect(x, y, (w / 8.0).round(), h, fg);
        }
        // Right half
        '\u{2590}' => {
            let hw = (w / 2.0).round();
            bg.push_rect(x + w - hw, y, hw, h, fg);
        }
        // Light shade (25%)
        '\u{2591}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.25];
            bg.push_rect(x, y, w, h, shade);
        }
        // Medium shade (50%)
        '\u{2592}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.5];
            bg.push_rect(x, y, w, h, shade);
        }
        // Dark shade (75%)
        '\u{2593}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.75];
            bg.push_rect(x, y, w, h, shade);
        }
        // Upper 1/8
        '\u{2594}' => {
            bg.push_rect(x, y, w, (h / 8.0).round(), fg);
        }
        // Right 1/8
        '\u{2595}' => {
            let bw = (w / 8.0).round();
            bg.push_rect(x + w - bw, y, bw, h, fg);
        }
        // Quadrant block elements (U+2596-U+259F)
        '\u{2596}' => {
            // Quadrant lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y + hh, hw, h - hh, fg);
        }
        '\u{2597}' => {
            // Quadrant lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg);
        }
        '\u{2598}' => {
            // Quadrant upper left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg);
        }
        '\u{2599}' => {
            // Quadrant upper left + lower left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg); // TL
            bg.push_rect(x, y + hh, w, h - hh, fg); // full bottom
        }
        '\u{259A}' => {
            // Quadrant upper left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg); // TL
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg); // BR
        }
        '\u{259B}' => {
            // Quadrant upper left + upper right + lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, w, hh, fg); // full top
            bg.push_rect(x, y + hh, hw, h - hh, fg); // BL
        }
        '\u{259C}' => {
            // Quadrant upper left + upper right + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, w, hh, fg); // full top
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg); // BR
        }
        '\u{259D}' => {
            // Quadrant upper right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg);
        }
        '\u{259E}' => {
            // Quadrant upper right + lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg); // TR
            bg.push_rect(x, y + hh, hw, h - hh, fg); // BL
        }
        '\u{259F}' => {
            // Quadrant upper right + lower left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg); // TR
            bg.push_rect(x, y + hh, w, h - hh, fg); // full bottom
        }
        _ => return false,
    }
    true
}
