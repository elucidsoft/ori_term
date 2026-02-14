//! GPU device, adapter, queue, and surface management.
//!
//! `GpuState` is shared across all windows and owns the wgpu device lifetime.

use std::sync::Arc;

use winit::window::Window;

/// GPU state shared across all windows.
pub struct GpuState {
    instance: wgpu::Instance,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// The native surface format (used for surface configuration).
    surface_format: wgpu::TextureFormat,
    /// The sRGB format used for render passes and pipelines.
    /// May differ from `surface_format` when the surface doesn't natively
    /// support sRGB (e.g. DX12 `DirectComposition`).
    pub(super) render_format: wgpu::TextureFormat,
    surface_alpha_mode: wgpu::CompositeAlphaMode,
    /// Vulkan pipeline cache (compiled shaders cached to disk across sessions).
    pub(super) pipeline_cache: Option<wgpu::PipelineCache>,
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

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("oriterm"),
            required_features: features,
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
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
        let transparency_supported =
            !matches!(surface_alpha_mode, wgpu::CompositeAlphaMode::Opaque);
        crate::log(&format!(
            "GPU init: adapter={}, backend={:?}, surface_format={surface_format:?}, \
             render_format={render_format:?}, \
             alpha_mode={surface_alpha_mode:?} (available: {:?}), \
             transparency={}",
            info.name,
            info.backend,
            caps.alpha_modes,
            if transparency_supported {
                "supported"
            } else {
                "not supported (opaque)"
            },
        ));

        // Pipeline cache: Vulkan supports caching compiled shaders to disk.
        // On subsequent launches, this skips shader recompilation.
        let (pipeline_cache, pipeline_cache_path) = Self::load_pipeline_cache(&device, &info);

        // Adapter is no longer needed — device and queue are independent.
        drop(adapter);

        Some(Self {
            instance,
            device,
            queue,
            surface_format,
            render_format,
            surface_alpha_mode,
            pipeline_cache,
            pipeline_cache_path,
        })
    }

    /// Returns true if the surface alpha mode supports transparency.
    pub fn supports_transparency(&self) -> bool {
        !matches!(self.surface_alpha_mode, wgpu::CompositeAlphaMode::Opaque)
    }

    /// Compute the `view_formats` list needed for surface configuration.
    /// When the render format differs from the surface format (e.g. DX12
    /// `DirectComposition`), we need an sRGB view format.
    fn view_formats(&self) -> Vec<wgpu::TextureFormat> {
        if self.render_format == self.surface_format {
            vec![]
        } else {
            vec![self.render_format]
        }
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
            if cache_data.is_some() {
                "existing"
            } else {
                "new"
            },
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
            crate::log(&format!(
                "pipeline cache: saved {} bytes to {}",
                data.len(),
                path.display()
            ));
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
            view_formats: self.view_formats(),
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        Some((surface, config))
    }
}
