//! Settings window rendering — theme picker with color swatches.

use super::color_util::{
    darken, lerp_color, lighten, ortho_projection, srgb_to_linear, vte_rgb_to_rgba,
};
use super::instance_writer::InstanceWriter;
use super::renderer::GpuRenderer;
use super::state::GpuState;
use crate::palette::{Palette, BUILTIN_SCHEMES};
use crate::render::FontSet;

impl GpuRenderer {
    /// Render the settings window frame.
    #[expect(
        clippy::too_many_arguments,
        reason = "Renderer function requires full context for drawing"
    )]
    pub fn draw_settings_frame(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        width: u32,
        height: u32,
        active_scheme: &str,
        palette: Option<&Palette>,
        glyphs: &mut FontSet, // UI font
    ) {
        let w = width as f32;
        let h = height as f32;

        // Update projection
        let projection = ortho_projection(w, h);
        gpu.queue.write_buffer(&self.uniform_buffer, 0, &projection);

        let mut bg = InstanceWriter::new();
        let mut fg = InstanceWriter::new();

        // Derive colors from palette or use defaults
        let (win_bg, title_fg, row_fg, row_hover, border_c) = if let Some(pal) = palette {
            let base = vte_rgb_to_rgba(pal.default_bg());
            let text = vte_rgb_to_rgba(pal.default_fg());
            let bg_dark = darken(base, 0.20);
            let hover = lighten(bg_dark, 0.15);
            let brd = lighten(bg_dark, 0.25);
            (bg_dark, text, text, hover, brd)
        } else {
            let s = srgb_to_linear;
            let bg_dark = [s(0.08), s(0.08), s(0.12), 1.0];
            let text = [s(0.8), s(0.84), s(0.96), 1.0];
            let hover = [s(0.18), s(0.18), s(0.25), 1.0];
            let brd = [s(0.3), s(0.3), s(0.4), 1.0];
            (bg_dark, text, text, hover, brd)
        };

        // Full background
        bg.push_rect(0.0, 0.0, w, h, win_bg);

        // 1px border
        bg.push_rect(0.0, 0.0, w, 1.0, border_c);
        bg.push_rect(0.0, h - 1.0, w, 1.0, border_c);
        bg.push_rect(0.0, 0.0, 1.0, h, border_c);
        bg.push_rect(w - 1.0, 0.0, 1.0, h, border_c);

        let cell_h = glyphs.cell_height;

        // Title "Theme"
        let title_y = (50.0 - cell_h as f32) / 2.0;
        self.push_text_instances(
            &mut fg, "Theme", 16.0, title_y, title_fg, glyphs, &gpu.queue,
        );

        // Close button — vector icon
        let close_cx = w - 30.0 + 15.0;
        let close_cy = 15.0;
        self.push_icon(
            &mut fg,
            crate::icons::Icon::Close,
            close_cx,
            close_cy,
            10.0,
            1.0,
            row_fg,
            &gpu.queue,
        );

        // Scheme rows
        let title_h: f32 = 50.0;
        let row_h: f32 = 40.0;

        for (i, scheme) in BUILTIN_SCHEMES.iter().enumerate() {
            let y0 = title_h + i as f32 * row_h;
            let is_active = scheme.name == active_scheme;

            if is_active {
                bg.push_rect(4.0, y0, w - 8.0, row_h, row_hover);
            }

            // Color preview swatch (small square of the scheme's bg)
            let swatch_color = vte_rgb_to_rgba(scheme.bg);
            let swatch_x: f32 = 16.0;
            let swatch_y = y0 + (row_h - 16.0) / 2.0;
            bg.push_rect(swatch_x, swatch_y, 16.0, 16.0, swatch_color);
            // Swatch border
            bg.push_rect(swatch_x, swatch_y, 16.0, 1.0, border_c);
            bg.push_rect(swatch_x, swatch_y + 15.0, 16.0, 1.0, border_c);
            bg.push_rect(swatch_x, swatch_y, 1.0, 16.0, border_c);
            bg.push_rect(swatch_x + 15.0, swatch_y, 1.0, 16.0, border_c);

            // Scheme name
            let text_x: f32 = 40.0;
            let text_y = y0 + (row_h - cell_h as f32) / 2.0;
            let name_color = if is_active {
                title_fg
            } else {
                lerp_color(win_bg, row_fg, 0.75)
            };
            self.push_text_instances(
                &mut fg,
                scheme.name,
                text_x,
                text_y,
                name_color,
                glyphs,
                &gpu.queue,
            );

            // Active indicator: checkmark icon
            if is_active {
                let check_cx = w - 30.0 + 5.0;
                let check_cy = y0 + row_h / 2.0;
                self.push_icon(
                    &mut fg,
                    crate::icons::Icon::Checkmark,
                    check_cx,
                    check_cy,
                    10.0,
                    1.0,
                    title_fg,
                    &gpu.queue,
                );
            }
        }

        // Submit render
        self.submit_simple_frame(gpu, surface, config, &bg, &fg, win_bg);
    }
}
