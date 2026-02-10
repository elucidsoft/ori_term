use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

use crate::drag::{DragPhase, DragState, DRAG_START_THRESHOLD, TEAR_OFF_THRESHOLD};
use crate::log;
use crate::palette::rgb_to_u32;
use crate::render::{self, GlyphCache, render_grid};
use crate::tab::{Tab, TabId, TermEvent};
use crate::tab_bar::{self, TabBarHit, TabBarLayout, TAB_BAR_HEIGHT, GRID_PADDING_LEFT, GRID_PADDING_BOTTOM, render_window_border};
use crate::term_mode::TermMode;
use crate::window::TermWindow;

const DEFAULT_COLS: usize = 120;
const DEFAULT_ROWS: usize = 30;

// Resize border thickness in pixels
const RESIZE_BORDER: f64 = 8.0;

// Double-click detection threshold in milliseconds
const DOUBLE_CLICK_MS: u128 = 400;

// Scroll lines per mouse wheel tick
const SCROLL_LINES: usize = 3;

pub struct App {
    windows: HashMap<WindowId, TermWindow>,
    tabs: HashMap<TabId, Tab>,
    glyphs: GlyphCache,
    drag: Option<DragState>,
    next_tab_id: u64,
    proxy: EventLoopProxy<TermEvent>,
    cursor_pos: HashMap<WindowId, PhysicalPosition<f64>>,
    hover_hit: HashMap<WindowId, TabBarHit>,
    modifiers: ModifiersState,
    first_window_created: bool,
    last_click_time: Option<Instant>,
    last_click_window: Option<WindowId>,
}

impl App {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        std::panic::set_hook(Box::new(|info| {
            let _ = std::fs::write("oriterm_panic.log", format!("{info}"));
        }));

        let _ = std::fs::remove_file(crate::log_path());
        log("starting");

        let font_data = render::load_font();
        log(&format!("font loaded: {} bytes", font_data.len()));
        let glyphs = GlyphCache::new(&font_data, render::FONT_SIZE);
        log(&format!(
            "glyphs: cell={}x{}, baseline={}",
            glyphs.cell_width, glyphs.cell_height, glyphs.baseline
        ));

        let event_loop = EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("event loop");
        let proxy = event_loop.create_proxy();

        let mut app = Self {
            windows: HashMap::new(),
            tabs: HashMap::new(),
            glyphs,
            drag: None,
            next_tab_id: 1,
            proxy,
            cursor_pos: HashMap::new(),
            hover_hit: HashMap::new(),
            modifiers: ModifiersState::empty(),
            first_window_created: false,
            last_click_time: None,
            last_click_window: None,
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }

    fn alloc_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    fn grid_dims_for_size(&self, width: u32, height: u32) -> (usize, usize) {
        let cw = self.glyphs.cell_width;
        let ch = self.glyphs.cell_height;
        let grid_w = (width as usize).saturating_sub(GRID_PADDING_LEFT);
        let grid_h = (height as usize).saturating_sub(TAB_BAR_HEIGHT + 10 + GRID_PADDING_BOTTOM);
        let cols = if cw > 0 { grid_w / cw } else { DEFAULT_COLS };
        let rows = if ch > 0 { grid_h / ch } else { DEFAULT_ROWS };
        (cols.max(2), rows.max(1))
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Option<WindowId> {
        let win_w = (self.glyphs.cell_width * DEFAULT_COLS) as u32 + GRID_PADDING_LEFT as u32;
        let win_h = (self.glyphs.cell_height * DEFAULT_ROWS) as u32 + TAB_BAR_HEIGHT as u32 + 10 + GRID_PADDING_BOTTOM as u32;

        let attrs = Window::default_attributes()
            .with_title("oriterm")
            .with_inner_size(winit::dpi::PhysicalSize::new(win_w, win_h))
            .with_decorations(false);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("failed to create window: {e}"));
                return None;
            }
        };

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                log(&format!("failed to create softbuffer context: {e}"));
                return None;
            }
        };

        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("failed to create surface: {e}"));
                return None;
            }
        };

        let id = window.id();
        let mut tw = TermWindow::new(window, context, surface);

        // Fill surface with dark background immediately to prevent white flash
        let bg_u32 = rgb_to_u32(crate::palette::Palette::new().default_bg());
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(win_w), NonZeroU32::new(win_h)) {
            if tw.surface.resize(nw, nh).is_ok() {
                if let Ok(mut buf) = tw.surface.buffer_mut() {
                    buf.fill(bg_u32);
                    let _ = buf.present();
                }
            }
        }

        self.windows.insert(id, tw);
        log(&format!("created window {:?}", id));
        Some(id)
    }

    fn new_tab_in_window(
        &mut self,
        window_id: WindowId,
    ) -> Option<TabId> {
        // Compute grid size from window
        let (cols, rows) = self.windows.get(&window_id)
            .map_or((DEFAULT_COLS, DEFAULT_ROWS), |tw| {
                let size = tw.window.inner_size();
                self.grid_dims_for_size(size.width, size.height)
            });

        let tab_id = self.alloc_tab_id();
        let tab = match Tab::spawn(tab_id, cols, rows, self.proxy.clone()) {
            Ok(t) => t,
            Err(e) => {
                log(&format!("failed to spawn tab: {e}"));
                return None;
            }
        };

        self.tabs.insert(tab_id, tab);

        if let Some(tw) = self.windows.get_mut(&window_id) {
            tw.add_tab(tab_id);
            tw.window.request_redraw();
        }

        log(&format!("new tab {:?} in window {:?}", tab_id, window_id));
        Some(tab_id)
    }

    fn close_tab(&mut self, tab_id: TabId, event_loop: &ActiveEventLoop) {
        self.tabs.remove(&tab_id);

        // Find the window containing this tab and remove it
        let mut empty_windows = Vec::new();
        for (wid, tw) in &mut self.windows {
            if tw.remove_tab(tab_id) {
                empty_windows.push(*wid);
            } else {
                tw.window.request_redraw();
            }
        }

        // Close windows that have no tabs left
        for wid in empty_windows {
            if self.windows.len() <= 1 {
                // Last window — exit
                event_loop.exit();
                return;
            }
            self.windows.remove(&wid);
        }
    }

    fn close_window(&mut self, window_id: WindowId, event_loop: &ActiveEventLoop) {
        let tab_ids: Vec<TabId> = self
            .windows
            .get(&window_id)
            .map(|tw| tw.tabs.clone())
            .unwrap_or_default();
        for tid in tab_ids {
            self.tabs.remove(&tid);
        }
        self.windows.remove(&window_id);

        if self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn render_window(&mut self, window_id: WindowId) {
        // Extract all info we need before borrowing the surface mutably.
        let (phys, tab_info, active_idx, active_tab_id, is_maximized) = {
            let tw = match self.windows.get(&window_id) {
                Some(tw) => tw,
                None => return,
            };
            let phys = tw.window.inner_size();
            let tab_info: Vec<(TabId, String)> = tw
                .tabs
                .iter()
                .map(|id| {
                    let title = self
                        .tabs
                        .get(id).map_or_else(|| "?".to_string(), |t| t.title.clone());
                    (*id, title)
                })
                .collect();
            let active_idx = tw.active_tab;
            let active_tab_id = tw.active_tab_id();
            let is_maximized = tw.is_maximized;
            (phys, tab_info, active_idx, active_tab_id, is_maximized)
        };

        if phys.width == 0 || phys.height == 0 {
            return;
        }

        let w = phys.width as usize;
        let h = phys.height as usize;
        let hover = self.hover_hit.get(&window_id).copied().unwrap_or(TabBarHit::None);

        // Get palette info for background color
        let bg_u32 = self.tabs.values().next()
            .map_or(0x001e1e2e, |t| rgb_to_u32(t.palette.default_bg()));

        let tw = match self.windows.get_mut(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        if tw.surface.resize(
            NonZeroU32::new(phys.width).unwrap(),
            NonZeroU32::new(phys.height).unwrap(),
        ).is_err() {
            return;
        }

        let mut buffer = match tw.surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };

        buffer.fill(bg_u32);

        // Render tab bar
        tab_bar::render_tab_bar(
            &mut self.glyphs,
            &mut buffer,
            w,
            h,
            &tab_info,
            active_idx,
            hover,
            is_maximized,
        );

        // Render active tab's grid
        if let Some(tab_id) = active_tab_id {
            if let Some(tab) = self.tabs.get(&tab_id) {
                render_grid(
                    &mut self.glyphs,
                    tab.grid(),
                    &tab.palette,
                    tab.mode,
                    &mut buffer,
                    w,
                    h,
                    GRID_PADDING_LEFT,
                    TAB_BAR_HEIGHT + 10,
                );
            }
        }

        // Draw 1px window border on top of everything (Windows 10 style)
        render_window_border(&mut buffer, w, h, is_maximized);

        let _ = buffer.present();
    }

    /// Detect if cursor is in the resize border zone. Returns the resize direction
    /// if so, or None if the cursor is in the client area.
    fn resize_direction_at(
        &self,
        window_id: WindowId,
        pos: PhysicalPosition<f64>,
    ) -> Option<ResizeDirection> {
        let tw = self.windows.get(&window_id)?;
        if tw.is_maximized {
            return None; // No resize when maximized
        }

        let size = tw.window.inner_size();
        let w = size.width as f64;
        let h = size.height as f64;
        let x = pos.x;
        let y = pos.y;

        let left = x < RESIZE_BORDER;
        let right = x >= w - RESIZE_BORDER;
        let top = y < RESIZE_BORDER;
        let bottom = y >= h - RESIZE_BORDER;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn handle_resize(&mut self, window_id: WindowId, width: u32, height: u32) {
        let (cols, rows) = self.grid_dims_for_size(width, height);
        log(&format!(
            "handle_resize: window={width}x{height} cell={}x{} cols={cols} rows={rows}",
            self.glyphs.cell_width, self.glyphs.cell_height
        ));

        let tw = match self.windows.get(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let pixel_w = width as u16;
        let pixel_h = height as u16;

        for &tab_id in &tw.tabs {
            if let Some(tab) = self.tabs.get_mut(&tab_id) {
                tab.resize(cols, rows, pixel_w, pixel_h);
            }
        }
    }

    fn handle_mouse_input(
        &mut self,
        window_id: WindowId,
        state: ElementState,
        button: MouseButton,
        event_loop: &ActiveEventLoop,
    ) {
        if button != MouseButton::Left {
            return;
        }

        let pos = self
            .cursor_pos
            .get(&window_id)
            .copied()
            .unwrap_or(PhysicalPosition::new(0.0, 0.0));

        let x = pos.x as usize;
        let y = pos.y as usize;

        match state {
            ElementState::Pressed => {
                // Check resize borders first
                if let Some(direction) = self.resize_direction_at(window_id, pos) {
                    if let Some(tw) = self.windows.get(&window_id) {
                        let _ = tw.window.drag_resize_window(direction);
                    }
                    return;
                }

                if y < TAB_BAR_HEIGHT {
                    let tw = match self.windows.get(&window_id) {
                        Some(tw) => tw,
                        None => return,
                    };
                    let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
                    let hit = layout.hit_test(x, y);

                    match hit {
                        TabBarHit::NewTab => {
                            self.new_tab_in_window(window_id);
                        }
                        TabBarHit::CloseTab(idx) => {
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                self.close_tab(tab_id, event_loop);
                            }
                        }
                        TabBarHit::Minimize => {
                            if let Some(tw) = self.windows.get(&window_id) {
                                tw.window.set_minimized(true);
                            }
                        }
                        TabBarHit::Maximize => {
                            if let Some(tw) = self.windows.get_mut(&window_id) {
                                let new_max = !tw.is_maximized;
                                tw.is_maximized = new_max;
                                tw.window.set_maximized(new_max);
                                tw.window.request_redraw();
                            }
                        }
                        TabBarHit::CloseWindow => {
                            self.close_window(window_id, event_loop);
                        }
                        TabBarHit::DragArea => {
                            // Check for double-click to toggle maximize
                            let now = Instant::now();
                            let is_double = self.last_click_time
                                .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS)
                                && self.last_click_window == Some(window_id);

                            if is_double {
                                // Double-click: toggle maximize
                                self.last_click_time = None;
                                self.last_click_window = None;
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    let new_max = !tw.is_maximized;
                                    tw.is_maximized = new_max;
                                    tw.window.set_maximized(new_max);
                                    tw.window.request_redraw();
                                }
                            } else {
                                // Single click: start window drag
                                self.last_click_time = Some(now);
                                self.last_click_window = Some(window_id);
                                if let Some(tw) = self.windows.get(&window_id) {
                                    let _ = tw.window.drag_window();
                                }
                            }
                        }
                        TabBarHit::Tab(idx) => {
                            // Start potential drag
                            let tw = match self.windows.get(&window_id) {
                                Some(tw) => tw,
                                None => return,
                            };
                            if let Some(&tab_id) = tw.tabs.get(idx) {
                                self.drag = Some(DragState::new(tab_id, window_id, pos, idx));
                                // Also select this tab
                                if let Some(tw) = self.windows.get_mut(&window_id) {
                                    tw.active_tab = idx;
                                    tw.window.request_redraw();
                                }
                            }
                        }
                        TabBarHit::None => {}
                    }
                }
            }
            ElementState::Released => {
                if let Some(drag) = self.drag.take() {
                    // drag.source_window was updated to the torn-off window
                    // during tear_off_tab — use it, not window_id
                    let torn_wid = drag.source_window;
                    match drag.phase {
                        DragPhase::TornOff => {
                            // Check if we're over another window's tab bar
                            if let Some(target_wid) = self.find_window_at_cursor(torn_wid) {
                                self.reattach_tab(drag.tab_id, torn_wid, target_wid, pos);
                            }
                            // Window is already created — nothing else needed
                        }
                        DragPhase::DraggingInBar | DragPhase::Pending => {
                            // Tab reorder finalized / was just a click — nothing extra needed
                        }
                    }
                }
            }
        }
    }

    fn handle_mouse_wheel(&mut self, window_id: WindowId, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                if y > 0.0 { SCROLL_LINES as i32 } else { -(SCROLL_LINES as i32) }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                let cell_h = self.glyphs.cell_height as f64;
                if cell_h > 0.0 {
                    (pos.y / cell_h).round() as i32
                } else {
                    0
                }
            }
        };

        if lines == 0 {
            return;
        }

        let tab_id = self.windows.get(&window_id)
            .and_then(TermWindow::active_tab_id);
        if let Some(tid) = tab_id {
            if let Some(tab) = self.tabs.get_mut(&tid) {
                let grid = tab.grid_mut();
                if lines > 0 {
                    // Scroll up (into history)
                    let max = grid.scrollback.len();
                    grid.display_offset = (grid.display_offset + lines as usize).min(max);
                } else {
                    // Scroll down (toward live)
                    grid.display_offset = grid.display_offset.saturating_sub((-lines) as usize);
                }
            }
            if let Some(tw) = self.windows.get(&window_id) {
                tw.window.request_redraw();
            }
        }
    }

    fn handle_cursor_moved(&mut self, window_id: WindowId, position: PhysicalPosition<f64>, event_loop: &ActiveEventLoop) {
        self.cursor_pos.insert(window_id, position);

        // Update cursor icon for resize borders
        let cursor_icon = if let Some(dir) = self.resize_direction_at(window_id, position) {
            dir.into()
        } else {
            CursorIcon::Default
        };
        if let Some(tw) = self.windows.get(&window_id) {
            tw.window.set_cursor(cursor_icon);
        }

        // Update hover state for tab bar
        let y = position.y as usize;
        let x = position.x as usize;

        if y < TAB_BAR_HEIGHT {
            if let Some(tw) = self.windows.get(&window_id) {
                let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
                let hit = layout.hit_test(x, y);
                let prev = self.hover_hit.insert(window_id, hit);
                if prev != Some(hit) {
                    tw.window.request_redraw();
                }
            }
        } else {
            let prev = self.hover_hit.insert(window_id, TabBarHit::None);
            if prev != Some(TabBarHit::None) {
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }
        }

        // Handle drag — extract values to avoid borrow conflicts with self
        let drag_action = self.drag.as_ref().map(|drag| {
            (drag.phase, drag.tab_id, drag.source_window, drag.grab_offset,
             drag.distance_from_origin(position), drag.vertical_distance(position))
        });

        if let Some((phase, tab_id, source_wid, grab_offset, dist, vert_dist)) = drag_action {
            match phase {
                DragPhase::Pending => {
                    if dist >= DRAG_START_THRESHOLD {
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::DraggingInBar;
                        }
                        log("drag: pending -> dragging in bar");
                    }
                }
                DragPhase::DraggingInBar => {
                    if vert_dist >= TEAR_OFF_THRESHOLD {
                        log("drag: tearing off!");
                        self.tear_off_tab(tab_id, source_wid, position, event_loop);
                        if let Some(ref mut drag) = self.drag {
                            drag.phase = DragPhase::TornOff;
                        }
                    } else {
                        self.reorder_tab_in_bar(window_id, position);
                    }
                }
                DragPhase::TornOff => {
                    // Convert cursor to screen coordinates using the window
                    // that actually sent this CursorMoved event.
                    let screen_cursor = self.windows.get(&window_id)
                        .and_then(|tw| tw.window.inner_position().ok())
                        .map(|ip| (ip.x as f64 + position.x, ip.y as f64 + position.y));

                    if let Some((sx, sy)) = screen_cursor {
                        // Position torn-off window so cursor stays at grab_offset
                        let torn_wid = self.window_containing_tab(tab_id);
                        if let Some(wid) = torn_wid {
                            if let Some(tw) = self.windows.get(&wid) {
                                let new_x = sx - grab_offset.x;
                                let new_y = sy - grab_offset.y;
                                tw.window.set_outer_position(
                                    PhysicalPosition::new(new_x as i32, new_y as i32),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn tear_off_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        cursor: PhysicalPosition<f64>,
        event_loop: &ActiveEventLoop,
    ) {
        // Remove tab from source window
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
            tw.window.request_redraw();
        }

        // Compute screen-space cursor position from source window
        let screen_cursor = self
            .windows
            .get(&source_wid)
            .and_then(|tw| tw.window.outer_position().ok())
            .map(|wp| (wp.x + cursor.x as i32, wp.y + cursor.y as i32));

        // The grab offset: where the cursor will be within the new window.
        // Put it at a comfortable spot — center-ish of a tab, vertically in tab bar.
        let grab_x = 75.0; // roughly half a tab width
        let grab_y = (TAB_BAR_HEIGHT / 2) as f64;

        // Create new frameless window at cursor position
        if let Some(new_wid) = self.create_window(event_loop) {
            if let Some(tw) = self.windows.get_mut(&new_wid) {
                tw.add_tab(tab_id);
                // Position so cursor is at grab_offset within the client area.
                // No title bar offset needed — frameless window has no OS decoration.
                if let Some((sx, sy)) = screen_cursor {
                    let win_x = sx - grab_x as i32;
                    let win_y = sy - grab_y as i32;
                    tw.window
                        .set_outer_position(PhysicalPosition::new(win_x, win_y));
                }
                tw.window.request_redraw();

                // Hand off to the OS native move loop. This is blocking —
                // Windows handles the drag with full Aero Snap support.
                // When the user releases the mouse, this returns.
                let _ = tw.window.drag_window();
            }

            // Native drag finished (mouse released). Clear our drag state.
            self.drag = None;
        }

        // If source window is empty, close it
        let source_empty = self
            .windows
            .get(&source_wid)
            .is_some_and(|tw| tw.tabs.is_empty());
        if source_empty {
            self.windows.remove(&source_wid);
        }
    }

    fn reattach_tab(
        &mut self,
        tab_id: TabId,
        source_wid: WindowId,
        target_wid: WindowId,
        _cursor: PhysicalPosition<f64>,
    ) {
        // Remove from source window
        if let Some(tw) = self.windows.get_mut(&source_wid) {
            tw.remove_tab(tab_id);
        }

        // Add to target window
        if let Some(tw) = self.windows.get_mut(&target_wid) {
            tw.add_tab(tab_id);
            tw.window.request_redraw();
        }

        // Close empty source window
        let source_empty = self
            .windows
            .get(&source_wid)
            .is_some_and(|tw| tw.tabs.is_empty());
        if source_empty {
            self.windows.remove(&source_wid);
        }
    }

    fn reorder_tab_in_bar(&mut self, window_id: WindowId, position: PhysicalPosition<f64>) {
        let drag = match &self.drag {
            Some(d) => d,
            None => return,
        };
        let tab_id = drag.tab_id;

        let tw = match self.windows.get_mut(&window_id) {
            Some(tw) => tw,
            None => return,
        };

        let layout = TabBarLayout::compute(tw.tabs.len(), tw.window.inner_size().width as usize);
        let new_idx = (position.x as usize / layout.tab_width).min(tw.tabs.len().saturating_sub(1));

        if let Some(current_idx) = tw.tab_index(tab_id) {
            if current_idx != new_idx {
                tw.tabs.remove(current_idx);
                tw.tabs.insert(new_idx, tab_id);
                tw.active_tab = new_idx;
                tw.window.request_redraw();
            }
        }
    }

    fn window_containing_tab(&self, tab_id: TabId) -> Option<WindowId> {
        for (wid, tw) in &self.windows {
            if tw.tabs.contains(&tab_id) {
                return Some(*wid);
            }
        }
        None
    }

    fn find_window_at_cursor(&self, _exclude: WindowId) -> Option<WindowId> {
        // For Phase 4: check if cursor is in another window's tab bar.
        // Simplified for now — would need screen coordinates.
        None
    }
}

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.first_window_created {
            return;
        }
        self.first_window_created = true;

        if let Some(wid) = self.create_window(event_loop) {
            self.new_tab_in_window(wid);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::PtyOutput(tab_id, data) => {
                log(&format!("pty_output: tab={:?} len={}", tab_id, data.len()));

                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    tab.process_output(&data);
                }
                // Redraw the window containing this tab
                if let Some(wid) = self.window_containing_tab(tab_id) {
                    // Only redraw if this is the active tab in that window
                    if let Some(tw) = self.windows.get(&wid) {
                        if tw.active_tab_id() == Some(tab_id) {
                            tw.window.request_redraw();
                        }
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }

            WindowEvent::RedrawRequested => {
                self.render_window(window_id);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(window_id, size.width, size.height);
                if let Some(tw) = self.windows.get(&window_id) {
                    tw.window.request_redraw();
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(window_id, state, button, event_loop);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(window_id, delta);
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(window_id, position, event_loop);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Handle Escape during drag
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    if self.drag.is_some() {
                        // Cancel drag — revert to original state
                        self.drag = None;
                        // Redraw all windows
                        for tw in self.windows.values() {
                            tw.window.request_redraw();
                        }
                        return;
                    }
                }

                let ctrl = self.modifiers.control_key();
                let shift = self.modifiers.shift_key();

                // Ctrl+T — new tab
                if ctrl && matches!(&event.logical_key, Key::Character(c) if c.as_str() == "t") {
                    self.new_tab_in_window(window_id);
                    return;
                }

                // Ctrl+W — close active tab
                if ctrl && matches!(&event.logical_key, Key::Character(c) if c.as_str() == "w") {
                    let tab_id = self
                        .windows
                        .get(&window_id)
                        .and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        self.close_tab(tid, event_loop);
                    }
                    return;
                }

                // Ctrl+Tab / Ctrl+Shift+Tab — cycle tabs
                if ctrl && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
                    if let Some(tw) = self.windows.get_mut(&window_id) {
                        let n = tw.tabs.len();
                        if n > 1 {
                            if shift {
                                tw.active_tab = (tw.active_tab + n - 1) % n;
                            } else {
                                tw.active_tab = (tw.active_tab + 1) % n;
                            }
                            tw.window.request_redraw();
                        }
                    }
                    return;
                }

                // Shift+PageUp/PageDown — page scroll through scrollback
                if shift && matches!(event.logical_key, Key::Named(NamedKey::PageUp)) {
                    let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            let grid = tab.grid_mut();
                            let page = grid.lines;
                            let max = grid.scrollback.len();
                            grid.display_offset = (grid.display_offset + page).min(max);
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    }
                    return;
                }

                if shift && matches!(event.logical_key, Key::Named(NamedKey::PageDown)) {
                    let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            let grid = tab.grid_mut();
                            let page = grid.lines;
                            grid.display_offset = grid.display_offset.saturating_sub(page);
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    }
                    return;
                }

                if shift && matches!(event.logical_key, Key::Named(NamedKey::Home)) {
                    let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            let grid = tab.grid_mut();
                            grid.display_offset = grid.scrollback.len();
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    }
                    return;
                }

                if shift && matches!(event.logical_key, Key::Named(NamedKey::End)) {
                    let tab_id = self.windows.get(&window_id).and_then(TermWindow::active_tab_id);
                    if let Some(tid) = tab_id {
                        if let Some(tab) = self.tabs.get_mut(&tid) {
                            tab.grid_mut().display_offset = 0;
                        }
                        if let Some(tw) = self.windows.get(&window_id) {
                            tw.window.request_redraw();
                        }
                    }
                    return;
                }

                // Any keyboard input to PTY — scroll to live
                let tab_id = self
                    .windows
                    .get(&window_id)
                    .and_then(TermWindow::active_tab_id);
                if let Some(tid) = tab_id {
                    // Scroll to live on any PTY input
                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        tab.grid_mut().display_offset = 0;
                    }

                    if let Some(tab) = self.tabs.get_mut(&tid) {
                        let app_cursor = tab.mode.contains(TermMode::APP_CURSOR);
                        match &event.logical_key {
                            Key::Named(NamedKey::Enter) => tab.send_pty(b"\r"),
                            Key::Named(NamedKey::Backspace) => tab.send_pty(&[0x7f]),
                            Key::Named(NamedKey::Tab) => tab.send_pty(b"\t"),
                            Key::Named(NamedKey::Escape) => tab.send_pty(&[0x1b]),
                            Key::Named(NamedKey::ArrowUp) => {
                                if app_cursor { tab.send_pty(b"\x1bOA") }
                                else { tab.send_pty(b"\x1b[A") }
                            }
                            Key::Named(NamedKey::ArrowDown) => {
                                if app_cursor { tab.send_pty(b"\x1bOB") }
                                else { tab.send_pty(b"\x1b[B") }
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                if app_cursor { tab.send_pty(b"\x1bOC") }
                                else { tab.send_pty(b"\x1b[C") }
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                if app_cursor { tab.send_pty(b"\x1bOD") }
                                else { tab.send_pty(b"\x1b[D") }
                            }
                            Key::Named(NamedKey::Home) => {
                                if app_cursor { tab.send_pty(b"\x1bOH") }
                                else { tab.send_pty(b"\x1b[H") }
                            }
                            Key::Named(NamedKey::End) => {
                                if app_cursor { tab.send_pty(b"\x1bOF") }
                                else { tab.send_pty(b"\x1b[F") }
                            }
                            Key::Named(NamedKey::Delete) => tab.send_pty(b"\x1b[3~"),
                            Key::Named(NamedKey::PageUp) => tab.send_pty(b"\x1b[5~"),
                            Key::Named(NamedKey::PageDown) => tab.send_pty(b"\x1b[6~"),
                            Key::Named(NamedKey::Insert) => tab.send_pty(b"\x1b[2~"),
                            Key::Named(NamedKey::F1) => tab.send_pty(b"\x1bOP"),
                            Key::Named(NamedKey::F2) => tab.send_pty(b"\x1bOQ"),
                            Key::Named(NamedKey::F3) => tab.send_pty(b"\x1bOR"),
                            Key::Named(NamedKey::F4) => tab.send_pty(b"\x1bOS"),
                            Key::Named(NamedKey::F5) => tab.send_pty(b"\x1b[15~"),
                            Key::Named(NamedKey::F6) => tab.send_pty(b"\x1b[17~"),
                            Key::Named(NamedKey::F7) => tab.send_pty(b"\x1b[18~"),
                            Key::Named(NamedKey::F8) => tab.send_pty(b"\x1b[19~"),
                            Key::Named(NamedKey::F9) => tab.send_pty(b"\x1b[20~"),
                            Key::Named(NamedKey::F10) => tab.send_pty(b"\x1b[21~"),
                            Key::Named(NamedKey::F11) => tab.send_pty(b"\x1b[23~"),
                            Key::Named(NamedKey::F12) => tab.send_pty(b"\x1b[24~"),
                            _ => {
                                if let Some(text) = &event.text {
                                    tab.send_pty(text.as_bytes());
                                }
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
