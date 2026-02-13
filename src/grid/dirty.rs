//! Row-level dirty tracking for incremental GPU instance rebuilds.

/// Tracks which viewport rows have changed and need GPU instance rebuilding.
///
/// When a row is clean, the renderer can skip re-processing its cells entirely.
/// Structural changes (scroll, resize, reflow) mark all rows dirty.
pub struct DirtyTracker {
    /// Per-row dirty flag. True = needs GPU instance rebuild.
    bits: Vec<bool>,
    /// True if any structural change occurred (resize, scroll, reflow).
    /// Forces full rebuild regardless of per-row flags.
    structural: bool,
}

impl DirtyTracker {
    /// Creates a new tracker with all rows marked dirty.
    pub fn new(lines: usize) -> Self {
        Self {
            bits: vec![true; lines],
            structural: true,
        }
    }

    /// Mark a single viewport row as dirty.
    pub fn mark_row(&mut self, line: usize) {
        if let Some(b) = self.bits.get_mut(line) {
            *b = true;
        }
    }

    /// Mark all rows as dirty (scroll, resize, theme change).
    pub fn mark_all(&mut self) {
        self.bits.fill(true);
        self.structural = true;
    }

    /// Mark a range of rows as dirty (scroll region operations).
    pub fn mark_range(&mut self, start: usize, end_inclusive: usize) {
        let end = end_inclusive.min(self.bits.len().saturating_sub(1));
        for b in &mut self.bits[start..=end] {
            *b = true;
        }
    }

    /// Check if a row needs rebuild.
    pub fn is_dirty(&self, line: usize) -> bool {
        self.structural || self.bits.get(line).copied().unwrap_or(true)
    }

    /// Returns true if any row is dirty (for Tab-level dirty check).
    pub fn any_dirty(&self) -> bool {
        self.structural || self.bits.iter().any(|&b| b)
    }

    /// Clear dirty flags after a frame is built.
    pub fn clear(&mut self) {
        self.bits.fill(false);
        self.structural = false;
    }

    /// Resize tracking to match new viewport dimensions.
    pub fn resize(&mut self, new_lines: usize) {
        self.bits.resize(new_lines, true);
        self.structural = true;
    }
}

impl Clone for DirtyTracker {
    fn clone(&self) -> Self {
        Self {
            bits: self.bits.clone(),
            structural: self.structural,
        }
    }
}

impl std::fmt::Debug for DirtyTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dirty_count = self.bits.iter().filter(|&&b| b).count();
        f.debug_struct("DirtyTracker")
            .field("dirty_rows", &dirty_count)
            .field("total_rows", &self.bits.len())
            .field("structural", &self.structural)
            .finish()
    }
}
