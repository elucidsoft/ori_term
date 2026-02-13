//! GPU renderer — owns pipelines, atlas, bind groups, and frame caching.
//!
//! Rendering sub-modules (`render_grid`, `render_tab_bar`, `render_overlay`,
//! `render_settings`) add domain-specific `impl GpuRenderer` blocks.

use winit::window::WindowId;

use vte::ansi::CursorShape;

use crate::config::AlphaBlending;
use crate::font::FontCollection;
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
use super::color_util::{ortho_projection, vte_rgb_to_rgba, TabBarColors};
use super::instance_writer::{reuse_or_create_buffer, InstanceWriter};
use super::pipeline;
use super::state::GpuState;

/// Frame data needed to build a frame.
#[expect(clippy::struct_excessive_bools, reason = "Frame params aggregate independent per-frame flags")]
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
    overlay_bg_buffer: Option<wgpu::Buffer>,
    overlay_bg_count: u32,
    overlay_fg_buffer: Option<wgpu::Buffer>,
    overlay_fg_count: u32,
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
    // Reusable byte buffers for instance building — avoids per-frame heap allocation.
    buf_bg: Vec<u8>,
    buf_fg: Vec<u8>,
    buf_overlay_bg: Vec<u8>,
    buf_overlay_fg: Vec<u8>,
    /// Column-to-shaped-glyph index map, reused across lines to avoid allocation.
    pub(super) col_glyph_map: Vec<Option<usize>>,
    /// Scratch buffer for shaped glyphs, reused across lines to avoid allocation.
    pub(super) shaped_scratch: Vec<crate::font::ShapedGlyph>,
    /// Scratch buffer for shaping runs, reused across lines to avoid allocation.
    pub(super) runs_scratch: Vec<crate::font::ShapingRun>,
}

impl GpuRenderer {
    pub fn new(gpu: &GpuState) -> Self {
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
        let atlas_bind_group =
            Self::create_atlas_bind_group(device, &atlas_layout, atlas.view(), &sampler);

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
            buf_bg: Vec::new(),
            buf_fg: Vec::new(),
            buf_overlay_bg: Vec::new(),
            buf_overlay_fg: Vec::new(),
            col_glyph_map: Vec::new(),
            shaped_scratch: Vec::new(),
            runs_scratch: Vec::new(),
        }
    }

    /// Rebuild the atlas after font size change.
    pub fn rebuild_atlas(&mut self, gpu: &GpuState) {
        self.atlas = GlyphAtlas::new(&gpu.device);
        self.cached_frame = None;
        self.atlas_bind_group = Self::create_atlas_bind_group(
            &gpu.device,
            &self.atlas_layout,
            self.atlas.view(),
            &self.sampler,
        );
    }

    /// Create the atlas bind group (texture view + sampler).
    fn create_atlas_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
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
    pub fn draw_frame(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
        params: &FrameParams<'_>,
        collection: &mut FontCollection,
        ui_glyphs: &mut FontSet,
    ) {
        self.prepare_frame(gpu, params, collection, ui_glyphs);
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
    #[expect(clippy::too_many_lines, reason = "Frame preparation has two distinct paths (cached overlay vs full rebuild) that share state")]
    fn prepare_frame(
        &mut self,
        gpu: &GpuState,
        params: &FrameParams<'_>,
        collection: &mut FontCollection,
        ui_glyphs: &mut FontSet,
    ) {
        // Advance atlas frame counter for LRU page tracking.
        self.atlas.begin_frame();

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

        // Compute tab bar colors once — used by tab bar, dragged tab, and search bar.
        let tc = TabBarColors::from_palette(params.palette);

        // Damage tracking: reuse cached frame (GPU buffers + all) when nothing changed.
        let needs_rebuild =
            params.grid_dirty || params.tab_bar_dirty || self.cached_frame.is_none();

        if !needs_rebuild {
            // Main frame cached. But dragged-tab overlay may need updating
            // (cursor moved during drag). Overlay is tiny (~10 instances) so
            // rebuilding it is cheap; the expensive part (grid + tab bar +
            // buffer creation) is skipped entirely.
            let needs_overlay = params.dragged_tab.is_some()
                || params.context_menu.is_some()
                || self
                    .cached_frame
                    .as_ref()
                    .is_some_and(|f| f.overlay_bg_buffer.is_some());
            if needs_overlay {
                // Build overlay instances (borrows &mut self for atlas access).
                // Reuse byte buffers from previous frame to avoid heap allocation.
                let mut overlay_bg_w =
                    InstanceWriter::from_buffer(std::mem::take(&mut self.buf_overlay_bg));
                let mut overlay_fg_w =
                    InstanceWriter::from_buffer(std::mem::take(&mut self.buf_overlay_fg));
                self.build_dragged_tab_overlay(
                    &mut overlay_bg_w,
                    &mut overlay_fg_w,
                    params,
                    &tc,
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
                        // Reuse existing GPU buffer if it fits, else create a new one.
                        cached.overlay_bg_buffer = Some(reuse_or_create_buffer(
                            &gpu.device,
                            &gpu.queue,
                            cached.overlay_bg_buffer.take(),
                            overlay_bg_bytes,
                            "overlay_bg",
                        ));
                        cached.overlay_fg_buffer = Some(reuse_or_create_buffer(
                            &gpu.device,
                            &gpu.queue,
                            cached.overlay_fg_buffer.take(),
                            overlay_fg_bytes,
                            "overlay_fg",
                        ));
                    } else {
                        cached.overlay_bg_buffer = None;
                        cached.overlay_fg_buffer = None;
                    }
                    cached.overlay_bg_count = overlay_bg_count;
                    cached.overlay_fg_count = overlay_fg_count;
                }

                // Reclaim byte buffers for next frame.
                self.buf_overlay_bg = overlay_bg_w.into_buffer();
                self.buf_overlay_fg = overlay_fg_w.into_buffer();
            }
            return;
        }

        // Extract old GPU buffers for reuse before building new instance data.
        let old = self.cached_frame.take();
        let (old_bg_buf, old_fg_buf, old_overlay_bg_buf, old_overlay_fg_buf) = match old {
            Some(f) => (
                Some(f.bg_buffer),
                Some(f.fg_buffer),
                f.overlay_bg_buffer,
                f.overlay_fg_buffer,
            ),
            None => (None, None, None, None),
        };

        // Build instance data — reuse byte buffers from previous frame.
        // Tab bar and window border are fully opaque (opacity=1.0).
        // Grid cells use the configured opacity for transparency.
        let mut bg = InstanceWriter::from_buffer(std::mem::take(&mut self.buf_bg));
        let mut fg = InstanceWriter::from_buffer(std::mem::take(&mut self.buf_fg));

        // Default background color
        let default_bg = vte_rgb_to_rgba(params.palette.default_bg());

        // 1. Tab bar (always fully opaque — individual tabs carry their own alpha)
        self.build_tab_bar_instances(&mut bg, &mut fg, params, &tc, ui_glyphs, &gpu.queue);

        // Switch to transparent for grid content
        bg.opacity = params.opacity;

        // 2. Grid cells (semi-transparent — glass shows through, shaped font)
        self.build_grid_instances(&mut bg, &mut fg, params, collection, &gpu.queue, &default_bg);

        // 3. Search bar overlay (at bottom of grid, UI font)
        self.build_search_bar_overlay(&mut bg, &mut fg, params, &tc, ui_glyphs, &gpu.queue);

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
        let mut overlay_bg_w =
            InstanceWriter::from_buffer(std::mem::take(&mut self.buf_overlay_bg));
        let mut overlay_fg_w =
            InstanceWriter::from_buffer(std::mem::take(&mut self.buf_overlay_fg));
        self.build_dragged_tab_overlay(
            &mut overlay_bg_w,
            &mut overlay_fg_w,
            params,
            &tc,
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
            self.buf_bg = bg.into_buffer();
            self.buf_fg = fg.into_buffer();
            self.buf_overlay_bg = overlay_bg_w.into_buffer();
            self.buf_overlay_fg = overlay_fg_w.into_buffer();
            return;
        }

        let bg_count = bg.count();
        let fg_count = fg.count();
        let overlay_bg_bytes = overlay_bg_w.as_bytes();
        let overlay_fg_bytes = overlay_fg_w.as_bytes();
        let overlay_bg_count = overlay_bg_w.count();
        let overlay_fg_count = overlay_fg_w.count();
        let has_overlay = !overlay_bg_bytes.is_empty() || !overlay_fg_bytes.is_empty();

        // Reuse GPU buffers from previous frame when capacity suffices.
        let bg_buffer =
            reuse_or_create_buffer(&gpu.device, &gpu.queue, old_bg_buf, bg_bytes, "bg_instances");
        let fg_buffer =
            reuse_or_create_buffer(&gpu.device, &gpu.queue, old_fg_buf, fg_bytes, "fg_instances");

        let (overlay_bg_buffer, overlay_fg_buffer) = if has_overlay {
            let bg_buf = reuse_or_create_buffer(
                &gpu.device,
                &gpu.queue,
                old_overlay_bg_buf,
                overlay_bg_bytes,
                "overlay_bg",
            );
            let fg_buf = reuse_or_create_buffer(
                &gpu.device,
                &gpu.queue,
                old_overlay_fg_buf,
                overlay_fg_bytes,
                "overlay_fg",
            );
            (Some(bg_buf), Some(fg_buf))
        } else {
            (None, None)
        };

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
        });

        // Reclaim byte buffers for next frame.
        self.buf_bg = bg.into_buffer();
        self.buf_fg = fg.into_buffer();
        self.buf_overlay_bg = overlay_bg_w.into_buffer();
        self.buf_overlay_fg = overlay_fg_w.into_buffer();
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
        if let Some(ref buf) = p.overlay_bg_buffer {
            if p.overlay_bg_count > 0 {
                rpass.set_pipeline(&self.bg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_vertex_buffer(0, buf.slice(..));
                rpass.draw(0..4, 0..p.overlay_bg_count);
            }
        }
        if let Some(ref buf) = p.overlay_fg_buffer {
            if p.overlay_fg_count > 0 {
                rpass.set_pipeline(&self.fg_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                rpass.set_bind_group(1, &self.atlas_bind_group, &[]);
                rpass.set_vertex_buffer(0, buf.slice(..));
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

        let bg_buffer =
            reuse_or_create_buffer(&gpu.device, &gpu.queue, None, bg_bytes, "simple_bg");
        let fg_buffer =
            reuse_or_create_buffer(&gpu.device, &gpu.queue, None, fg_bytes, "simple_fg");

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
                    entry.page,
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
            fg.push_glyph(x, y, w, h, entry.uv_pos, entry.uv_size, color, [0.0; 4], entry.page);
        }
    }
}

