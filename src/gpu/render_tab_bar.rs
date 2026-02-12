//! Tab bar instance building — inactive/active tabs, close buttons, window controls.

use super::color_util::{
    CONTROL_CLOSE_HOVER_BG, CONTROL_CLOSE_HOVER_FG, TabBarColors, lerp_color, lighten,
};
use super::renderer::{FrameParams, GpuRenderer, InstanceWriter};
use crate::render::FontSet;
use crate::tab_bar::{
    DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TabBarHit,
    TabBarLayout,
};

const TAB_TOP_MARGIN: usize = 8;
const TAB_PADDING: usize = 8;
const CLOSE_BUTTON_WIDTH: usize = 24;
const CLOSE_BUTTON_RIGHT_PAD: usize = 8;

// Windows 10/11 style: wide rectangular buttons
#[cfg(target_os = "windows")]
const CONTROL_BUTTON_WIDTH: usize = 58;
#[cfg(target_os = "windows")]
const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_WIDTH * 3;
#[cfg(target_os = "windows")]
const ICON_SIZE: usize = 10;

// Linux (GNOME-style): circular buttons, semi-transparent
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_DIAMETER: usize = 24;
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_SPACING: usize = 8;
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_MARGIN: usize = 12;
#[cfg(not(target_os = "windows"))]
const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_MARGIN
    + 3 * CONTROL_BUTTON_DIAMETER
    + 2 * CONTROL_BUTTON_SPACING
    + CONTROL_BUTTON_MARGIN;
#[cfg(not(target_os = "windows"))]
const ICON_SIZE: usize = 8;
#[cfg(not(target_os = "windows"))]
const CONTROL_CIRCLE_ALPHA: f32 = 0.3;

impl GpuRenderer {
    pub(super) fn build_tab_bar_instances(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let tc = TabBarColors::from_palette(params.palette);
        let w = params.width as f32;
        let s = params.scale;
        let tab_bar_h = TAB_BAR_HEIGHT as f32 * s;

        // Full-width bar background (darkest layer)
        bg.push_rect(0.0, 0.0, w, tab_bar_h, tc.bar_bg);

        let tab_count = params.tab_info.len();
        let layout = TabBarLayout::compute(
            tab_count,
            params.width as usize,
            s as f64,
            params.tab_width_lock,
        );
        let tab_w = layout.tab_width;

        let cell_h = glyphs.cell_height;

        let dragged_idx = params.dragged_tab.map(|(idx, _)| idx);
        let top = TAB_TOP_MARGIN as f32 * s;
        let tab_h = tab_bar_h - top;

        let tab_wf = tab_w as f32;

        // Pass 1: inactive tabs (drawn first, behind active)
        let left_margin = (TAB_LEFT_MARGIN as f32 * s) as usize;
        for (i, (_id, title)) in params.tab_info.iter().enumerate() {
            if Some(i) == dragged_idx || i == params.active_tab {
                continue;
            }

            let base_x = (left_margin + i * tab_w) as f32;
            let x0 = base_x + params.tab_offsets.get(i).copied().unwrap_or(0.0);
            let is_hovered = params.hover_hit == TabBarHit::Tab(i);

            let has_bell = params.bell_badges.get(i).copied().unwrap_or(false);
            let tab_bg = if is_hovered {
                tc.tab_hover_bg
            } else if has_bell {
                // Subtle sine pulse between inactive and hover bg
                let t = params.bell_phase.clamp(0.0, 1.0);
                lerp_color(tc.inactive_bg, tc.tab_hover_bg, t)
            } else {
                tc.inactive_bg
            };

            bg.push_rounded_rect(x0, top, tab_wf, tab_h, tab_bg, 8.0 * s);

            // Separator — hidden next to active, hovered, or dragged tabs
            let next_is_active = (i + 1) == params.active_tab;
            let next_is_dragged = Some(i + 1) == dragged_idx;
            let next_is_hovered = params.hover_hit == TabBarHit::Tab(i + 1);
            if !is_hovered && !next_is_active && !next_is_dragged && !next_is_hovered {
                let sep_x = x0 + tab_wf - 0.5;
                bg.push_rect(
                    sep_x,
                    top + 8.0 * s,
                    1.0 * s,
                    tab_h - 16.0 * s,
                    tc.separator,
                );
            }

            self.render_tab_content(
                bg,
                fg,
                x0,
                tab_w,
                cell_h,
                title,
                tc.inactive_text,
                &tc,
                i,
                params,
                glyphs,
                queue,
            );
        }

        // Pass 2: active tab (drawn on top of inactive)
        if let Some((_id, title)) = params.tab_info.get(params.active_tab) {
            if Some(params.active_tab) != dragged_idx {
                let base_x = (left_margin + params.active_tab * tab_w) as f32;
                let x0 = base_x
                    + params
                        .tab_offsets
                        .get(params.active_tab)
                        .copied()
                        .unwrap_or(0.0);

                bg.push_rounded_rect(x0, top, tab_wf, tab_h, tc.active_bg, 8.0 * s);

                self.render_tab_content(
                    bg,
                    fg,
                    x0,
                    tab_w,
                    cell_h,
                    title,
                    tc.text_fg,
                    &tc,
                    params.active_tab,
                    params,
                    glyphs,
                    queue,
                );
            }
        }

        // Dragged tab is rendered in the overlay pass (see build_dragged_tab_overlay)
        // so its bg+fg both draw AFTER all main-pass bg+fg, giving correct occlusion.

        // New tab "+" and dropdown buttons — when dragging, these are rendered
        // in the overlay pass so they move with the dragged tab (fast path).
        if params.dragged_tab.is_none() {
            let new_tab_w = NEW_TAB_BUTTON_WIDTH as f32 * s;
            let plus_x = (left_margin + tab_count * tab_w) as f32;
            let plus_hovered = params.hover_hit == TabBarHit::NewTab;
            let plus_bg = if plus_hovered {
                tc.button_hover_bg
            } else {
                tc.bar_bg
            };
            bg.push_rect(plus_x, top, new_tab_w, tab_h, plus_bg);
            let plus_cx = plus_x + new_tab_w / 2.0;
            let plus_cy = top + tab_h / 2.0;
            self.push_icon(
                fg,
                crate::icons::Icon::Plus,
                plus_cx,
                plus_cy,
                10.0,
                s,
                tc.text_fg,
                queue,
            );

            // Dropdown button — vector chevron icon
            let dropdown_w = DROPDOWN_BUTTON_WIDTH as f32 * s;
            let dropdown_x = plus_x + new_tab_w;
            let dropdown_hovered = params.hover_hit == TabBarHit::DropdownButton;
            let dropdown_bg = if dropdown_hovered || params.context_menu.is_some() {
                tc.button_hover_bg
            } else {
                tc.bar_bg
            };
            bg.push_rect(dropdown_x, top, dropdown_w, tab_h, dropdown_bg);
            let dd_cx = dropdown_x + dropdown_w / 2.0;
            let dd_cy = top + tab_h / 2.0;
            self.push_icon(
                fg,
                crate::icons::Icon::ChevronDown,
                dd_cx,
                dd_cy,
                10.0,
                s,
                tc.text_fg,
                queue,
            );
        }

        // Window control buttons
        let controls_zone_w = (CONTROLS_ZONE_WIDTH as f32 * s) as usize;
        let controls_start = (params.width as usize).saturating_sub(controls_zone_w) as f32;
        self.build_window_controls(bg, controls_start, params, &tc);
    }

    /// Render a single tab's text content and close button.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_tab_content(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        x0: f32,
        tab_w: usize,
        cell_h: usize,
        title: &str,
        text_fg: [f32; 4],
        tc: &TabBarColors,
        tab_idx: usize,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let s = params.scale;
        let top = TAB_TOP_MARGIN as f32 * s;
        let tab_h = TAB_BAR_HEIGHT as f32 * s - top;
        let tab_padding = (TAB_PADDING as f32 * s) as usize;
        let close_btn_w = (CLOSE_BUTTON_WIDTH as f32 * s) as usize;

        // Title text — only truncated if it overflows the available space
        let max_text_px = (tab_w - tab_padding * 2 - close_btn_w) as f32;
        let display_title = glyphs.truncate_to_pixel_width(title, max_text_px);

        let text_x = x0 + tab_padding as f32;
        let text_y = top + (tab_h - cell_h as f32) / 2.0;
        self.push_text_instances(fg, &display_title, text_x, text_y, text_fg, glyphs, queue);

        // Close button — vector icon
        let close_btn_wf = close_btn_w as f32;
        let close_right_pad = CLOSE_BUTTON_RIGHT_PAD as f32 * s;
        let close_x = x0 + tab_w as f32 - close_btn_wf - close_right_pad;
        let close_hovered = params.hover_hit == TabBarHit::CloseTab(tab_idx);
        if close_hovered {
            let sq_y = top + (tab_h - close_btn_wf) / 2.0;
            bg.push_rect(
                close_x,
                sq_y,
                close_btn_wf,
                close_btn_wf,
                lighten(tc.bar_bg, 0.10),
            );
        }
        let close_fg = if close_hovered {
            tc.text_fg
        } else {
            tc.close_fg
        };
        let icon_cx = close_x + close_btn_wf / 2.0;
        let icon_cy = top + tab_h / 2.0;
        self.push_icon(
            fg,
            crate::icons::Icon::Close,
            icon_cx,
            icon_cy,
            10.0,
            s,
            close_fg,
            queue,
        );
    }

    #[cfg(target_os = "windows")]
    fn build_window_controls(
        &self,
        bg: &mut InstanceWriter,
        controls_start: f32,
        params: &FrameParams<'_>,
        tc: &TabBarColors,
    ) {
        let sc = params.scale;
        let btn_w = CONTROL_BUTTON_WIDTH as f32 * sc;
        let bar_h = TAB_BAR_HEIGHT as f32 * sc;
        let icon_sz = ICON_SIZE as f32 * sc;
        let line_t = 1.0 * sc; // line thickness

        // Minimize button (geometric horizontal line)
        {
            let btn_x = controls_start;
            let hovered = params.hover_hit == TabBarHit::Minimize;
            if hovered {
                bg.push_rect(btn_x, 0.0, btn_w, bar_h, tc.control_hover_bg);
            }
            let fg_color = if hovered {
                tc.control_fg
            } else {
                tc.control_fg_dim
            };
            let line_w: f32 = 10.0 * sc;
            let line_x = btn_x + (btn_w - line_w) / 2.0;
            let line_y = bar_h / 2.0;
            bg.push_rect(line_x, line_y, line_w, line_t, fg_color);
        }

        // Maximize/Restore button
        {
            let btn_x = controls_start + btn_w;
            let hovered = params.hover_hit == TabBarHit::Maximize;
            if hovered {
                bg.push_rect(btn_x, 0.0, btn_w, bar_h, tc.control_hover_bg);
            }
            let fg_color = if hovered {
                tc.control_fg
            } else {
                tc.control_fg_dim
            };
            let icon_x = btn_x + (btn_w - icon_sz) / 2.0;
            let icon_y = (bar_h - icon_sz) / 2.0;
            if params.is_maximized {
                let sm = icon_sz - 2.0 * sc;
                bg.push_rect(icon_x + 2.0 * sc, icon_y, sm, line_t, fg_color);
                bg.push_rect(
                    icon_x + 2.0 * sc,
                    icon_y + sm - line_t,
                    sm,
                    line_t,
                    fg_color,
                );
                bg.push_rect(icon_x + 2.0 * sc, icon_y, line_t, sm, fg_color);
                bg.push_rect(
                    icon_x + sm + 2.0 * sc - line_t,
                    icon_y,
                    line_t,
                    sm,
                    fg_color,
                );
                bg.push_rect(icon_x, icon_y + 2.0 * sc, sm, line_t, fg_color);
                bg.push_rect(
                    icon_x,
                    icon_y + sm + 2.0 * sc - line_t,
                    sm,
                    line_t,
                    fg_color,
                );
                bg.push_rect(icon_x, icon_y + 2.0 * sc, line_t, sm, fg_color);
                bg.push_rect(
                    icon_x + sm - line_t,
                    icon_y + 2.0 * sc,
                    line_t,
                    sm,
                    fg_color,
                );
                let inner_bg = if hovered {
                    tc.control_hover_bg
                } else {
                    tc.bar_bg
                };
                bg.push_rect(
                    icon_x + line_t,
                    icon_y + 3.0 * sc,
                    sm - 2.0 * line_t,
                    sm - 2.0 * line_t,
                    inner_bg,
                );
            } else {
                bg.push_rect(icon_x, icon_y, icon_sz, line_t, fg_color);
                bg.push_rect(icon_x, icon_y + icon_sz - line_t, icon_sz, line_t, fg_color);
                bg.push_rect(icon_x, icon_y, line_t, icon_sz, fg_color);
                bg.push_rect(icon_x + icon_sz - line_t, icon_y, line_t, icon_sz, fg_color);
            }
        }

        // Close button (Windows 11 style: geometric x drawn with small rects)
        {
            let btn_x = controls_start + btn_w * 2.0;
            let hovered = params.hover_hit == TabBarHit::CloseWindow;
            if hovered {
                bg.push_rect(btn_x, 0.0, btn_w, bar_h, CONTROL_CLOSE_HOVER_BG);
            }
            let close_fg = if hovered {
                CONTROL_CLOSE_HOVER_FG
            } else {
                tc.control_fg_dim
            };
            let x_size: f32 = 10.0 * sc;
            let cx = btn_x + (btn_w - x_size) / 2.0;
            let cy = (bar_h - x_size) / 2.0;
            let steps = (10.0 * sc) as usize;
            let step = x_size / steps as f32;
            for i in 0..steps {
                let fi = i as f32 * step;
                bg.push_rect(cx + fi, cy + fi, sc, sc, close_fg);
                bg.push_rect(cx + x_size - sc - fi, cy + fi, sc, sc, close_fg);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn build_window_controls(
        &self,
        bg: &mut InstanceWriter,
        controls_start: f32,
        params: &FrameParams<'_>,
        tc: &TabBarColors,
    ) {
        let sc = params.scale;
        let bar_h = TAB_BAR_HEIGHT as f32 * sc;
        let r = CONTROL_BUTTON_DIAMETER as f32 * sc / 2.0;
        let cy = bar_h / 2.0;
        let icon_s = ICON_SIZE as f32 * sc;
        let margin = CONTROL_BUTTON_MARGIN as f32 * sc;
        let diameter = CONTROL_BUTTON_DIAMETER as f32 * sc;
        let spacing = CONTROL_BUTTON_SPACING as f32 * sc;
        let line_t = 1.0 * sc;

        // Button center X positions: minimize, maximize, close (left to right)
        let btn_cx = [
            controls_start + margin + r,
            controls_start + margin + diameter + spacing + r,
            controls_start + margin + 2.0 * (diameter + spacing) + r,
        ];
        let hits = [
            TabBarHit::Minimize,
            TabBarHit::Maximize,
            TabBarHit::CloseWindow,
        ];

        for (i, &bcx) in btn_cx.iter().enumerate() {
            let hovered = params.hover_hit == hits[i];
            let is_close = i == 2;

            // Circle background (blended with bar bg for semi-transparency)
            let circle_fg = if hovered && is_close {
                CONTROL_CLOSE_HOVER_BG
            } else if hovered {
                tc.control_hover_bg
            } else {
                tc.control_fg_dim
            };
            let circle_bg = lerp_color(tc.bar_bg, circle_fg, CONTROL_CIRCLE_ALPHA);
            // Draw filled circle as horizontal slices
            let d = (CONTROL_BUTTON_DIAMETER as f32 * sc) as i32;
            for row in 0..d {
                let dy = row as f32 - r + 0.5;
                let half_w = (r * r - dy * dy).sqrt();
                let x0 = bcx - half_w;
                let w = half_w * 2.0;
                bg.push_rect(x0, cy - r + row as f32, w, 1.0, circle_bg);
            }

            // Icon foreground
            let fg_color = if hovered && is_close {
                CONTROL_CLOSE_HOVER_FG
            } else {
                tc.control_fg
            };
            let ix = bcx - icon_s / 2.0;
            let iy = cy - icon_s / 2.0;

            match i {
                0 => {
                    // Minimize: horizontal line
                    bg.push_rect(ix, cy, icon_s, line_t, fg_color);
                }
                1 => {
                    // Maximize/Restore
                    if params.is_maximized {
                        let sm = icon_s - 2.0 * sc;
                        // Back square
                        bg.push_rect(ix + 2.0 * sc, iy, sm, line_t, fg_color);
                        bg.push_rect(ix + 2.0 * sc, iy + sm - line_t, sm, line_t, fg_color);
                        bg.push_rect(ix + 2.0 * sc, iy, line_t, sm, fg_color);
                        bg.push_rect(ix + sm + 2.0 * sc - line_t, iy, line_t, sm, fg_color);
                        // Front square
                        bg.push_rect(ix, iy + 2.0 * sc, sm, line_t, fg_color);
                        bg.push_rect(ix, iy + sm + 2.0 * sc - line_t, sm, line_t, fg_color);
                        bg.push_rect(ix, iy + 2.0 * sc, line_t, sm, fg_color);
                        bg.push_rect(ix + sm - line_t, iy + 2.0 * sc, line_t, sm, fg_color);
                        bg.push_rect(
                            ix + line_t,
                            iy + 3.0 * sc,
                            sm - 2.0 * line_t,
                            sm - 2.0 * line_t,
                            circle_bg,
                        );
                    } else {
                        // Single square outline
                        bg.push_rect(ix, iy, icon_s, line_t, fg_color);
                        bg.push_rect(ix, iy + icon_s - line_t, icon_s, line_t, fg_color);
                        bg.push_rect(ix, iy, line_t, icon_s, fg_color);
                        bg.push_rect(ix + icon_s - line_t, iy, line_t, icon_s, fg_color);
                    }
                }
                _ => {
                    // Close: x drawn as 1px diagonal squares
                    let steps = (ICON_SIZE as f32 * sc) as usize;
                    let step = icon_s / steps as f32;
                    for j in 0..steps {
                        let fj = j as f32 * step;
                        bg.push_rect(ix + fj, iy + fj, sc, sc, fg_color);
                        bg.push_rect(ix + icon_s - sc - fj, iy + fj, sc, sc, fg_color);
                    }
                }
            }
        }
    }
}
