//! Grid cell instance building — characters, cursors, underlines, block elements.

use vte::ansi::CursorShape;

use crate::cell::CellFlags;
use crate::font::{FontCollection, shape_line};
use crate::grid::{GRID_PADDING_LEFT, GRID_PADDING_TOP, StableRowIndex};
use crate::render::FontStyle;
use crate::search::MatchType;
use crate::tab_bar::TAB_BAR_HEIGHT;
use crate::term_mode::TermMode;
use super::atlas::size_key;
use super::builtin_glyphs;
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
        collection: &mut FontCollection,
        queue: &wgpu::Queue,
        default_bg: &[f32; 4],
    ) {
        let grid = params.grid;
        let palette = params.palette;
        let cw = collection.cell_width;
        let ch = collection.cell_height;
        let baseline = collection.baseline;
        let synthetic_bold = collection.needs_synthetic_bold();
        let sc = params.scale;
        let x_offset = (GRID_PADDING_LEFT as f32 * sc).round() as usize;
        let y_offset = ((TAB_BAR_HEIGHT + GRID_PADDING_TOP) as f32 * sc).round() as usize;
        let size_q6 = size_key(collection.size);

        let default_bg_u32 = crate::palette::rgb_to_u32(palette.default_bg());

        for line in 0..grid.lines {
            let row = grid.visible_row(line);

            // Shape this line: produces glyphs with IDs, positions, and spans.
            // Combining marks are folded into base glyphs by the shaper.
            let shaped = shape_line(row.as_slice(), grid.cols, collection);
            self.col_glyph_map.clear();
            self.col_glyph_map.resize(grid.cols, None);
            for (i, g) in shaped.iter().enumerate() {
                self.col_glyph_map[g.col_start] = Some(i);
            }

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

                // Compute stable row index for search/selection
                let stable_row = StableRowIndex::from_visible(grid, line);

                // Search match highlighting
                if let Some(search) = params.search {
                    match search.cell_match_type(stable_row, col) {
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
                    .is_some_and(|sel| sel.contains(stable_row, col));
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
                let abs_row = grid.viewport_to_absolute(line);
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

                // Built-in glyph rendering (pixel-perfect, no font glyph).
                // Covers box drawing, block elements, braille, and Powerline.
                // Temporarily force opacity=1.0 so built-in glyphs match atlas
                // text brightness. The bg writer premultiplies color by opacity,
                // but text glyphs on the fg writer are fully opaque.
                let saved_opacity = bg.opacity;
                bg.opacity = 1.0;
                if builtin_glyphs::draw_builtin_glyph(cell.c, x0, y0, cell_w, ch as f32, fg_rgba, bg) {
                    bg.opacity = saved_opacity;
                    continue;
                }
                bg.opacity = saved_opacity;

                // Shaped glyph rendering: look up the pre-shaped glyph for this column.
                // Columns without a mapped glyph are ligature continuations — skip them.
                let Some(glyph_idx) = self.col_glyph_map[col] else {
                    continue;
                };
                let glyph = shaped[glyph_idx];
                let face_idx = glyph.face_idx;
                let gid = glyph.glyph_id;
                let entry = self.atlas.get_or_insert_shaped(
                    gid,
                    face_idx.0,
                    size_q6,
                    || collection.rasterize_glyph(face_idx, gid),
                    queue,
                );

                if entry.metrics.width == 0 || entry.metrics.height == 0 {
                    continue;
                }

                // Glyph position (shaper offsets applied for combining/ligature positioning)
                let gx = x0 + entry.metrics.xmin as f32 + glyph.x_offset;
                let gy = y0 + baseline as f32
                    - entry.metrics.height as f32
                    - entry.metrics.ymin as f32
                    + glyph.y_offset;

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
                    entry.page,
                );

                // Synthetic bold: render glyph again 1px to the right
                if synthetic_bold {
                    let style = FontStyle::from_cell_flags(cell.flags);
                    if style == FontStyle::Bold || style == FontStyle::BoldItalic {
                        fg.push_glyph(
                            gx + 1.0,
                            gy,
                            entry.metrics.width as f32,
                            entry.metrics.height as f32,
                            entry.uv_pos,
                            entry.uv_size,
                            glyph_fg,
                            glyph_bg,
                            entry.page,
                        );
                    }
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

