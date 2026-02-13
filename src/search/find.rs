//! Search algorithm — find all matches in the grid.

use crate::grid::Grid;

use super::SearchMatch;
use super::text::{byte_span_to_cols, extract_row_text};

/// Find all matches in the grid for the given query.
/// Returns matches sorted by position (earliest first).
#[expect(
    clippy::string_slice,
    reason = "Slicing is safe after find() returns valid byte positions"
)]
pub(super) fn find_matches(
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
