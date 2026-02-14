//! Pixel-to-cell coordinate conversion.

use winit::dpi::PhysicalPosition;

use crate::grid::{GRID_PADDING_LEFT, GRID_PADDING_TOP};
use crate::selection::Side;
use crate::tab_bar::TAB_BAR_HEIGHT;

use super::App;

impl App {
    /// Top pixel of the grid area (below tab bar + padding).
    pub(super) fn grid_top(&self) -> usize {
        self.scale_px(TAB_BAR_HEIGHT) + self.scale_px(GRID_PADDING_TOP)
    }

    /// Convert pixel coordinates to grid cell (col, `viewport_line`).
    /// Returns None if outside the grid area.
    pub(super) fn pixel_to_cell(&self, pos: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        let x = pos.x as usize;
        let y = pos.y as usize;
        let grid_top = self.grid_top();
        let padding_left = self.scale_px(GRID_PADDING_LEFT);
        if y < grid_top || x < padding_left {
            return None;
        }
        let cw = self.font_collection.cell_width;
        let ch = self.font_collection.cell_height;
        if cw == 0 || ch == 0 {
            return None;
        }
        let col = (x - padding_left) / cw;
        let line = (y - grid_top) / ch;
        Some((col, line))
    }

    /// Determine which side of the cell the cursor is on.
    pub(super) fn pixel_to_side(&self, pos: PhysicalPosition<f64>) -> Side {
        let x = pos.x as usize;
        let cw = self.font_collection.cell_width;
        if cw == 0 {
            return Side::Left;
        }
        let padding_left = self.scale_px(GRID_PADDING_LEFT);
        let cell_x = (x.saturating_sub(padding_left)) % cw;
        if cell_x < cw / 2 {
            Side::Left
        } else {
            Side::Right
        }
    }
}
