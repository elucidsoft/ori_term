//! Overlay instance building — dragged tab, context menu, and search bar.
//!
//! Overlays are rendered in a separate pass after the main bg+fg, so they
//! appear on top of all grid and tab bar content.

use crate::context_menu::MenuEntry;
use crate::render::FontSet;
use crate::tab_bar::{
    DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TAB_TOP_MARGIN,
    TabBarLayout,
};

use super::color_util::{
    lighten, TabBarColors, UI_BG, UI_BG_HOVER, UI_SEPARATOR, UI_TEXT, UI_TEXT_DIM,
};
use super::instance_writer::InstanceWriter;
use super::renderer::{FrameParams, GpuRenderer};

impl GpuRenderer {
    pub(super) fn build_dragged_tab_overlay(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        tc: &TabBarColors,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let (drag_idx, drag_x) = match params.dragged_tab {
            Some(d) => d,
            None => return,
        };
        let (_id, title) = match params.tab_info.get(drag_idx) {
            Some(t) => t,
            None => return,
        };

        let s = params.scale;
        let layout = TabBarLayout::compute(
            params.tab_info.len(),
            params.width as usize,
            s as f64,
            params.tab_width_lock,
        );
        let tab_w = layout.tab_width;
        let tab_wf = tab_w as f32;
        let cell_h = glyphs.cell_height;
        let top = TAB_TOP_MARGIN as f32 * s;
        let tab_h = TAB_BAR_HEIGHT as f32 * s - top;

        // Opaque backing rect (covers underlying tab text from main FG pass)
        bg.push_rect(drag_x, top, tab_wf, tab_h, tc.bar_bg);
        // Rounded tab shape on top
        bg.push_rounded_rect(drag_x, top, tab_wf, tab_h, tc.active_bg, 8.0 * s);

        // Tab content (text + close button)
        self.render_tab_content(
            bg, fg, drag_x, tab_w, cell_h, title, tc.text_fg, tc, drag_idx, params, glyphs, queue,
        );

        // + and dropdown buttons — rendered in overlay during drag so they
        // move with the dragged tab via the fast path.
        let left_margin = (TAB_LEFT_MARGIN as f32 * s) as usize;
        let tab_count = params.tab_info.len();
        let default_plus_x = (left_margin + tab_count * tab_w) as f32;
        let plus_x = default_plus_x.max(drag_x + tab_wf);

        let new_tab_w = NEW_TAB_BUTTON_WIDTH as f32 * s;
        let plus_bg = tc.bar_bg;
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

        let dropdown_w = DROPDOWN_BUTTON_WIDTH as f32 * s;
        let dropdown_x = plus_x + new_tab_w;
        let dropdown_bg = if params.context_menu.is_some() {
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

    pub(super) fn build_context_menu_overlay(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let menu = match params.context_menu {
            Some(m) => m,
            None => return,
        };

        let (mx, my) = menu.position;
        let mw = menu.width;
        let mh = menu.height;
        let cell_h = glyphs.cell_height as f32;

        let menu_bg = UI_BG;
        let hover_bg = UI_BG_HOVER;
        let separator_color = UI_SEPARATOR;
        let text_color = UI_TEXT;
        let text_dim = UI_TEXT_DIM;
        let check_color = text_color;

        // 1. Shadow (offset down-right, all corners rounded)
        let shadow_offset = 2.0 * menu.scale;
        let shadow_color = [0.0, 0.0, 0.0, 0.35];
        bg.push_all_rounded_rect(
            mx + shadow_offset,
            my + shadow_offset,
            mw,
            mh,
            shadow_color,
            menu.menu_radius(),
        );

        // 2. Menu background with all corners rounded
        bg.push_all_rounded_rect(mx, my, mw, mh, menu_bg, menu.menu_radius());

        // 3. Iterate entries and draw items
        let mut y = my + menu.menu_padding_y();
        for (i, entry) in menu.entries.iter().enumerate() {
            let item_h = entry.height() * menu.scale;

            match entry {
                MenuEntry::Item { label, .. } => {
                    // Hover highlight
                    if menu.hovered == Some(i) {
                        let inset = menu.item_hover_inset();
                        bg.push_all_rounded_rect(
                            mx + inset,
                            y,
                            mw - inset * 2.0,
                            item_h,
                            hover_bg,
                            menu.item_hover_radius(),
                        );
                    }
                    // Text
                    let tx = mx + menu.item_padding_x();
                    let ty = y + (item_h - cell_h) / 2.0;
                    self.push_text_instances(fg, label, tx, ty, text_color, glyphs, queue);
                }
                MenuEntry::Check { label, checked, .. } => {
                    // Hover highlight
                    if menu.hovered == Some(i) {
                        let inset = menu.item_hover_inset();
                        bg.push_all_rounded_rect(
                            mx + inset,
                            y,
                            mw - inset * 2.0,
                            item_h,
                            hover_bg,
                            menu.item_hover_radius(),
                        );
                    }
                    let tx = mx + menu.item_padding_x();
                    let ty = y + (item_h - cell_h) / 2.0;

                    // Checkmark (vector icon — no font fallback needed)
                    let icon_sz = crate::context_menu::CHECKMARK_ICON_SIZE * menu.scale;
                    let gap = crate::context_menu::CHECKMARK_GAP * menu.scale;
                    if *checked {
                        let icon_cx = tx + icon_sz / 2.0;
                        let icon_cy = y + item_h / 2.0;
                        self.push_icon(
                            fg,
                            crate::icons::Icon::Checkmark,
                            icon_cx,
                            icon_cy,
                            crate::context_menu::CHECKMARK_ICON_SIZE,
                            menu.scale,
                            check_color,
                            queue,
                        );
                    }
                    // Label (indented past checkmark space)
                    let label_x = tx + icon_sz + gap;
                    let color = if *checked { text_color } else { text_dim };
                    self.push_text_instances(fg, label, label_x, ty, color, glyphs, queue);
                }
                MenuEntry::Separator => {
                    let sep_y = y + (item_h - menu.separator_thickness()) / 2.0;
                    let sep_mx = menu.separator_margin_x();
                    bg.push_rect(
                        mx + sep_mx,
                        sep_y,
                        mw - sep_mx * 2.0,
                        menu.separator_thickness(),
                        separator_color,
                    );
                }
            }
            y += item_h;
        }
    }

    pub(super) fn build_search_bar_overlay(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        tc: &TabBarColors,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let search = match params.search {
            Some(s) => s,
            None => return,
        };

        let sc = params.scale;
        let w = params.width as f32;
        let h = params.height as f32;
        let cell_h = glyphs.cell_height;

        let bar_h = cell_h as f32 + 12.0 * sc; // cell height + padding
        let bar_y = h - bar_h;

        // Bar background
        bg.push_rect(0.0, bar_y, w, bar_h, tc.bar_bg);

        // Top border
        let border_c = lighten(tc.bar_bg, 0.25);
        bg.push_rect(0.0, bar_y, w, 1.0 * sc, border_c);

        let text_y = bar_y + (bar_h - cell_h as f32) / 2.0;

        // Search icon ">" prefix
        let prefix = "> ";
        let prefix_x = 8.0 * sc;
        self.push_text_instances(
            fg,
            prefix,
            prefix_x,
            text_y,
            tc.inactive_text,
            glyphs,
            queue,
        );

        // Query text
        let query_x = prefix_x + glyphs.text_advance(prefix);
        if !search.query.is_empty() {
            self.push_text_instances(
                fg,
                &search.query,
                query_x,
                text_y,
                tc.text_fg,
                glyphs,
                queue,
            );
        }

        // Cursor (blinking rect after query text)
        let cursor_x = query_x + glyphs.text_advance(&search.query);
        bg.push_rect(cursor_x, text_y, 2.0 * sc, cell_h as f32, tc.text_fg);

        // Match count on the right — avoid heap allocation by using a stack buffer.
        if search.matches.is_empty() {
            if !search.query.is_empty() {
                let text = "No matches";
                let count_w = glyphs.text_advance(text);
                let count_x = w - count_w - 12.0 * sc;
                self.push_text_instances(
                    fg, text, count_x, text_y, tc.inactive_text, glyphs, queue,
                );
            }
        } else {
            let mut buf = [0u8; 32];
            let len = {
                use std::io::Write;
                let mut cursor = &mut buf[..];
                let _ = write!(cursor, "{} of {}", search.focused + 1, search.matches.len());
                32 - cursor.len()
            };
            let text = std::str::from_utf8(&buf[..len]).unwrap_or("");
            let count_w = glyphs.text_advance(text);
            let count_x = w - count_w - 12.0 * sc;
            self.push_text_instances(
                fg, text, count_x, text_y, tc.inactive_text, glyphs, queue,
            );
        }
    }
}
