use std::sync::Arc;

use winit::window::Window;

use crate::gpu::GpuState;
use crate::tab::TabId;
use crate::tab_bar::TAB_BAR_HEIGHT;

pub struct TermWindow {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub tabs: Vec<TabId>,
    pub active_tab: usize,
    pub tab_bar_height: usize,
    pub is_maximized: bool,
}

impl TermWindow {
    pub fn new(window: Arc<Window>, gpu: &GpuState) -> Option<Self> {
        let (surface, config) = gpu.create_surface(&window)?;
        Some(Self {
            window,
            surface,
            surface_config: config,
            tabs: Vec::new(),
            active_tab: 0,
            tab_bar_height: TAB_BAR_HEIGHT,
            is_maximized: false,
        })
    }

    pub fn resize_surface(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(device, &self.surface_config);
    }

    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).copied()
    }

    pub fn add_tab(&mut self, id: TabId) {
        self.tabs.push(id);
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn insert_tab_at(&mut self, id: TabId, index: usize) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, id);
        self.active_tab = idx;
    }

    pub fn remove_tab(&mut self, id: TabId) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| *t == id) {
            self.tabs.remove(pos);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
        self.tabs.is_empty()
    }

    pub fn tab_index(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| *t == id)
    }
}
