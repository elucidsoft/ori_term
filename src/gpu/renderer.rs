use std::sync::Arc;

use winit::window::Window;

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::palette::Palette;
use crate::render::{FontSet, FontStyle};
use crate::selection::Selection;
use crate::tab::TabId;
use crate::palette::BUILTIN_SCHEMES;
use crate::tab_bar::{
    TabBarHit, TabBarLayout, GRID_PADDING_LEFT, TAB_BAR_HEIGHT,
    WINDOW_BORDER_COLOR, WINDOW_BORDER_WIDTH, DROPDOWN_BUTTON_WIDTH,
    TAB_LEFT_MARGIN, NEW_TAB_BUTTON_WIDTH,
};
use crate::term_mode::TermMode;

use super::atlas::GlyphAtlas;
use super::pipeline::{self, INSTANCE_STRIDE};

const CONTROL_CLOSE_HOVER_BG: [f32; 4] = rgb_const(0xc4, 0x2b, 0x1c);
const CONTROL_CLOSE_HOVER_FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Tab bar colors derived dynamically from the palette's background.
struct TabBarColors {
    bar_bg: [f32; 4],
    inactive_bg: [f32; 4],
    hover_bg: [f32; 4],
    border: [f32; 4],
    text_fg: [f32; 4],
    inactive_text: [f32; 4],
    close_fg: [f32; 4],
    control_fg: [f32; 4],
    control_fg_dim: [f32; 4],
    control_hover_bg: [f32; 4],
}

impl TabBarColors {
    fn from_palette(palette: &Palette) -> Self {
        let base = vte_rgb_to_rgba(palette.default_bg());
        let fg = vte_rgb_to_rgba(palette.default_fg());

        let bar_bg = darken(base, 0.35);
        let inactive_bg = lighten(bar_bg, 0.15);
        let hover_bg = lighten(bar_bg, 0.20);
        let border = lighten(bar_bg, 0.08);
        let inactive_text = blend(fg, bar_bg, 0.6);
        let close_fg = inactive_text;
        let control_fg = fg;
        let control_fg_dim = blend(fg, bar_bg, 0.5);
        let control_hover_bg = lighten(bar_bg, 0.12);

        Self {
            bar_bg,
            inactive_bg,
            hover_bg,
            border,
            text_fg: fg,
            inactive_text,
            close_fg,
            control_fg,
            control_fg_dim,
            control_hover_bg,
        }
    }
}

/// Darken a color by a factor (0.0 = unchanged, 1.0 = black).
fn darken(c: [f32; 4], amount: f32) -> [f32; 4] {
    let f = 1.0 - amount;
    [c[0] * f, c[1] * f, c[2] * f, c[3]]
}

/// Lighten a color by a factor (0.0 = unchanged, 1.0 = white).
fn lighten(c: [f32; 4], amount: f32) -> [f32; 4] {
    [
        c[0] + (1.0 - c[0]) * amount,
        c[1] + (1.0 - c[1]) * amount,
        c[2] + (1.0 - c[2]) * amount,
        c[3],
    ]
}

/// Blend two colors: result = a * t + b * (1 - t).
fn blend(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] * t + b[0] * (1.0 - t),
        a[1] * t + b[1] * (1.0 - t),
        a[2] * t + b[2] * (1.0 - t),
        1.0,
    ]
}

const TAB_TOP_MARGIN: usize = 8;
const TAB_PADDING: usize = 8;
const CLOSE_BUTTON_WIDTH: usize = 24;
const CLOSE_BUTTON_RIGHT_PAD: usize = 8;
const CONTROL_BUTTON_WIDTH: usize = 58;
const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_WIDTH * 3;
const ICON_SIZE: usize = 10;

const fn rgb_const(r: u8, g: u8, b: u8) -> [f32; 4] {
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ]
}

/// Frame data needed to build a frame.
pub struct FrameParams<'a> {
    pub width: u32,
    pub height: u32,
    pub grid: &'a Grid,
    pub palette: &'a Palette,
    pub mode: TermMode,
    pub selection: Option<&'a Selection>,
    pub tab_info: &'a [(TabId, String)],
    pub active_tab: usize,
    pub hover_hit: TabBarHit,
    pub is_maximized: bool,
    pub dropdown_open: bool,
}

/// GPU state shared across all windows.
pub struct GpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_format: wgpu::TextureFormat,
    pub surface_alpha_mode: wgpu::CompositeAlphaMode,
}

impl GpuState {
    /// Initialize GPU: create instance, surface, adapter, device, queue.
    /// The initial window is needed to create a compatible surface for adapter selection.
    pub fn new(window: &Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create initial wgpu surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("failed to find GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("oriterm"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .expect("failed to create GPU device");

        let caps = surface.get_capabilities(&adapter);
        // Use a non-sRGB format so our sRGB color values pass through without
        // double gamma correction. Terminal colors are already in sRGB space.
        let surface_format = caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let surface_alpha_mode = caps.alpha_modes[0];

        // Configure the initial surface (it will be used by the first window)
        let size = window.inner_size();
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: surface_alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );

        crate::log(&format!(
            "GPU init: adapter={}, format={surface_format:?}",
            adapter.get_info().name,
        ));

        Self {
            instance,
            adapter,
            device,
            queue,
            surface_format,
            surface_alpha_mode,
        }
    }

    /// Create and configure a new surface for a window.
    pub fn create_surface(
        &self,
        window: &Arc<Window>,
    ) -> Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)> {
        let surface = self.instance.create_surface(window.clone()).ok()?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: self.surface_alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        Some((surface, config))
    }
}

/// The GPU renderer: owns pipelines, atlas, bind groups.
pub struct GpuRenderer {
    bg_pipeline: wgpu::RenderPipeline,
    fg_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    atlas: GlyphAtlas,
    atlas_bind_group: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl GpuRenderer {
    pub fn new(gpu: &GpuState, glyphs: &mut FontSet) -> Self {
        let device = &gpu.device;
        let format = gpu.surface_format;

        // Bind group layouts
        let uniform_layout = pipeline::create_uniform_bind_group_layout(device);
        let atlas_layout = pipeline::create_atlas_bind_group_layout(device);

        // Pipelines
        let bg_pipeline = pipeline::create_bg_pipeline(device, format, &uniform_layout);
        let fg_pipeline =
            pipeline::create_fg_pipeline(device, format, &uniform_layout, &atlas_layout);

        // Uniform buffer (64 bytes for mat4x4)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Glyph atlas
        let mut atlas = GlyphAtlas::new(device);
        atlas.precache_ascii(glyphs, &gpu.queue);

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Atlas bind group
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            bg_pipeline,
            fg_pipeline,
            uniform_buffer,
            uniform_bind_group,
            atlas,
            atlas_bind_group,
            atlas_layout,
            sampler,
        }
    }

    /// Rebuild the atlas after font size change.
    pub fn rebuild_atlas(&mut self, gpu: &GpuState, glyphs: &mut FontSet) {
        self.atlas = GlyphAtlas::new(&gpu.device);
        self.atlas.precache_ascii(glyphs, &gpu.queue);

        // Recreate atlas bind group with new texture view
        self.atlas_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(self.atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    /// Render a single clear frame to eliminate the white flash on startup.
    pub fn clear_surface(
        &self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        bg: [f32; 4],
    ) {
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            gpu.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(bg[0]),
                            g: f64::from(bg[1]),
                            b: f64::from(bg[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    /// Render a full frame to the given surface.
    #[allow(clippy::too_many_lines)]
    pub fn draw_frame(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
    ) {
        let w = params.width as f32;
        let h = params.height as f32;

        // Update projection matrix (orthographic: pixels → NDC)
        let projection = ortho_projection(w, h);
        gpu.queue
            .write_buffer(&self.uniform_buffer, 0, &projection);

        // Build instance data
        let mut bg = InstanceWriter::new();
        let mut fg = InstanceWriter::new();

        // Default background color
        let default_bg = palette_to_rgba(params.palette.default_bg());

        // 1. Tab bar
        self.build_tab_bar_instances(&mut bg, &mut fg, params, glyphs, &gpu.queue);

        // 2. Grid cells
        self.build_grid_instances(&mut bg, &mut fg, params, glyphs, &gpu.queue, &default_bg);

        // 3. Window border
        if !params.is_maximized {
            let border_color = u32_to_rgba(WINDOW_BORDER_COLOR);
            let bw = WINDOW_BORDER_WIDTH as f32;
            bg.push_rect(0.0, 0.0, w, bw, border_color);
            bg.push_rect(0.0, h - bw, w, bw, border_color);
            bg.push_rect(0.0, 0.0, bw, h, border_color);
            bg.push_rect(w - bw, 0.0, bw, h, border_color);
        }

        // 4. Dropdown overlay (separate buffers — drawn after main bg+fg)
        let mut overlay_bg = InstanceWriter::new();
        let mut overlay_fg = InstanceWriter::new();
        self.build_dropdown_overlay(
            &mut overlay_bg, &mut overlay_fg, params, glyphs, &gpu.queue,
        );

        // Upload instance buffers
        let bg_bytes = bg.as_bytes();
        let fg_bytes = fg.as_bytes();

        if bg_bytes.is_empty() && fg_bytes.is_empty() {
            return;
        }

        let bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bg_instances"),
            size: (bg_bytes.len() as u64).max(INSTANCE_STRIDE),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !bg_bytes.is_empty() {
            gpu.queue.write_buffer(&bg_buffer, 0, bg_bytes);
        }

        let fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fg_instances"),
            size: (fg_bytes.len() as u64).max(INSTANCE_STRIDE),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !fg_bytes.is_empty() {
            gpu.queue.write_buffer(&fg_buffer, 0, fg_bytes);
        }

        // Upload overlay buffers (only when dropdown is open)
        let overlay_bg_bytes = overlay_bg.as_bytes();
        let overlay_fg_bytes = overlay_fg.as_bytes();
        let has_overlay = !overlay_bg_bytes.is_empty() || !overlay_fg_bytes.is_empty();

        let overlay_bg_buffer;
        let overlay_fg_buffer;
        if has_overlay {
            overlay_bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay_bg"),
                size: (overlay_bg_bytes.len() as u64).max(INSTANCE_STRIDE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !overlay_bg_bytes.is_empty() {
                gpu.queue.write_buffer(&overlay_bg_buffer, 0, overlay_bg_bytes);
            }
            overlay_fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay_fg"),
                size: (overlay_fg_bytes.len() as u64).max(INSTANCE_STRIDE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !overlay_fg_bytes.is_empty() {
                gpu.queue.write_buffer(&overlay_fg_buffer, 0, overlay_fg_bytes);
            }
        } else {
            // Dummy — never used
            overlay_bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None, size: INSTANCE_STRIDE,
                usage: wgpu::BufferUsages::VERTEX, mapped_at_creation: false,
            });
            overlay_fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None, size: INSTANCE_STRIDE,
                usage: wgpu::BufferUsages::VERTEX, mapped_at_creation: false,
            });
        }

        // Get surface texture
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost) => {
                surface.configure(&gpu.device, config);
                return;
            }
            Err(e) => {
                crate::log(&format!("surface error: {e}"));
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(default_bg[0]),
                            g: f64::from(default_bg[1]),
                            b: f64::from(default_bg[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Background pass
            if bg.count() > 0 {
                rpass.set_pipeline(&self.bg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_vertex_buffer(0, bg_buffer.slice(..));
                rpass.draw(0..4, 0..bg.count());
            }

            // Foreground pass
            if fg.count() > 0 {
                rpass.set_pipeline(&self.fg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
                rpass.set_vertex_buffer(0, fg_buffer.slice(..));
                rpass.draw(0..4, 0..fg.count());
            }

            // Overlay pass (dropdown menu on top of everything)
            if has_overlay {
                if overlay_bg.count() > 0 {
                    rpass.set_pipeline(&self.bg_pipeline);
                    rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    rpass.set_vertex_buffer(0, overlay_bg_buffer.slice(..));
                    rpass.draw(0..4, 0..overlay_bg.count());
                }
                if overlay_fg.count() > 0 {
                    rpass.set_pipeline(&self.fg_pipeline);
                    rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
                    rpass.set_vertex_buffer(0, overlay_fg_buffer.slice(..));
                    rpass.draw(0..4, 0..overlay_fg.count());
                }
            }
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    // --- Instance building: Tab bar ---

    fn build_tab_bar_instances(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let tc = TabBarColors::from_palette(params.palette);
        let w = params.width as f32;
        let tab_bar_h = TAB_BAR_HEIGHT as f32;

        // Tab bar background
        bg.push_rect(0.0, 0.0, w, tab_bar_h, tc.bar_bg);

        let tab_count = params.tab_info.len();
        let layout = TabBarLayout::compute(tab_count, params.width as usize);
        let tab_w = layout.tab_width;

        let cell_w = glyphs.cell_width;
        let cell_h = glyphs.cell_height;

        for (i, (_id, title)) in params.tab_info.iter().enumerate() {
            let x0 = (TAB_LEFT_MARGIN + i * tab_w) as f32;
            let is_active = i == params.active_tab;
            let is_hovered = params.hover_hit == TabBarHit::Tab(i);

            let tab_bg = if is_active {
                vte_rgb_to_rgba(params.palette.default_bg())
            } else if is_hovered {
                tc.hover_bg
            } else {
                tc.inactive_bg
            };

            let text_fg = if is_active {
                tc.text_fg
            } else {
                tc.inactive_text
            };

            // Tab background rect
            let top = TAB_TOP_MARGIN as f32;
            let bot = tab_bar_h;
            bg.push_rect(x0, top, tab_w as f32, bot - top, tab_bg);

            // Tab right border
            let border_x = x0 + tab_w as f32 - 1.0;
            bg.push_rect(border_x, top + 2.0, 1.0, bot - top - 4.0, tc.border);

            // Tab title
            let max_text_chars =
                (tab_w - TAB_PADDING * 2 - CLOSE_BUTTON_WIDTH) / cell_w.max(1);
            let display_title: String = if title.len() > max_text_chars {
                let mut t: String = title.chars().take(max_text_chars.saturating_sub(1)).collect();
                t.push('\u{2026}');
                t
            } else {
                title.clone()
            };

            let text_x = x0 + TAB_PADDING as f32;
            let text_y = TAB_TOP_MARGIN as f32
                + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - cell_h) as f32 / 2.0;
            self.push_text_instances(
                fg, &display_title, text_x, text_y, text_fg, glyphs, queue,
            );

            // Close button "×"
            let close_x =
                x0 + (tab_w - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD) as f32;
            let close_hovered = params.hover_hit == TabBarHit::CloseTab(i);
            if close_hovered {
                let sq_y = TAB_TOP_MARGIN as f32
                    + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - CLOSE_BUTTON_WIDTH) as f32 / 2.0;
                bg.push_rect(
                    close_x,
                    sq_y,
                    CLOSE_BUTTON_WIDTH as f32,
                    CLOSE_BUTTON_WIDTH as f32,
                    lighten(tc.bar_bg, 0.10),
                );
            }
            let close_fg = if close_hovered { tc.text_fg } else { tc.close_fg };
            let close_text_x = close_x + (CLOSE_BUTTON_WIDTH as f32 - cell_w as f32) / 2.0;
            let close_text_y = TAB_TOP_MARGIN as f32
                + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - cell_h) as f32 / 2.0;
            self.push_text_instances(
                fg, "\u{00D7}", close_text_x, close_text_y, close_fg, glyphs, queue,
            );
        }

        // New tab "+" button
        let plus_x = (TAB_LEFT_MARGIN + tab_count * tab_w) as f32;
        let plus_hovered = params.hover_hit == TabBarHit::NewTab;
        let plus_bg = if plus_hovered { tc.hover_bg } else { tc.bar_bg };
        bg.push_rect(
            plus_x,
            TAB_TOP_MARGIN as f32,
            NEW_TAB_BUTTON_WIDTH as f32,
            tab_bar_h - TAB_TOP_MARGIN as f32,
            plus_bg,
        );
        let plus_text_x = plus_x + (NEW_TAB_BUTTON_WIDTH as f32 - cell_w as f32) / 2.0;
        let plus_text_y =
            TAB_TOP_MARGIN as f32 + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - cell_h) as f32 / 2.0;
        self.push_text_instances(fg, "+", plus_text_x, plus_text_y, tc.text_fg, glyphs, queue);

        // Dropdown "▾" button (right of the "+" button)
        let dropdown_x = plus_x + NEW_TAB_BUTTON_WIDTH as f32;
        let dropdown_hovered = params.hover_hit == TabBarHit::DropdownButton;
        let dropdown_bg = if dropdown_hovered || params.dropdown_open {
            tc.hover_bg
        } else {
            tc.bar_bg
        };
        bg.push_rect(
            dropdown_x,
            TAB_TOP_MARGIN as f32,
            DROPDOWN_BUTTON_WIDTH as f32,
            tab_bar_h - TAB_TOP_MARGIN as f32,
            dropdown_bg,
        );
        let dd_text_x = dropdown_x + (DROPDOWN_BUTTON_WIDTH as f32 - cell_w as f32) / 2.0;
        let dd_text_y =
            TAB_TOP_MARGIN as f32 + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - cell_h) as f32 / 2.0;
        self.push_text_instances(
            fg, "\u{25BE}", dd_text_x, dd_text_y, tc.text_fg, glyphs, queue,
        );

        // Window control buttons
        let controls_start = (params.width as usize).saturating_sub(CONTROLS_ZONE_WIDTH) as f32;
        self.build_window_controls(bg, controls_start, params, &tc);
    }

    fn build_window_controls(
        &self,
        bg: &mut InstanceWriter,
        controls_start: f32,
        params: &FrameParams<'_>,
        tc: &TabBarColors,
    ) {
        let btn_w = CONTROL_BUTTON_WIDTH as f32;
        let bar_h = TAB_BAR_HEIGHT as f32;

        // Minimize button (geometric horizontal line)
        {
            let btn_x = controls_start;
            let hovered = params.hover_hit == TabBarHit::Minimize;
            if hovered {
                bg.push_rect(btn_x, 0.0, btn_w, bar_h, tc.control_hover_bg);
            }
            let fg_color = if hovered { tc.control_fg } else { tc.control_fg_dim };
            let line_w: f32 = 10.0;
            let line_x = btn_x + (btn_w - line_w) / 2.0;
            let line_y = bar_h / 2.0;
            bg.push_rect(line_x, line_y, line_w, 1.0, fg_color);
        }

        // Maximize/Restore button
        {
            let btn_x = controls_start + btn_w;
            let hovered = params.hover_hit == TabBarHit::Maximize;
            if hovered {
                bg.push_rect(btn_x, 0.0, btn_w, bar_h, tc.control_hover_bg);
            }
            let fg_color = if hovered { tc.control_fg } else { tc.control_fg_dim };
            let icon_x = btn_x + (btn_w - ICON_SIZE as f32) / 2.0;
            let icon_y = (bar_h - ICON_SIZE as f32) / 2.0;
            let icon_s = ICON_SIZE as f32;
            if params.is_maximized {
                // Restore: two overlapping squares drawn as rectangles
                let s = icon_s - 2.0;
                // Back square (offset +2, 0)
                bg.push_rect(icon_x + 2.0, icon_y, s, 1.0, fg_color);
                bg.push_rect(icon_x + 2.0, icon_y + s - 1.0, s, 1.0, fg_color);
                bg.push_rect(icon_x + 2.0, icon_y, 1.0, s, fg_color);
                bg.push_rect(icon_x + s + 1.0, icon_y, 1.0, s, fg_color);
                // Front square (offset 0, +2)
                bg.push_rect(icon_x, icon_y + 2.0, s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y + s + 1.0, s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y + 2.0, 1.0, s, fg_color);
                bg.push_rect(icon_x + s - 1.0, icon_y + 2.0, 1.0, s, fg_color);
                // Fill front interior to cover back square lines
                let inner_bg = if hovered {
                    tc.control_hover_bg
                } else {
                    tc.bar_bg
                };
                bg.push_rect(icon_x + 1.0, icon_y + 3.0, s - 2.0, s - 2.0, inner_bg);
            } else {
                // Maximize: single square outline
                bg.push_rect(icon_x, icon_y, icon_s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y + icon_s - 1.0, icon_s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y, 1.0, icon_s, fg_color);
                bg.push_rect(icon_x + icon_s - 1.0, icon_y, 1.0, icon_s, fg_color);
            }
        }

        // Close button (Windows 11 style: geometric × drawn with small rects)
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
            // Draw × as two diagonal lines using 1px squares
            let x_size: f32 = 10.0;
            let cx = btn_x + (btn_w - x_size) / 2.0;
            let cy = (bar_h - x_size) / 2.0;
            for i in 0..10 {
                let fi = i as f32;
                // Top-left to bottom-right diagonal
                bg.push_rect(cx + fi, cy + fi, 1.0, 1.0, close_fg);
                // Top-right to bottom-left diagonal
                bg.push_rect(cx + x_size - 1.0 - fi, cy + fi, 1.0, 1.0, close_fg);
            }
        }
    }

    // --- Instance building: Dropdown overlay (rendered AFTER grid so it's on top) ---

    fn build_dropdown_overlay(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        if !params.dropdown_open {
            return;
        }

        let tc = TabBarColors::from_palette(params.palette);
        let tab_count = params.tab_info.len();
        let layout = TabBarLayout::compute(tab_count, params.width as usize);
        let tabs_end = TAB_LEFT_MARGIN + tab_count * layout.tab_width;
        let dropdown_x = (tabs_end + NEW_TAB_BUTTON_WIDTH) as f32;
        let tab_bar_h = TAB_BAR_HEIGHT as f32;
        let cell_h = glyphs.cell_height;

        let menu_x = dropdown_x;
        let menu_y = tab_bar_h;
        let menu_w: f32 = 140.0;
        let menu_h: f32 = 32.0;

        // Menu background (opaque)
        let menu_bg = lighten(tc.bar_bg, 0.10);
        bg.push_rect(menu_x, menu_y, menu_w, menu_h, menu_bg);

        // 1px border
        let border_c = lighten(tc.bar_bg, 0.25);
        bg.push_rect(menu_x, menu_y, menu_w, 1.0, border_c);
        bg.push_rect(menu_x, menu_y + menu_h - 1.0, menu_w, 1.0, border_c);
        bg.push_rect(menu_x, menu_y, 1.0, menu_h, border_c);
        bg.push_rect(menu_x + menu_w - 1.0, menu_y, 1.0, menu_h, border_c);

        // "Settings" text
        let item_x = menu_x + 12.0;
        let item_y = menu_y + (menu_h - cell_h as f32) / 2.0;
        self.push_text_instances(
            fg, "Settings", item_x, item_y, tc.text_fg, glyphs, queue,
        );
    }

    // --- Instance building: Grid ---

    #[allow(clippy::too_many_lines)]
    fn build_grid_instances(
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
        let x_offset = GRID_PADDING_LEFT;
        let y_offset = TAB_BAR_HEIGHT + 10;

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

                // Selection highlight
                let is_selected = params.selection.is_some_and(|sel| {
                    let abs_row = grid
                        .scrollback
                        .len()
                        .saturating_sub(grid.display_offset)
                        + line;
                    sel.contains(abs_row, col)
                });
                if is_selected {
                    std::mem::swap(&mut fg_rgb, &mut bg_rgb);
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

                // Cursor block
                let is_cursor = grid.display_offset == 0
                    && params.mode.contains(TermMode::SHOW_CURSOR)
                    && line == grid.cursor.row
                    && col == grid.cursor.col;
                if is_cursor {
                    let cursor_color = vte_rgb_to_rgba(palette.cursor_color());
                    bg.push_rect(x0, y0, cw as f32, ch as f32, cursor_color);
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
                            let phase =
                                (dx as f32 / draw_w) * std::f32::consts::TAU;
                            let offset = (phase.sin() * 2.0).round();
                            bg.push_rect(
                                x0 + dx as f32,
                                underline_y + offset,
                                1.0,
                                1.0,
                                ul_color,
                            );
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
                                bg.push_rect(
                                    x0 + dx as f32,
                                    underline_y,
                                    1.0,
                                    1.0,
                                    ul_color,
                                );
                            }
                        }
                    } else if cell.flags.contains(CellFlags::UNDERLINE) {
                        bg.push_rect(x0, underline_y, draw_w, 1.0, ul_color);
                    } else {
                        // No underline decoration
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

                let style = FontStyle::from_cell_flags(cell.flags);
                let entry =
                    self.atlas
                        .get_or_insert(cell.c, style, glyphs, queue);

                if entry.metrics.width == 0 || entry.metrics.height == 0 {
                    continue;
                }

                // Glyph position
                let gx = x0 + entry.metrics.xmin as f32;
                let gy = y0 + baseline as f32 - entry.metrics.height as f32
                    - entry.metrics.ymin as f32;

                // Use dark text under cursor for contrast
                let glyph_fg = if is_cursor {
                    *default_bg
                } else {
                    fg_rgba
                };

                fg.push_glyph(
                    gx,
                    gy,
                    entry.metrics.width as f32,
                    entry.metrics.height as f32,
                    entry.uv_pos,
                    entry.uv_size,
                    glyph_fg,
                );

                // Synthetic bold: render glyph again 1px to the right
                if synthetic_bold
                    && (style == FontStyle::Bold || style == FontStyle::BoldItalic)
                {
                    fg.push_glyph(
                        gx + 1.0,
                        gy,
                        entry.metrics.width as f32,
                        entry.metrics.height as f32,
                        entry.uv_pos,
                        entry.uv_size,
                        glyph_fg,
                    );
                }
            }
        }
    }

    // --- Settings window rendering ---

    /// Render the settings window frame.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_settings_frame(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        width: u32,
        height: u32,
        active_scheme: &str,
        palette: Option<&Palette>,
        glyphs: &mut FontSet,
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
            let bg_dark = [0.08, 0.08, 0.12, 1.0];
            let text = [0.8, 0.84, 0.96, 1.0];
            let hover = [0.18, 0.18, 0.25, 1.0];
            let brd = [0.3, 0.3, 0.4, 1.0];
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
        let cell_w = glyphs.cell_width;

        // Title "Theme"
        let title_y = (50.0 - cell_h as f32) / 2.0;
        self.push_text_instances(
            &mut fg, "Theme", 16.0, title_y, title_fg, glyphs, &gpu.queue,
        );

        // Close button "×" in top-right
        let close_x = w - 30.0 + (30.0 - cell_w as f32) / 2.0;
        let close_y = (30.0 - cell_h as f32) / 2.0;
        self.push_text_instances(
            &mut fg, "\u{00D7}", close_x, close_y, row_fg, glyphs, &gpu.queue,
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
                blend(row_fg, win_bg, 0.75)
            };
            self.push_text_instances(
                &mut fg, scheme.name, text_x, text_y, name_color, glyphs, &gpu.queue,
            );

            // Active indicator: checkmark
            if is_active {
                let check_x = w - 30.0;
                let check_y = y0 + (row_h - cell_h as f32) / 2.0;
                self.push_text_instances(
                    &mut fg, "\u{2713}", check_x, check_y, title_fg, glyphs, &gpu.queue,
                );
            }
        }

        // Submit render
        let bg_bytes = bg.as_bytes();
        let fg_bytes = fg.as_bytes();

        if bg_bytes.is_empty() && fg_bytes.is_empty() {
            return;
        }

        let bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("settings_bg"),
            size: (bg_bytes.len() as u64).max(INSTANCE_STRIDE),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !bg_bytes.is_empty() {
            gpu.queue.write_buffer(&bg_buffer, 0, bg_bytes);
        }

        let fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("settings_fg"),
            size: (fg_bytes.len() as u64).max(INSTANCE_STRIDE),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !fg_bytes.is_empty() {
            gpu.queue.write_buffer(&fg_buffer, 0, fg_bytes);
        }

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost) => {
                surface.configure(&gpu.device, config);
                return;
            }
            Err(e) => {
                crate::log(&format!("settings surface error: {e}"));
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("settings_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("settings_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(win_bg[0]),
                            g: f64::from(win_bg[1]),
                            b: f64::from(win_bg[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if bg.count() > 0 {
                rpass.set_pipeline(&self.bg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_vertex_buffer(0, bg_buffer.slice(..));
                rpass.draw(0..4, 0..bg.count());
            }

            if fg.count() > 0 {
                rpass.set_pipeline(&self.fg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
                rpass.set_vertex_buffer(0, fg_buffer.slice(..));
                rpass.draw(0..4, 0..fg.count());
            }
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    // --- Text rendering helpers ---

    /// Push glyph instances for a text string at the given pixel position.
    fn push_text_instances(
        &mut self,
        fg: &mut InstanceWriter,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let cw = glyphs.cell_width;
        let baseline = glyphs.baseline;
        let mut cx = x;

        for ch in text.chars() {
            let entry =
                self.atlas
                    .get_or_insert(ch, FontStyle::Regular, glyphs, queue);

            if entry.metrics.width > 0 && entry.metrics.height > 0 {
                let gx = cx + entry.metrics.xmin as f32;
                let gy = y + baseline as f32 - entry.metrics.height as f32
                    - entry.metrics.ymin as f32;

                fg.push_glyph(
                    gx,
                    gy,
                    entry.metrics.width as f32,
                    entry.metrics.height as f32,
                    entry.uv_pos,
                    entry.uv_size,
                    color,
                );
            }

            cx += cw as f32;
        }
    }
}

// --- Instance data serialization ---

/// Writes cell instance data to a byte buffer without unsafe code.
struct InstanceWriter {
    data: Vec<u8>,
}

impl InstanceWriter {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(4096),
        }
    }

    /// Push a colored background rectangle (no texture).
    fn push_rect(&mut self, x: f32, y: f32, w: f32, h: f32, bg_color: [f32; 4]) {
        self.push_raw(
            [x, y],
            [w, h],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            bg_color,
            0,
        );
    }

    /// Push a textured glyph quad (alpha-blended).
    fn push_glyph(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        fg_color: [f32; 4],
    ) {
        self.push_raw(
            [x, y],
            [w, h],
            uv_pos,
            uv_size,
            fg_color,
            [0.0, 0.0, 0.0, 0.0],
            1,
        );
    }

    /// Write a full 80-byte instance record.
    #[allow(clippy::too_many_arguments)]
    fn push_raw(
        &mut self,
        pos: [f32; 2],
        size: [f32; 2],
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
        flags: u32,
    ) {
        for &v in &pos {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &size {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &uv_pos {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &uv_size {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &fg_color {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &bg_color {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        self.data.extend_from_slice(&flags.to_ne_bytes());
        // 12 bytes padding to reach 80-byte stride
        self.data.extend_from_slice(&[0u8; 12]);
    }

    fn count(&self) -> u32 {
        (self.data.len() / INSTANCE_STRIDE as usize) as u32
    }

    fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

// --- Color conversion helpers ---

/// Convert VTE RGB to [f32; 4] RGBA (sRGB, alpha=1.0).
fn vte_rgb_to_rgba(rgb: vte::ansi::Rgb) -> [f32; 4] {
    [
        f32::from(rgb.r) / 255.0,
        f32::from(rgb.g) / 255.0,
        f32::from(rgb.b) / 255.0,
        1.0,
    ]
}

/// Convert palette `default_bg()` to RGBA.
fn palette_to_rgba(rgb: vte::ansi::Rgb) -> [f32; 4] {
    vte_rgb_to_rgba(rgb)
}

/// Convert a u32 color (0x00RRGGBB) to [f32; 4] RGBA.
fn u32_to_rgba(c: u32) -> [f32; 4] {
    [
        ((c >> 16) & 0xFF) as f32 / 255.0,
        ((c >> 8) & 0xFF) as f32 / 255.0,
        (c & 0xFF) as f32 / 255.0,
        1.0,
    ]
}

/// Build an orthographic projection matrix (pixels → NDC) as 64 bytes.
/// Maps (0,0)-(w,h) to (-1,1)-(1,-1), column-major for WGSL mat4x4.
fn ortho_projection(w: f32, h: f32) -> [u8; 64] {
    let proj: [f32; 16] = [
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        -1.0,
        1.0,
        0.0,
        1.0,
    ];

    let mut bytes = [0u8; 64];
    for (i, &v) in proj.iter().enumerate() {
        let b = v.to_ne_bytes();
        bytes[i * 4..i * 4 + 4].copy_from_slice(&b);
    }
    bytes
}
