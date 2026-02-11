use std::sync::Arc;

use winit::window::Window;

use vte::ansi::CursorShape;

use crate::cell::CellFlags;
use crate::config::AlphaBlending;
use crate::grid::Grid;
use crate::palette::Palette;
use crate::render::{FontSet, FontStyle};
use crate::search::{MatchType, SearchState};
use crate::selection::Selection;
use crate::tab::TabId;
use crate::palette::BUILTIN_SCHEMES;
use crate::tab_bar::{
    TabBarHit, TabBarLayout, GRID_PADDING_LEFT, TAB_BAR_HEIGHT,
    DROPDOWN_BUTTON_WIDTH, TAB_LEFT_MARGIN, NEW_TAB_BUTTON_WIDTH,
};
#[cfg(target_os = "windows")]
use crate::tab_bar::{WINDOW_BORDER_COLOR, WINDOW_BORDER_WIDTH};
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
const CONTROLS_ZONE_WIDTH: usize =
    CONTROL_BUTTON_MARGIN + 3 * CONTROL_BUTTON_DIAMETER + 2 * CONTROL_BUTTON_SPACING + CONTROL_BUTTON_MARGIN;
#[cfg(not(target_os = "windows"))]
const ICON_SIZE: usize = 8;
#[cfg(not(target_os = "windows"))]
const CONTROL_CIRCLE_ALPHA: f32 = 0.3;

/// Convert an sRGB u8 triplet to linear RGBA at compile time (approximate).
/// Uses a simple gamma-2.2 power curve (close enough for UI constants).
const fn rgb_const(r: u8, g: u8, b: u8) -> [f32; 4] {
    // Approximate sRGB→linear via x^2.2.  We need const fn so we use a
    // piecewise quadratic approximation: (x/255)^2.2 ≈ pow22(x).
    // For exact conversion at runtime, see `srgb_to_linear`.
    const fn pow22(v: u8) -> f32 {
        let x = v as f32 / 255.0;
        // x^2.2 ≈ x^2 * x^0.2;  x^0.2 ≈ 1 - 0.8*(1-x) for x>0.04
        // Simpler: just use x*x as a rough gamma-2.0 approximation which
        // is close enough for compile-time UI element colors.
        x * x
    }
    [pow22(r), pow22(g), pow22(b), 1.0]
}

/// Frame data needed to build a frame.
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
    pub dropdown_open: bool,
    pub opacity: f32,
    pub tab_bar_opacity: f32,
    pub hover_hyperlink: Option<&'a str>,
    pub hover_url_range: Option<&'a [(usize, usize, usize)]>,
    pub minimum_contrast: f32,
    pub alpha_blending: AlphaBlending,
}

/// GPU state shared across all windows.
pub struct GpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// The native surface format (used for surface configuration).
    pub surface_format: wgpu::TextureFormat,
    /// The sRGB format used for render passes and pipelines.
    /// May differ from `surface_format` when the surface doesn't natively
    /// support sRGB (e.g. DX12 `DirectComposition`).
    pub render_format: wgpu::TextureFormat,
    pub surface_alpha_mode: wgpu::CompositeAlphaMode,
    /// Vulkan pipeline cache (compiled shaders cached to disk across sessions).
    pub pipeline_cache: Option<wgpu::PipelineCache>,
    pipeline_cache_path: Option<std::path::PathBuf>,
}

impl GpuState {
    /// Initialize GPU: create instance, surface, adapter, device, queue.
    /// When `transparent` is true on Windows, uses DX12 with `DirectComposition`
    /// (the only path that gives `PreMultiplied` alpha on Windows HWND swapchains).
    /// Otherwise prefers Vulkan (supports pipeline caching for faster subsequent launches).
    pub fn new(window: &Arc<Window>, transparent: bool) -> Self {
        #[cfg(not(target_os = "windows"))]
        let _ = transparent;
        // On Windows with transparency, DX12+DComp is the only path for PreMultiplied alpha
        #[cfg(target_os = "windows")]
        if transparent {
            if let Some(state) = Self::try_init(window, wgpu::Backends::DX12, true) {
                return state;
            }
            crate::log("DX12 DComp init failed, falling back to Vulkan");
        }

        // Prefer Vulkan — it supports pipeline caching (compiled shaders persisted to disk)
        if let Some(state) = Self::try_init(window, wgpu::Backends::VULKAN, false) {
            return state;
        }
        // Fall back to other primary backends (DX12, Metal)
        if let Some(state) = Self::try_init(window, wgpu::Backends::PRIMARY, false) {
            return state;
        }
        // Last resort: secondary backends (GL, etc.)
        Self::try_init(window, wgpu::Backends::SECONDARY, false)
            .expect("failed to initialize GPU with any backend")
    }

    fn try_init(window: &Arc<Window>, backends: wgpu::Backends, dcomp: bool) -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    presentation_system: if dcomp {
                        wgpu::Dx12SwapchainKind::DxgiFromVisual
                    } else {
                        wgpu::Dx12SwapchainKind::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).ok()?;

        // Use enumerate_adapters instead of the slow request_adapter.
        // Pick the first discrete GPU that supports our surface, falling
        // back to any compatible adapter.
        let mut adapter: Option<wgpu::Adapter> = None;
        let mut fallback: Option<wgpu::Adapter> = None;
        for a in pollster::block_on(instance.enumerate_adapters(backends)) {
            if !a.is_surface_supported(&surface) {
                continue;
            }
            let info = a.get_info();
            if info.device_type == wgpu::DeviceType::DiscreteGpu {
                adapter = Some(a);
                break;
            }
            if fallback.is_none() {
                fallback = Some(a);
            }
        }
        let adapter = adapter.or(fallback)?;

        // Request PIPELINE_CACHE if the adapter supports it (Vulkan only)
        let mut features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
            features |= wgpu::Features::PIPELINE_CACHE;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("oriterm"),
                required_features: features,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .map_err(|e| crate::log(&format!("GPU device request failed: {e}")))
        .ok()?;

        let caps = surface.get_capabilities(&adapter);
        // Pick the native surface format, then derive an sRGB render format.
        // Some backends (DX12 DirectComposition) only expose non-sRGB surface
        // formats.  We use `view_formats` + `add_srgb_suffix()` so the GPU
        // still performs gamma-aware blending in all cases.
        let surface_format = caps.formats[0];
        let render_format = surface_format.add_srgb_suffix();

        // view_formats lets us create an sRGB view of a non-sRGB surface
        let view_formats = if render_format == surface_format {
            vec![]
        } else {
            vec![render_format]
        };

        // Prefer a non-opaque alpha mode so the compositor can see our
        // transparent pixels and show blur/acrylic through them.
        let surface_alpha_mode = if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            caps.alpha_modes[0]
        };

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
                view_formats,
                desired_maximum_frame_latency: 2,
            },
        );

        let info = adapter.get_info();
        crate::log(&format!(
            "GPU init: adapter={}, backend={:?}, surface_format={surface_format:?}, \
             render_format={render_format:?}, \
             alpha_mode={surface_alpha_mode:?} (available: {:?})",
            info.name, info.backend, caps.alpha_modes,
        ));

        // Pipeline cache: Vulkan supports caching compiled shaders to disk.
        // On subsequent launches, this skips shader recompilation.
        let (pipeline_cache, pipeline_cache_path) =
            Self::load_pipeline_cache(&device, &info);

        Some(Self {
            instance,
            adapter,
            device,
            queue,
            surface_format,
            render_format,
            surface_alpha_mode,
            pipeline_cache,
            pipeline_cache_path,
        })
    }

    /// Load a pipeline cache from disk (Vulkan only).
    /// Returns `(None, None)` on non-Vulkan backends.
    ///
    /// Safety: `create_pipeline_cache` is unsafe because it accepts arbitrary bytes.
    /// If the data is corrupt or from a different driver version, Vulkan silently
    /// ignores it and starts with an empty cache (we set `fallback: true`).
    #[allow(unsafe_code)]
    fn load_pipeline_cache(
        device: &wgpu::Device,
        adapter_info: &wgpu::AdapterInfo,
    ) -> (Option<wgpu::PipelineCache>, Option<std::path::PathBuf>) {
        let cache_key = match wgpu::util::pipeline_cache_key(adapter_info) {
            Some(key) if device.features().contains(wgpu::Features::PIPELINE_CACHE) => key,
            _ => return (None, None),
        };
        let cache_dir = crate::config::config_dir();
        let cache_path = cache_dir.join(cache_key);
        let cache_data = std::fs::read(&cache_path).ok();

        // Safety: cache data came from a previous `get_data()` call on the same adapter.
        // If the data is corrupt or from a different driver, wgpu/Vulkan silently ignores it.
        let cache = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("oriterm_pipeline_cache"),
                data: cache_data.as_deref(),
                fallback: true,
            })
        };

        crate::log(&format!(
            "pipeline cache: loaded from {} ({})",
            cache_path.display(),
            if cache_data.is_some() { "existing" } else { "new" },
        ));

        (Some(cache), Some(cache_path))
    }

    /// Save the pipeline cache to disk. Call before exit.
    pub fn save_pipeline_cache(&self) {
        let (Some(cache), Some(path)) = (&self.pipeline_cache, &self.pipeline_cache_path) else {
            return;
        };
        let Some(data) = cache.get_data() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Atomic write: write to temp, then rename
        let temp = path.with_extension("tmp");
        if std::fs::write(&temp, &data).is_ok() {
            let _ = std::fs::rename(&temp, path);
            crate::log(&format!("pipeline cache: saved {} bytes to {}", data.len(), path.display()));
        }
    }

    /// Create and configure a new surface for a window.
    pub fn create_surface(
        &self,
        window: &Arc<Window>,
    ) -> Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)> {
        let surface = self.instance.create_surface(window.clone()).ok()?;
        let size = window.inner_size();
        let view_formats = if self.render_format == self.surface_format {
            vec![]
        } else {
            vec![self.render_format]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: self.surface_alpha_mode,
            view_formats,
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        Some((surface, config))
    }
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
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    atlas: GlyphAtlas,
    atlas_bind_group: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    render_format: wgpu::TextureFormat,
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
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
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
        }
    }

    /// Rebuild the atlas after font size change.
    pub fn rebuild_atlas(&mut self, gpu: &GpuState, _glyphs: &mut FontSet, _ui_glyphs: &mut FontSet) {
        self.atlas = GlyphAtlas::new(&gpu.device);

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
        let prepared = self.prepare_frame(gpu, params, glyphs, ui_glyphs);
        let Some(prepared) = prepared else { return };

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

        self.encode_render_pass(&mut encoder, &view, &prepared);

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    /// Build all instance data for a frame. Returns `None` if nothing to draw.
    #[allow(clippy::too_many_lines)]
    fn prepare_frame(
        &mut self,
        gpu: &GpuState,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        ui_glyphs: &mut FontSet,
    ) -> Option<PreparedFrame> {
        let w = params.width as f32;
        let h = params.height as f32;

        // Update projection matrix (orthographic: pixels → NDC)
        let projection = ortho_projection(w, h);
        gpu.queue
            .write_buffer(&self.uniform_buffer, 0, &projection);

        // Write rendering flags and minimum contrast ratio
        let flags: u32 = u32::from(params.alpha_blending == AlphaBlending::LinearCorrected);
        gpu.queue.write_buffer(&self.uniform_buffer, 64, &flags.to_ne_bytes());
        gpu.queue.write_buffer(&self.uniform_buffer, 68, &params.minimum_contrast.to_ne_bytes());

        // Build instance data.
        // Tab bar and window border are fully opaque (opacity=1.0).
        // Grid cells use the configured opacity for transparency.
        let mut bg = InstanceWriter::new();
        let mut fg = InstanceWriter::new();

        // Default background color
        let default_bg = palette_to_rgba(params.palette.default_bg());

        // 1. Tab bar (uses configured tab_bar_opacity, UI font)
        bg.opacity = params.tab_bar_opacity;
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
            let bw = WINDOW_BORDER_WIDTH as f32;
            bg.push_rect(0.0, 0.0, w, bw, border_color);
            bg.push_rect(0.0, h - bw, w, bw, border_color);
            bg.push_rect(0.0, 0.0, bw, h, border_color);
            bg.push_rect(w - bw, 0.0, bw, h, border_color);
        }

        // 5. Dropdown overlay (separate buffers — drawn after main bg+fg, opaque)
        let mut overlay_bg = InstanceWriter::new();
        let mut overlay_fg = InstanceWriter::new();
        self.build_dropdown_overlay(
            &mut overlay_bg, &mut overlay_fg, params, ui_glyphs, &gpu.queue,
        );

        // Upload instance buffers
        let bg_bytes = bg.as_bytes();
        let fg_bytes = fg.as_bytes();

        if bg_bytes.is_empty() && fg_bytes.is_empty() {
            return None;
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

        Some(PreparedFrame {
            default_bg,
            opacity: params.opacity,
            bg_buffer, bg_count: bg.count(),
            fg_buffer, fg_count: fg.count(),
            overlay_bg_buffer, overlay_bg_count: overlay_bg.count(),
            overlay_fg_buffer, overlay_fg_count: overlay_fg.count(),
            has_overlay,
        })
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

    #[cfg(target_os = "windows")]
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
                let s = icon_s - 2.0;
                bg.push_rect(icon_x + 2.0, icon_y, s, 1.0, fg_color);
                bg.push_rect(icon_x + 2.0, icon_y + s - 1.0, s, 1.0, fg_color);
                bg.push_rect(icon_x + 2.0, icon_y, 1.0, s, fg_color);
                bg.push_rect(icon_x + s + 1.0, icon_y, 1.0, s, fg_color);
                bg.push_rect(icon_x, icon_y + 2.0, s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y + s + 1.0, s, 1.0, fg_color);
                bg.push_rect(icon_x, icon_y + 2.0, 1.0, s, fg_color);
                bg.push_rect(icon_x + s - 1.0, icon_y + 2.0, 1.0, s, fg_color);
                let inner_bg = if hovered { tc.control_hover_bg } else { tc.bar_bg };
                bg.push_rect(icon_x + 1.0, icon_y + 3.0, s - 2.0, s - 2.0, inner_bg);
            } else {
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
            let x_size: f32 = 10.0;
            let cx = btn_x + (btn_w - x_size) / 2.0;
            let cy = (bar_h - x_size) / 2.0;
            for i in 0..10 {
                let fi = i as f32;
                bg.push_rect(cx + fi, cy + fi, 1.0, 1.0, close_fg);
                bg.push_rect(cx + x_size - 1.0 - fi, cy + fi, 1.0, 1.0, close_fg);
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
        let bar_h = TAB_BAR_HEIGHT as f32;
        let r = CONTROL_BUTTON_DIAMETER as f32 / 2.0;
        let cy = bar_h / 2.0;
        let icon_s = ICON_SIZE as f32;

        // Button center X positions: minimize, maximize, close (left to right)
        let btn_cx = [
            controls_start + CONTROL_BUTTON_MARGIN as f32 + r,
            controls_start + CONTROL_BUTTON_MARGIN as f32 + CONTROL_BUTTON_DIAMETER as f32
                + CONTROL_BUTTON_SPACING as f32 + r,
            controls_start + CONTROL_BUTTON_MARGIN as f32
                + 2.0 * (CONTROL_BUTTON_DIAMETER as f32 + CONTROL_BUTTON_SPACING as f32) + r,
        ];
        let hits = [TabBarHit::Minimize, TabBarHit::Maximize, TabBarHit::CloseWindow];

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
            let circle_bg = blend(circle_fg, tc.bar_bg, CONTROL_CIRCLE_ALPHA);
            // Draw filled circle as horizontal slices
            let d = CONTROL_BUTTON_DIAMETER as i32;
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
                    bg.push_rect(ix, cy, icon_s, 1.0, fg_color);
                }
                1 => {
                    // Maximize/Restore
                    if params.is_maximized {
                        let s = icon_s - 2.0;
                        // Back square
                        bg.push_rect(ix + 2.0, iy, s, 1.0, fg_color);
                        bg.push_rect(ix + 2.0, iy + s - 1.0, s, 1.0, fg_color);
                        bg.push_rect(ix + 2.0, iy, 1.0, s, fg_color);
                        bg.push_rect(ix + s + 1.0, iy, 1.0, s, fg_color);
                        // Front square
                        bg.push_rect(ix, iy + 2.0, s, 1.0, fg_color);
                        bg.push_rect(ix, iy + s + 1.0, s, 1.0, fg_color);
                        bg.push_rect(ix, iy + 2.0, 1.0, s, fg_color);
                        bg.push_rect(ix + s - 1.0, iy + 2.0, 1.0, s, fg_color);
                        bg.push_rect(ix + 1.0, iy + 3.0, s - 2.0, s - 2.0, circle_bg);
                    } else {
                        // Single square outline
                        bg.push_rect(ix, iy, icon_s, 1.0, fg_color);
                        bg.push_rect(ix, iy + icon_s - 1.0, icon_s, 1.0, fg_color);
                        bg.push_rect(ix, iy, 1.0, icon_s, fg_color);
                        bg.push_rect(ix + icon_s - 1.0, iy, 1.0, icon_s, fg_color);
                    }
                }
                _ => {
                    // Close: × drawn as 1px diagonal squares
                    for j in 0..ICON_SIZE {
                        let fj = j as f32;
                        bg.push_rect(ix + fj, iy + fj, 1.0, 1.0, fg_color);
                        bg.push_rect(ix + icon_s - 1.0 - fj, iy + fj, 1.0, 1.0, fg_color);
                    }
                }
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

    // --- Instance building: Search bar ---

    fn build_search_bar_overlay(
        &mut self,
        bg: &mut InstanceWriter,
        fg: &mut InstanceWriter,
        params: &FrameParams<'_>,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) {
        let search = match params.search {
            Some(s) => s,
            None => return,
        };

        let w = params.width as f32;
        let h = params.height as f32;
        let cell_h = glyphs.cell_height;
        let cell_w = glyphs.cell_width;

        let bar_h = cell_h as f32 + 12.0; // cell height + padding
        let bar_y = h - bar_h;

        let tc = TabBarColors::from_palette(params.palette);

        // Bar background
        bg.push_rect(0.0, bar_y, w, bar_h, tc.bar_bg);

        // Top border
        let border_c = lighten(tc.bar_bg, 0.25);
        bg.push_rect(0.0, bar_y, w, 1.0, border_c);

        let text_y = bar_y + (bar_h - cell_h as f32) / 2.0;

        // Search icon ">" prefix
        let prefix = "> ";
        let prefix_x = 8.0;
        self.push_text_instances(fg, prefix, prefix_x, text_y, tc.inactive_text, glyphs, queue);

        // Query text
        let query_x = prefix_x + (prefix.len() * cell_w) as f32;
        if !search.query.is_empty() {
            self.push_text_instances(fg, &search.query, query_x, text_y, tc.text_fg, glyphs, queue);
        }

        // Cursor (blinking rect after query text)
        let cursor_x = query_x + (search.query.chars().count() * cell_w) as f32;
        bg.push_rect(cursor_x, text_y, 2.0, cell_h as f32, tc.text_fg);

        // Match count on the right
        let count_text = if search.matches.is_empty() {
            if search.query.is_empty() {
                String::new()
            } else {
                "No matches".to_owned()
            }
        } else {
            format!("{} of {}", search.focused + 1, search.matches.len())
        };

        if !count_text.is_empty() {
            let count_w = (count_text.len() * cell_w) as f32;
            let count_x = w - count_w - 12.0;
            self.push_text_instances(fg, &count_text, count_x, text_y, tc.inactive_text, glyphs, queue);
        }
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

                // Compute absolute row for search/selection
                let abs_row = grid
                    .scrollback
                    .len()
                    .saturating_sub(grid.display_offset)
                    + line;

                // Search match highlighting
                if let Some(search) = params.search {
                    match search.cell_match_type(abs_row, col) {
                        MatchType::FocusedMatch => {
                            bg_rgb = vte::ansi::Rgb { r: 200, g: 120, b: 30 };
                            fg_rgb = vte::ansi::Rgb { r: 0, g: 0, b: 0 };
                        }
                        MatchType::Match => {
                            bg_rgb = vte::ansi::Rgb { r: 80, g: 80, b: 20 };
                        }
                        MatchType::None => {}
                    }
                }

                // Selection highlight
                let is_selected = params.selection.is_some_and(|sel| {
                    sel.contains(abs_row, col)
                });
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
                if is_cursor {
                    let cursor_color = vte_rgb_to_rgba(palette.cursor_color());
                    match params.cursor_shape {
                        CursorShape::Beam => {
                            // 2px vertical bar at left edge
                            bg.push_rect(x0, y0, 2.0, ch as f32, cursor_color);
                        }
                        CursorShape::Underline => {
                            // 2px horizontal bar at bottom
                            bg.push_rect(x0, y0 + ch as f32 - 2.0, cw as f32, 2.0, cursor_color);
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

                // Hyperlink underline (only when cell doesn't already have an underline)
                if cell.hyperlink().is_some()
                    && !cell.flags.intersects(CellFlags::ANY_UNDERLINE)
                {
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
                    let in_url = segments.iter().any(|&(r, sc, ec)| {
                        abs_row == r && col >= sc && col <= ec
                    });
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

                // Only invert text color for block cursor (beam/underline don't cover the glyph)
                let is_block_cursor = is_cursor && matches!(params.cursor_shape, CursorShape::Block);
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
                        glyph_bg,
                    );
                }

                // Overlay combining marks (zerowidth characters stored in CellExtra)
                for &zw in cell.zerowidth() {
                    let zw_entry =
                        self.atlas
                            .get_or_insert(zw, style, glyphs, queue);
                    if zw_entry.metrics.width == 0 || zw_entry.metrics.height == 0 {
                        continue;
                    }
                    let zx = x0 + zw_entry.metrics.xmin as f32;
                    let zy = y0 + baseline as f32 - zw_entry.metrics.height as f32
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
                    [0.0, 0.0, 0.0, 0.0],
                );
            }

            cx += cw as f32;
        }
    }
}

// --- Block character rendering ---

/// Draw a Unicode block element (U+2580–U+259F) as pixel-perfect rectangles.
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
        // ▀ Upper half block
        '\u{2580}' => {
            bg.push_rect(x, y, w, (h / 2.0).round(), fg);
        }
        // ▁ Lower 1/8
        '\u{2581}' => {
            let bh = (h / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▂ Lower 1/4
        '\u{2582}' => {
            let bh = (h / 4.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▃ Lower 3/8
        '\u{2583}' => {
            let bh = (h * 3.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▄ Lower half
        '\u{2584}' => {
            let bh = (h / 2.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▅ Lower 5/8
        '\u{2585}' => {
            let bh = (h * 5.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▆ Lower 3/4
        '\u{2586}' => {
            let bh = (h * 3.0 / 4.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // ▇ Lower 7/8
        '\u{2587}' => {
            let bh = (h * 7.0 / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // █ Full block
        '\u{2588}' => {
            bg.push_rect(x, y, w, h, fg);
        }
        // ▉ Left 7/8
        '\u{2589}' => {
            bg.push_rect(x, y, (w * 7.0 / 8.0).round(), h, fg);
        }
        // ▊ Left 3/4
        '\u{258A}' => {
            bg.push_rect(x, y, (w * 3.0 / 4.0).round(), h, fg);
        }
        // ▋ Left 5/8
        '\u{258B}' => {
            bg.push_rect(x, y, (w * 5.0 / 8.0).round(), h, fg);
        }
        // ▌ Left half
        '\u{258C}' => {
            bg.push_rect(x, y, (w / 2.0).round(), h, fg);
        }
        // ▍ Left 3/8
        '\u{258D}' => {
            bg.push_rect(x, y, (w * 3.0 / 8.0).round(), h, fg);
        }
        // ▎ Left 1/4
        '\u{258E}' => {
            bg.push_rect(x, y, (w / 4.0).round(), h, fg);
        }
        // ▏ Left 1/8
        '\u{258F}' => {
            bg.push_rect(x, y, (w / 8.0).round(), h, fg);
        }
        // ▐ Right half
        '\u{2590}' => {
            let hw = (w / 2.0).round();
            bg.push_rect(x + w - hw, y, hw, h, fg);
        }
        // ░ Light shade (25%)
        '\u{2591}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.25];
            bg.push_rect(x, y, w, h, shade);
        }
        // ▒ Medium shade (50%)
        '\u{2592}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.5];
            bg.push_rect(x, y, w, h, shade);
        }
        // ▓ Dark shade (75%)
        '\u{2593}' => {
            let shade = [fg[0], fg[1], fg[2], fg[3] * 0.75];
            bg.push_rect(x, y, w, h, shade);
        }
        // ▔ Upper 1/8
        '\u{2594}' => {
            bg.push_rect(x, y, w, (h / 8.0).round(), fg);
        }
        // ▕ Right 1/8
        '\u{2595}' => {
            let bw = (w / 8.0).round();
            bg.push_rect(x + w - bw, y, bw, h, fg);
        }
        // Quadrant block elements (U+2596–U+259F)
        // Each is a combination of quarter-cell fills:
        //   TL = top-left, TR = top-right, BL = bottom-left, BR = bottom-right
        '\u{2596}' => {
            // ▖ Quadrant lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y + hh, hw, h - hh, fg);
        }
        '\u{2597}' => {
            // ▗ Quadrant lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg);
        }
        '\u{2598}' => {
            // ▘ Quadrant upper left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg);
        }
        '\u{2599}' => {
            // ▙ Quadrant upper left + lower left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg);           // TL
            bg.push_rect(x, y + hh, w, h - hh, fg);   // full bottom
        }
        '\u{259A}' => {
            // ▚ Quadrant upper left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, hw, hh, fg);                    // TL
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg); // BR
        }
        '\u{259B}' => {
            // ▛ Quadrant upper left + upper right + lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, w, hh, fg);            // full top
            bg.push_rect(x, y + hh, hw, h - hh, fg);  // BL
        }
        '\u{259C}' => {
            // ▜ Quadrant upper left + upper right + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x, y, w, hh, fg);                     // full top
            bg.push_rect(x + hw, y + hh, w - hw, h - hh, fg);  // BR
        }
        '\u{259D}' => {
            // ▝ Quadrant upper right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg);
        }
        '\u{259E}' => {
            // ▞ Quadrant upper right + lower left
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg);       // TR
            bg.push_rect(x, y + hh, hw, h - hh, fg);       // BL
        }
        '\u{259F}' => {
            // ▟ Quadrant upper right + lower left + lower right
            let hw = (w / 2.0).round();
            let hh = (h / 2.0).round();
            bg.push_rect(x + hw, y, w - hw, hh, fg);   // TR
            bg.push_rect(x, y + hh, w, h - hh, fg);    // full bottom
        }
        _ => return false,
    }
    true
}

// --- Instance data serialization ---

/// Writes cell instance data to a byte buffer without unsafe code.
struct InstanceWriter {
    data: Vec<u8>,
    opacity: f32,
}

impl InstanceWriter {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(4096),
            opacity: 1.0,
        }
    }

    /// Push a colored background rectangle (no texture).
    /// When opacity < 1.0, the color is premultiplied by opacity.
    fn push_rect(&mut self, x: f32, y: f32, w: f32, h: f32, bg_color: [f32; 4]) {
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
        );
    }

    /// Push a textured glyph quad (alpha-blended).
    /// `bg_color` is passed through to the shader for contrast/correction.
    fn push_glyph(
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
        self.push_raw(
            [x, y],
            [w, h],
            uv_pos,
            uv_size,
            fg_color,
            bg_color,
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

/// Convert an sRGB component (0.0–1.0) to linear light.
pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert VTE RGB to [f32; 4] RGBA in **linear** space (alpha=1.0).
fn vte_rgb_to_rgba(rgb: vte::ansi::Rgb) -> [f32; 4] {
    [
        srgb_to_linear(f32::from(rgb.r) / 255.0),
        srgb_to_linear(f32::from(rgb.g) / 255.0),
        srgb_to_linear(f32::from(rgb.b) / 255.0),
        1.0,
    ]
}

/// Convert palette `default_bg()` to RGBA (linear).
fn palette_to_rgba(rgb: vte::ansi::Rgb) -> [f32; 4] {
    vte_rgb_to_rgba(rgb)
}

/// Convert a u32 color (0x00RRGGBB) to [f32; 4] RGBA in **linear** space.
#[cfg(target_os = "windows")]
fn u32_to_rgba(c: u32) -> [f32; 4] {
    [
        srgb_to_linear(((c >> 16) & 0xFF) as f32 / 255.0),
        srgb_to_linear(((c >> 8) & 0xFF) as f32 / 255.0),
        srgb_to_linear((c & 0xFF) as f32 / 255.0),
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
