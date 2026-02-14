//! Chrome-style tab drag state machine (pending → dragging → OS drag).

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
}

#[derive(Debug)]
pub struct DragState {
    pub tab_id: TabId,
    pub source_window: WindowId,
    pub origin: PhysicalPosition<f64>,
    pub phase: DragPhase,
    /// X distance from the tab's left edge to the cursor at drag start.
    /// Used for pixel-perfect tab tracking during in-bar drag.
    pub mouse_offset_in_tab: f64,
}

impl DragState {
    pub fn new(
        tab_id: TabId,
        source_window: WindowId,
        origin: PhysicalPosition<f64>,
    ) -> Self {
        Self {
            tab_id,
            source_window,
            origin,
            phase: DragPhase::Pending,
            mouse_offset_in_tab: 0.0,
        }
    }

    /// Euclidean distance from origin.
    pub fn distance_from_origin(&self, pos: PhysicalPosition<f64>) -> f64 {
        let dx = pos.x - self.origin.x;
        let dy = pos.y - self.origin.y;
        dx.hypot(dy)
    }
}
