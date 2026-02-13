//! Search functionality — plain text and regex search across grid content.

use crate::cell::CellFlags;
use crate::grid::Grid;

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
        self.matches = find_matches(grid, &self.query, self.case_sensitive, self.use_regex);
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

/// Extract text from a single grid row, returning the text and a mapping
/// from character index (in the returned string) to column index.
pub(crate) fn extract_row_text(row: &crate::grid::row::Row) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut col_map = Vec::new();
    for (col, cell) in row.iter().enumerate() {
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
            || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let c = if cell.c == '\0' { ' ' } else { cell.c };
        text.push(c);
        col_map.push(col);
        for &zw in cell.zerowidth() {
            text.push(zw);
        }
    }
    (text, col_map)
}

/// Find all matches in the grid for the given query.
/// Returns matches sorted by position (earliest first).
#[expect(
    clippy::string_slice,
    reason = "Slicing is safe after find() returns valid byte positions"
)]
fn find_matches(
    grid: &Grid,
    query: &str,
    case_sensitive: bool,
    use_regex: bool,
) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let total_rows = grid.scrollback.len() + grid.lines;

    if use_regex {
        let re = regex::RegexBuilder::new(query)
            .case_insensitive(!case_sensitive)
            .build();
        let re = match re {
            Ok(r) => r,
            Err(_) => return matches, // Invalid regex — return empty
        };

        // Search row by row (multi-row regex matches deferred)
        for abs_row in 0..total_rows {
            if let Some(row) = grid.absolute_row(abs_row) {
                let (text, col_map) = extract_row_text(row);
                for m in re.find_iter(&text) {
                    if let Some(span) = byte_span_to_cols(&text, &col_map, m.start(), m.end()) {
                        matches.push(SearchMatch {
                            start_row: abs_row,
                            start_col: span.0,
                            end_row: abs_row,
                            end_col: span.1,
                        });
                    }
                }
            }
        }
    } else {
        // Plain text search
        let query_lower;
        let search_query = if case_sensitive {
            query
        } else {
            query_lower = query.to_lowercase();
            &query_lower
        };

        for abs_row in 0..total_rows {
            if let Some(row) = grid.absolute_row(abs_row) {
                let (text, col_map) = extract_row_text(row);
                let search_text;
                let haystack = if case_sensitive {
                    &text
                } else {
                    search_text = text.to_lowercase();
                    &search_text
                };

                let mut start = 0;
                while let Some(pos) = haystack[start..].find(search_query) {
                    let byte_start = start + pos;
                    let byte_end = byte_start + search_query.len();
                    if let Some(span) = byte_span_to_cols(&text, &col_map, byte_start, byte_end) {
                        matches.push(SearchMatch {
                            start_row: abs_row,
                            start_col: span.0,
                            end_row: abs_row,
                            end_col: span.1,
                        });
                    }
                    // Advance past this match to find overlapping matches
                    start = byte_start + 1;
                    if start >= haystack.len() {
                        break;
                    }
                }
            }
        }
    }

    matches
}

/// Convert a byte span in the extracted text to column indices.
/// Returns `(start_col, end_col)` inclusive, or `None` if the span is empty.
pub(crate) fn byte_span_to_cols(
    text: &str,
    col_map: &[usize],
    byte_start: usize,
    byte_end: usize,
) -> Option<(usize, usize)> {
    if byte_start >= byte_end {
        return None;
    }

    // Find the character index for byte_start
    let mut char_idx_start = 0;
    let mut byte_pos = 0;
    for (i, ch) in text.chars().enumerate() {
        if byte_pos >= byte_start {
            char_idx_start = i;
            break;
        }
        byte_pos += ch.len_utf8();
        char_idx_start = i + 1;
    }

    // Find the character index for byte_end (exclusive -> we want the last char)
    let mut char_idx_end = 0;
    byte_pos = 0;
    for (i, ch) in text.chars().enumerate() {
        byte_pos += ch.len_utf8();
        if byte_pos >= byte_end {
            char_idx_end = i;
            break;
        }
        char_idx_end = i;
    }

    // Map character indices to column indices via col_map
    // col_map only has entries for non-spacer cells, so char indices
    // from zero-width chars may exceed col_map length.
    let start_col = col_map.get(char_idx_start).copied()?;
    let end_col = col_map.get(char_idx_end).copied()?;
    Some((start_col, end_col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_basic() {
        let mut grid = Grid::new(20, 2);
        for (i, c) in "hello world".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }
        for (i, c) in "foo hello bar".chars().enumerate() {
            grid.goto(1, i);
            grid.put_char(c);
        }

        let matches = find_matches(&grid, "hello", false, false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start_col, 0);
        assert_eq!(matches[0].end_col, 4);
        assert_eq!(matches[0].start_row, 0);
        assert_eq!(matches[1].start_col, 4);
        assert_eq!(matches[1].end_col, 8);
        assert_eq!(matches[1].start_row, 1);
    }

    #[test]
    fn search_case_insensitive() {
        let mut grid = Grid::new(20, 1);
        for (i, c) in "Hello HELLO hello".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }

        let matches = find_matches(&grid, "hello", false, false);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn search_case_sensitive() {
        let mut grid = Grid::new(20, 1);
        for (i, c) in "Hello HELLO hello".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }

        let matches = find_matches(&grid, "hello", true, false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_col, 12);
    }

    #[test]
    fn search_regex() {
        let mut grid = Grid::new(20, 1);
        for (i, c) in "abc 123 def 456".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }

        let matches = find_matches(&grid, r"\d+", false, true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start_col, 4);
        assert_eq!(matches[0].end_col, 6);
        assert_eq!(matches[1].start_col, 12);
        assert_eq!(matches[1].end_col, 14);
    }

    #[test]
    fn search_invalid_regex() {
        let grid = Grid::new(20, 1);
        let matches = find_matches(&grid, r"[invalid", false, true);
        assert!(matches.is_empty());
    }

    #[test]
    fn search_empty_query() {
        let mut state = SearchState::new();
        let grid = Grid::new(20, 1);
        state.query = String::new();
        state.update_query(&grid);
        assert!(state.matches.is_empty());
    }

    #[test]
    fn search_next_prev() {
        let mut state = SearchState::new();
        state.matches = vec![
            SearchMatch {
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 2,
            },
            SearchMatch {
                start_row: 1,
                start_col: 0,
                end_row: 1,
                end_col: 2,
            },
            SearchMatch {
                start_row: 2,
                start_col: 0,
                end_row: 2,
                end_col: 2,
            },
        ];
        state.focused = 0;

        state.next_match();
        assert_eq!(state.focused, 1);
        state.next_match();
        assert_eq!(state.focused, 2);
        state.next_match();
        assert_eq!(state.focused, 0); // wrap

        state.prev_match();
        assert_eq!(state.focused, 2); // wrap back
        state.prev_match();
        assert_eq!(state.focused, 1);
    }

    #[test]
    fn cell_match_type_check() {
        let mut state = SearchState::new();
        state.matches = vec![
            SearchMatch {
                start_row: 0,
                start_col: 5,
                end_row: 0,
                end_col: 9,
            },
            SearchMatch {
                start_row: 2,
                start_col: 0,
                end_row: 2,
                end_col: 3,
            },
        ];
        state.focused = 0;

        assert_eq!(state.cell_match_type(0, 5), MatchType::FocusedMatch);
        assert_eq!(state.cell_match_type(0, 7), MatchType::FocusedMatch);
        assert_eq!(state.cell_match_type(0, 4), MatchType::None);
        assert_eq!(state.cell_match_type(0, 10), MatchType::None);
        assert_eq!(state.cell_match_type(2, 1), MatchType::Match);
        assert_eq!(state.cell_match_type(1, 0), MatchType::None);
    }
}
