//! Stable row identity that survives scrollback eviction.

use super::Grid;

/// Monotonically increasing row identity that survives scrollback eviction.
///
/// Row 0 is the first row ever written to this grid. Unlike raw absolute
/// indices (which shift when scrollback evicts rows), `StableRowIndex`
/// values remain valid across eviction, scroll, and resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableRowIndex(pub u64);

impl StableRowIndex {
    /// Convert a visible line (accounting for `display_offset`) to a stable index.
    pub fn from_visible(grid: &Grid, line: usize) -> Self {
        let abs = grid.scrollback.len().saturating_sub(grid.display_offset) + line;
        Self(grid.total_evicted() as u64 + abs as u64)
    }

    /// Convert back to an absolute row index (scrollback + viewport).
    ///
    /// Returns `None` if the row has been evicted from scrollback.
    pub fn to_absolute(self, grid: &Grid) -> Option<usize> {
        let evicted = grid.total_evicted() as u64;
        if self.0 < evicted {
            return None;
        }
        let abs = (self.0 - evicted) as usize;
        let total = grid.scrollback.len() + grid.lines;
        if abs < total {
            Some(abs)
        } else {
            None
        }
    }

    /// Create from a raw absolute row index (scrollback + viewport coordinate).
    pub fn from_absolute(grid: &Grid, abs_row: usize) -> Self {
        Self(grid.total_evicted() as u64 + abs_row as u64)
    }
}
