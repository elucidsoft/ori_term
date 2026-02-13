//! Search functionality — plain text and regex search across grid content.

mod find;
#[cfg(test)]
mod tests;
mod text;

use crate::grid::Grid;

pub(crate) use text::extract_row_text;

/// Type of match at a cell position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    None,
    Match,
    FocusedMatch,
}

/// A single search match span in absolute grid coordinates.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

/// State for an active search session, including query, matches, and navigation.
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub focused: usize,
    pub case_sensitive: bool,
    pub use_regex: bool,
}

impl SearchState {
    /// Creates a new empty search state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance to the next match, wrapping around.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.focused = (self.focused + 1) % self.matches.len();
        }
    }

    /// Go to the previous match, wrapping around.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.focused == 0 {
                self.focused = self.matches.len() - 1;
            } else {
                self.focused -= 1;
            }
        }
    }

    /// Re-run the search with the current query settings.
    pub fn update_query(&mut self, grid: &Grid) {
        if self.query.is_empty() {
            self.matches.clear();
            self.focused = 0;
            return;
        }
        self.matches = find::find_matches(grid, &self.query, self.case_sensitive, self.use_regex);
        // Clamp focused index
        if self.matches.is_empty() {
            self.focused = 0;
        } else {
            self.focused = self.focused.min(self.matches.len() - 1);
        }
    }

    /// Returns the currently focused match, if any.
    pub fn focused_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.focused)
    }

    /// Check whether a cell at (`abs_row`, `col`) is inside any match.
    /// Uses binary search for efficient lookup on sorted matches.
    pub fn cell_match_type(&self, abs_row: usize, col: usize) -> MatchType {
        // Binary search: find the first match whose end_row >= abs_row
        let idx = self
            .matches
            .partition_point(|m| m.end_row < abs_row || (m.end_row == abs_row && m.end_col < col));

        // Check a small window of matches near the found index
        for i in idx.saturating_sub(1)..self.matches.len().min(idx + 2) {
            let m = &self.matches[i];
            if cell_in_match(m, abs_row, col) {
                return if i == self.focused {
                    MatchType::FocusedMatch
                } else {
                    MatchType::Match
                };
            }
        }
        MatchType::None
    }
}

/// Check whether (`abs_row`, `col`) falls within a match span.
fn cell_in_match(m: &SearchMatch, abs_row: usize, col: usize) -> bool {
    if abs_row < m.start_row || abs_row > m.end_row {
        return false;
    }
    if m.start_row == m.end_row {
        return col >= m.start_col && col <= m.end_col;
    }
    if abs_row == m.start_row {
        return col >= m.start_col;
    }
    if abs_row == m.end_row {
        return col <= m.end_col;
    }
    // Middle rows are fully matched
    true
}
