use std::sync::Arc;

use winit::window::Window;

use crate::tab::TabId;
use crate::tab_bar::TAB_BAR_HEIGHT;

pub struct TermWindow {
    pub window: Arc<Window>,
    pub context: softbuffer::Context<Arc<Window>>,
    pub surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    pub tabs: Vec<TabId>,
    pub active_tab: usize,
    pub tab_bar_height: usize,
    pub is_maximized: bool,
}

impl TermWindow {
    pub fn new(
        window: Arc<Window>,
        context: softbuffer::Context<Arc<Window>>,
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    ) -> Self {
        Self {
            window,
            context,
            surface,
            tabs: Vec::new(),
            active_tab: 0,
            tab_bar_height: TAB_BAR_HEIGHT,
            is_maximized: false,
        }
    }

    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).copied()
    }

    pub fn add_tab(&mut self, id: TabId) {
        self.tabs.push(id);
        self.active_tab = self.tabs.len() - 1;
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
