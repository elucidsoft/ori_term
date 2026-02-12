//! GPU renderer — owns pipelines, atlas, bind groups, and frame caching.
//!
//! Rendering sub-modules (`render_grid`, `render_tab_bar`, `render_overlay`,
//! `render_settings`) add domain-specific `impl GpuRenderer` blocks.

use winit::window::WindowId;

use vte::ansi::CursorShape;

use crate::config::AlphaBlending;
use crate::grid::Grid;
use crate::palette::Palette;
use crate::render::{FontSet, FontStyle};
use crate::search::SearchState;
use crate::selection::Selection;
use crate::tab::TabId;
use crate::tab_bar::TabBarHit;
#[cfg(target_os = "windows")]
use crate::tab_bar::{WINDOW_BORDER_COLOR, WINDOW_BORDER_WIDTH};
use crate::term_mode::TermMode;

use super::atlas::GlyphAtlas;
#[cfg(target_os = "windows")]
use super::color_util::u32_to_rgba;
use super::color_util::{ortho_projection, palette_to_rgba};
use super::pipeline::{self, INSTANCE_STRIDE};
use super::state::GpuState;

/// Frame data needed to build a frame.
#[allow(clippy::struct_excessive_bools)]
pub struct FrameParams<'a> {
    pub width: u32,
    pub height: u32,
    pub grid: &'a Grid,
    pub palette: &'a Palette,
    pub mode: TermMode,
    pub cursor_shape: CursorShape,
    pub selection: Option<&'a Selection>,
    pub search: Option<&'a SearchState>,
    pub tab_info: &'a [(TabId, String)],
    pub active_tab: usize,
    pub hover_hit: TabBarHit,
    pub is_maximized: bool,
    pub context_menu: Option<&'a crate::context_menu::MenuOverlay>,
    pub opacity: f32,
    pub tab_bar_opacity: f32,
    pub hover_hyperlink: Option<&'a str>,
    pub hover_url_range: Option<&'a [(usize, usize, usize)]>,
    pub minimum_contrast: f32,
    pub alpha_blending: AlphaBlending,
    /// Dragged tab: (index, pixel X). Rendered at this X, drawn on top.
    pub dragged_tab: Option<(usize, f32)>,
    /// Per-tab X offsets for dodge animation.
    pub tab_offsets: &'a [f32],
    /// Per-tab bell badge: true for tabs that received a bell while inactive.
    pub bell_badges: &'a [bool],
    /// Sine pulse phase for bell badge animation (0.0-1.0).
    pub bell_phase: f32,
    /// Display scale factor for `HiDPI` (1.0 = normal, 2.0 = Retina).
    pub scale: f32,
    /// Whether the cursor should be visible (false when blink is in off phase).
    pub cursor_visible: bool,
    /// True when grid content changed and instances need rebuild.
    pub grid_dirty: bool,
    /// True when tab bar changed and instances need rebuild.
    pub tab_bar_dirty: bool,
    /// Window being rendered — used for per-window cache invalidation.
    pub window_id: WindowId,
    /// Chrome-style locked tab width (after close, tabs keep width until mouse leaves bar).
    pub tab_width_lock: Option<usize>,
}

/// Pre-built frame data ready for a render pass.
struct PreparedFrame {
    default_bg: [f32; 4],
    opacity: f32,
    bg_buffer: wgpu::Buffer,
    bg_count: u32,
    fg_buffer: wgpu::Buffer,
    fg_count: u32,
    overlay_bg_buffer: wgpu::Buffer,
    overlay_bg_count: u32,
    overlay_fg_buffer: wgpu::Buffer,
    overlay_fg_count: u32,
    has_overlay: bool,
}

/// The GPU renderer: owns pipelines, atlas, bind groups.
pub struct GpuRenderer {
    bg_pipeline: wgpu::RenderPipeline,
    fg_pipeline: wgpu::RenderPipeline,
    pub(super) uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    pub(super) atlas: GlyphAtlas,
    atlas_bind_group: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    render_format: wgpu::TextureFormat,
    // Damage tracking: cached prepared frame with GPU buffers from previous render.
    // When nothing changed, we skip instance building AND buffer creation entirely.
    cached_frame: Option<PreparedFrame>,
    /// Tracks which window was last rendered; invalidates cache on window switch.
    last_rendered_window: Option<WindowId>,
}

impl GpuRenderer {
    pub fn new(gpu: &GpuState, _glyphs: &mut FontSet, _ui_glyphs: &mut FontSet) -> Self {
        let device = &gpu.device;
        let format = gpu.render_format;

        // Bind group layouts
        let uniform_layout = pipeline::create_uniform_bind_group_layout(device);
        let atlas_layout = pipeline::create_atlas_bind_group_layout(device);

        // Pipelines (with pipeline cache for Vulkan shader reuse across sessions)
        let pcache = gpu.pipeline_cache.as_ref();
        let bg_pipeline = pipeline::create_bg_pipeline(device, format, &uniform_layout, pcache);
        let fg_pipeline =
            pipeline::create_fg_pipeline(device, format, &uniform_layout, &atlas_layout, pcache);

        // Uniform buffer (80 bytes: mat4x4 + flags + min_contrast + padding)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_buffer"),
            size: 80,
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

        // Glyph atlas (holds both grid and UI font glyphs, keyed by size)
        // Glyphs are rasterized on-demand via get_or_insert() during rendering.
        let atlas = GlyphAtlas::new(device);

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
            render_format: format,
            cached_frame: None,
            last_rendered_window: None,
        }
    }

    /// Rebuild the atlas after font size change.
    pub fn rebuild_atlas(
        &mut self,
        gpu: &GpuState,
        _glyphs: &mut FontSet,
        _ui_glyphs: &mut FontSet,
    ) {
        self.atlas = GlyphAtlas::new(&gpu.device);
        self.cached_frame = None;

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
    /// `opacity` premultiplies the clear color for transparent windows.
    pub fn clear_surface(
        &self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        bg: [f32; 4],
        opacity: f32,
    ) {
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.render_format),
            ..Default::default()
        });
        let mut encoder = gpu
            .device
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
                            r: f64::from(bg[0]) * f64::from(opacity),
                            g: f64::from(bg[1]) * f64::from(opacity),
                            b: f64::from(bg[2]) * f64::from(opacity),
                            a: f64::from(opacity),
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

    /// Render a full frame to the given surface (normal swapchain path).
    #[allow(clippy::too_many_lines)]
    pub fn draw_frame(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        ui_glyphs: &mut FontSet,
    ) {
        self.prepare_frame(gpu, params, glyphs, ui_glyphs);
        let Some(prepared) = self.cached_frame.as_ref() else {
            return;
        };

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

        self.encode_render_pass(&mut encoder, &view, prepared);

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    /// Build all instance data for a frame and cache the `PreparedFrame`.
    /// When nothing changed (no grid or tab bar dirty flags), the cached
    /// frame with its GPU buffers is reused — zero allocation, zero upload.
    #[allow(clippy::too_many_lines)]
    fn prepare_frame(
        &mut self,
        gpu: &GpuState,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        ui_glyphs: &mut FontSet,
    ) {
        // Invalidate cached frame when switching between windows — a single
        // cache shared across windows would cause stale tab bars.
        if self.last_rendered_window != Some(params.window_id) {
            self.cached_frame = None;
            self.last_rendered_window = Some(params.window_id);
        }

        let w = params.width as f32;
        let h = params.height as f32;

        // Update projection matrix (orthographic: pixels to NDC)
        let projection = ortho_projection(w, h);
        gpu.queue.write_buffer(&self.uniform_buffer, 0, &projection);

        // Write rendering flags and minimum contrast ratio
        let flags: u32 = u32::from(params.alpha_blending == AlphaBlending::LinearCorrected);
        gpu.queue
            .write_buffer(&self.uniform_buffer, 64, &flags.to_ne_bytes());
        gpu.queue.write_buffer(
            &self.uniform_buffer,
            68,
            &params.minimum_contrast.to_ne_bytes(),
        );

        // Damage tracking: reuse cached frame (GPU buffers + all) when nothing changed.
        let needs_rebuild =
            params.grid_dirty || params.tab_bar_dirty || self.cached_frame.is_none();

        if !needs_rebuild {
            // Main frame cached. But dragged-tab overlay may need updating
            // (cursor moved during drag). Overlay is tiny (~10 instances) so
            // rebuilding it is cheap; the expensive part (grid + tab bar +
            // buffer creation) is skipped entirely.
            let needs_overlay = params.dragged_tab.is_some()
                || self.cached_frame.as_ref().is_some_and(|f| f.has_overlay);
            if needs_overlay {
                // Build overlay instances (borrows &mut self for atlas access)
                let mut overlay_bg_w = InstanceWriter::new();
                let mut overlay_fg_w = InstanceWriter::new();
                self.build_dragged_tab_overlay(
                    &mut overlay_bg_w,
                    &mut overlay_fg_w,
                    params,
                    ui_glyphs,
                    &gpu.queue,
                );
                self.build_context_menu_overlay(
                    &mut overlay_bg_w,
                    &mut overlay_fg_w,
                    params,
                    ui_glyphs,
                    &gpu.queue,
                );

                let overlay_bg_bytes = overlay_bg_w.as_bytes();
                let overlay_fg_bytes = overlay_fg_w.as_bytes();
                let overlay_bg_count = overlay_bg_w.count();
                let overlay_fg_count = overlay_fg_w.count();
                let has_overlay = !overlay_bg_bytes.is_empty() || !overlay_fg_bytes.is_empty();

                if let Some(ref mut cached) = self.cached_frame {
                    if has_overlay {
                        let new_bg = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("overlay_bg"),
                            size: (overlay_bg_bytes.len() as u64).max(INSTANCE_STRIDE),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        if !overlay_bg_bytes.is_empty() {
                            gpu.queue.write_buffer(&new_bg, 0, overlay_bg_bytes);
                        }
                        let new_fg = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("overlay_fg"),
                            size: (overlay_fg_bytes.len() as u64).max(INSTANCE_STRIDE),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        if !overlay_fg_bytes.is_empty() {
                            gpu.queue.write_buffer(&new_fg, 0, overlay_fg_bytes);
                        }
                        cached.overlay_bg_buffer = new_bg;
                        cached.overlay_fg_buffer = new_fg;
                    }
                    cached.overlay_bg_count = overlay_bg_count;
                    cached.overlay_fg_count = overlay_fg_count;
                    cached.has_overlay = has_overlay;
                }
            }
            return;
        }

        // Build instance data.
        // Tab bar and window border are fully opaque (opacity=1.0).
        // Grid cells use the configured opacity for transparency.
        let mut bg = InstanceWriter::new();
        let mut fg = InstanceWriter::new();

        // Default background color
        let default_bg = palette_to_rgba(params.palette.default_bg());

        // 1. Tab bar (always fully opaque — individual tabs carry their own alpha)
        self.build_tab_bar_instances(&mut bg, &mut fg, params, ui_glyphs, &gpu.queue);

        // Switch to transparent for grid content
        bg.opacity = params.opacity;

        // 2. Grid cells (semi-transparent — glass shows through, grid font)
        self.build_grid_instances(&mut bg, &mut fg, params, glyphs, &gpu.queue, &default_bg);

        // 3. Search bar overlay (at bottom of grid, UI font)
        self.build_search_bar_overlay(&mut bg, &mut fg, params, ui_glyphs, &gpu.queue);

        // 4. Window border (opaque, Windows only — Linux WM draws its own)
        bg.opacity = 1.0;
        #[cfg(target_os = "windows")]
        if !params.is_maximized {
            let border_color = u32_to_rgba(WINDOW_BORDER_COLOR);
            let bw = WINDOW_BORDER_WIDTH as f32 * params.scale;
            bg.push_rect(0.0, 0.0, w, bw, border_color);
            bg.push_rect(0.0, h - bw, w, bw, border_color);
            bg.push_rect(0.0, 0.0, bw, h, border_color);
            bg.push_rect(w - bw, 0.0, bw, h, border_color);
        }

        // 5. Overlay pass (drawn after main bg+fg — dragged tab + dropdown)
        let mut overlay_bg_w = InstanceWriter::new();
        let mut overlay_fg_w = InstanceWriter::new();
        self.build_dragged_tab_overlay(
            &mut overlay_bg_w,
            &mut overlay_fg_w,
            params,
            ui_glyphs,
            &gpu.queue,
        );
        self.build_context_menu_overlay(
            &mut overlay_bg_w,
            &mut overlay_fg_w,
            params,
            ui_glyphs,
            &gpu.queue,
        );

        let bg_bytes = bg.as_bytes();
        let fg_bytes = fg.as_bytes();

        if bg_bytes.is_empty() && fg_bytes.is_empty() {
            self.cached_frame = None;
            return;
        }

        let bg_count = bg.count();
        let fg_count = fg.count();
        let overlay_bg_count = overlay_bg_w.count();
        let overlay_fg_count = overlay_fg_w.count();
        let has_overlay =
            !overlay_bg_w.as_bytes().is_empty() || !overlay_fg_w.as_bytes().is_empty();

        // Create and upload GPU buffers
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

        let overlay_bg_bytes = overlay_bg_w.as_bytes();
        let overlay_fg_bytes = overlay_fg_w.as_bytes();

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
                gpu.queue
                    .write_buffer(&overlay_bg_buffer, 0, overlay_bg_bytes);
            }
            overlay_fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay_fg"),
                size: (overlay_fg_bytes.len() as u64).max(INSTANCE_STRIDE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !overlay_fg_bytes.is_empty() {
                gpu.queue
                    .write_buffer(&overlay_fg_buffer, 0, overlay_fg_bytes);
            }
        } else {
            // Dummy — never used
            overlay_bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: INSTANCE_STRIDE,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
            overlay_fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: INSTANCE_STRIDE,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        }

        self.cached_frame = Some(PreparedFrame {
            default_bg,
            opacity: params.opacity,
            bg_buffer,
            bg_count,
            fg_buffer,
            fg_count,
            overlay_bg_buffer,
            overlay_bg_count,
            overlay_fg_buffer,
            overlay_fg_count,
            has_overlay,
        });
    }

    /// Issue the render pass draw calls to the given texture view.
    fn encode_render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        p: &PreparedFrame,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(p.default_bg[0]) * f64::from(p.opacity),
                        g: f64::from(p.default_bg[1]) * f64::from(p.opacity),
                        b: f64::from(p.default_bg[2]) * f64::from(p.opacity),
                        a: f64::from(p.opacity),
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
        if p.bg_count > 0 {
            rpass.set_pipeline(&self.bg_pipeline);
            rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
            rpass.set_vertex_buffer(0, p.bg_buffer.slice(..));
            rpass.draw(0..4, 0..p.bg_count);
        }

        // Foreground pass
        if p.fg_count > 0 {
            rpass.set_pipeline(&self.fg_pipeline);
            rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
            rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
            rpass.set_vertex_buffer(0, p.fg_buffer.slice(..));
            rpass.draw(0..4, 0..p.fg_count);
        }

        // Overlay pass (dropdown menu on top of everything)
        if p.has_overlay {
            if p.overlay_bg_count > 0 {
                rpass.set_pipeline(&self.bg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_vertex_buffer(0, p.overlay_bg_buffer.slice(..));
                rpass.draw(0..4, 0..p.overlay_bg_count);
            }
            if p.overlay_fg_count > 0 {
                rpass.set_pipeline(&self.fg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
                rpass.set_vertex_buffer(0, p.overlay_fg_buffer.slice(..));
                rpass.draw(0..4, 0..p.overlay_fg_count);
            }
        }
    }

    /// Submit a simple two-pass frame (bg + fg) to a surface.
    /// Used by `draw_settings_frame` and any other single-shot window rendering.
    pub(super) fn submit_simple_frame(
        &self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        bg: &InstanceWriter,
        fg: &InstanceWriter,
        clear_color: [f32; 4],
    ) {
        let bg_bytes = bg.as_bytes();
        let fg_bytes = fg.as_bytes();

        if bg_bytes.is_empty() && fg_bytes.is_empty() {
            return;
        }

        let bg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("simple_bg"),
            size: (bg_bytes.len() as u64).max(INSTANCE_STRIDE),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !bg_bytes.is_empty() {
            gpu.queue.write_buffer(&bg_buffer, 0, bg_bytes);
        }

        let fg_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("simple_fg"),
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
                label: Some("simple_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("simple_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear_color[0]),
                            g: f64::from(clear_color[1]),
                            b: f64::from(clear_color[2]),
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
    pub(super) fn push_text_instances(
        &mut self,
        fg: &mut InstanceWriter,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let baseline = glyphs.baseline;
        let mut cx = x;

        for ch in text.chars() {
            let entry = self
                .atlas
                .get_or_insert(ch, FontStyle::Regular, glyphs, queue);

            let advance = entry.metrics.advance_width.ceil();

            if entry.metrics.width > 0 && entry.metrics.height > 0 {
                // Round to pixel boundaries — with nearest-neighbor sampling,
                // sub-pixel positions cause jagged/distorted glyphs.
                let gx = (cx + entry.metrics.xmin as f32).round();
                let gy =
                    (y + baseline as f32 - entry.metrics.height as f32 - entry.metrics.ymin as f32)
                        .round();

                fg.push_glyph(
                    gx,
                    gy,
                    entry.metrics.width as f32,
                    entry.metrics.height as f32,
                    entry.uv_pos,
                    entry.uv_size,
                    color,
                    [0.0, 0.0, 0.0, 0.0],
                );
            }

            cx += advance;
        }
    }

    /// Push a vector icon centered at (cx, cy). `size` is in logical pixels.
    pub(super) fn push_icon(
        &mut self,
        fg: &mut InstanceWriter,
        icon: crate::icons::Icon,
        cx: f32,
        cy: f32,
        size: f32,
        scale: f32,
        color: [f32; 4],
        queue: &wgpu::Queue,
    ) {
        let px_size = (size * scale).round() as u16;
        if px_size == 0 {
            return;
        }
        let entry = self.atlas.get_or_insert_icon(icon, px_size, queue);
        if entry.metrics.width > 0 && entry.metrics.height > 0 {
            let w = entry.metrics.width as f32;
            let h = entry.metrics.height as f32;
            let x = (cx - w / 2.0).round();
            let y = (cy - h / 2.0).round();
            fg.push_glyph(x, y, w, h, entry.uv_pos, entry.uv_size, color, [0.0; 4]);
        }
    }
}

// --- Instance data serialization ---

/// Writes cell instance data to a byte buffer without unsafe code.
pub(super) struct InstanceWriter {
    data: Vec<u8>,
    pub(super) opacity: f32,
}

impl InstanceWriter {
    pub(super) fn new() -> Self {
        Self {
            data: Vec::with_capacity(4096),
            opacity: 1.0,
        }
    }

    /// Push a colored background rectangle (no texture, sharp corners).
    /// When opacity < 1.0, the color is premultiplied by opacity.
    pub(super) fn push_rect(&mut self, x: f32, y: f32, w: f32, h: f32, bg_color: [f32; 4]) {
        let color = if self.opacity < 1.0 {
            [
                bg_color[0] * self.opacity,
                bg_color[1] * self.opacity,
                bg_color[2] * self.opacity,
                bg_color[3] * self.opacity,
            ]
        } else {
            bg_color
        };
        self.push_raw(
            [x, y],
            [w, h],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            color,
            0,
            0.0,
        );
    }

    /// Push a colored background rectangle with rounded top corners.
    pub(super) fn push_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg_color: [f32; 4],
        radius: f32,
    ) {
        let color = if self.opacity < 1.0 {
            [
                bg_color[0] * self.opacity,
                bg_color[1] * self.opacity,
                bg_color[2] * self.opacity,
                bg_color[3] * self.opacity,
            ]
        } else {
            bg_color
        };
        self.push_raw(
            [x, y],
            [w, h],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            color,
            0,
            radius,
        );
    }

    /// Push a colored rectangle with all four corners rounded.
    pub(super) fn push_all_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg_color: [f32; 4],
        radius: f32,
    ) {
        let color = if self.opacity < 1.0 {
            [
                bg_color[0] * self.opacity,
                bg_color[1] * self.opacity,
                bg_color[2] * self.opacity,
                bg_color[3] * self.opacity,
            ]
        } else {
            bg_color
        };
        // Negative radius signals the shader to round all 4 corners.
        self.push_raw(
            [x, y],
            [w, h],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            color,
            0,
            -radius,
        );
    }

    /// Push a textured glyph quad (alpha-blended).
    /// `bg_color` is passed through to the shader for contrast/correction.
    pub(super) fn push_glyph(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
    ) {
        self.push_raw([x, y], [w, h], uv_pos, uv_size, fg_color, bg_color, 1, 0.0);
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
        corner_radius: f32,
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
        self.data.extend_from_slice(&corner_radius.to_ne_bytes());
        // 8 bytes padding to reach 80-byte stride
        self.data.extend_from_slice(&[0u8; 8]);
    }

    pub(super) fn count(&self) -> u32 {
        (self.data.len() / INSTANCE_STRIDE as usize) as u32
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}
