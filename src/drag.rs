use winit::dpi::PhysicalPosition;
use winit::window::WindowId;

use crate::tab::TabId;

/// Chrome-style thresholds from `tab_drag_controller.cc`
pub const DRAG_START_THRESHOLD: f64 = 10.0;
pub const TEAR_OFF_THRESHOLD: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPhase {
    /// Mouse down, hasn't moved enough yet.
    Pending,
    /// Reordering within the tab strip.
    DraggingInBar,
    /// Tab is in its own window, following cursor.
    TornOff,
}

#[derive(Debug)]
pub struct DragState {
    pub tab_id: TabId,
    pub source_window: WindowId,
    pub origin: PhysicalPosition<f64>,
    pub phase: DragPhase,
    /// Original index of the tab before drag started (for revert on Escape).
    pub original_index: usize,
    /// Where in the torn-off window the cursor "grabs" — the window moves
    /// so the cursor stays at this position within it.
    pub grab_offset: PhysicalPosition<f64>,
    /// X distance from the tab's left edge to the cursor at drag start.
    /// Used for pixel-perfect tab tracking during in-bar drag.
    pub mouse_offset_in_tab: f64,
}

impl DragState {
    pub fn new(
        tab_id: TabId,
        source_window: WindowId,
        origin: PhysicalPosition<f64>,
        original_index: usize,
    ) -> Self {
        Self {
            tab_id,
            source_window,
            origin,
            phase: DragPhase::Pending,
            original_index,
            grab_offset: PhysicalPosition::new(0.0, 0.0),
            mouse_offset_in_tab: 0.0,
        }
    }

    /// Euclidean distance from origin.
    pub fn distance_from_origin(&self, pos: PhysicalPosition<f64>) -> f64 {
        let dx = pos.x - self.origin.x;
        let dy = pos.y - self.origin.y;
        dx.hypot(dy)
    }

    /// Vertical distance from origin (for tear-off detection).
    pub fn vertical_distance(&self, pos: PhysicalPosition<f64>) -> f64 {
        (pos.y - self.origin.y).abs()
    }
}
