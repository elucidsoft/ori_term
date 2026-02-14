//! Implicit URL detection in grid text using regex patterns.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::grid::{Grid, WrapDetection};
use crate::search::extract_row_text;

/// A single row-segment of a detected URL.
pub type UrlSegment = (usize, usize, usize);

/// A URL detected across one or more grid rows (handles soft-wrapped lines).
#[derive(Debug, Clone)]
pub struct DetectedUrl {
    /// Per-row segments, each inclusive.
    pub segments: Vec<UrlSegment>,
    pub url: String,
}

impl DetectedUrl {
    /// Check whether this URL covers (`abs_row`, `col`).
    pub fn contains(&self, abs_row: usize, col: usize) -> bool {
        self.segments
            .iter()
            .any(|&(r, sc, ec)| r == abs_row && col >= sc && col <= ec)
    }
}

/// Cache of detected URLs keyed by the first absolute row of the logical line.
///
/// Lazily computes URLs for logical lines (sequences of wrapped rows) and caches
/// them to avoid redundant regex matching on hover/click.
#[derive(Default)]
pub struct UrlDetectCache {
    /// Logical line start row -> detected URLs for that logical line.
    lines: HashMap<usize, Vec<DetectedUrl>>,
    /// Row index -> logical line start (for fast lookup of any row).
    row_to_line: HashMap<usize, usize>,
}

impl UrlDetectCache {
    /// Finds a URL at the specified grid position, computing and caching the logical line if needed.
    ///
    /// Returns the URL string and its segments if a URL is found at this position.
    pub fn url_at(&mut self, grid: &Grid, abs_row: usize, col: usize) -> Option<DetectedUrl> {
        let line_start = self.ensure_logical_line(grid, abs_row);
        let urls = self.lines.get(&line_start)?;
        urls.iter().find(|u| u.contains(abs_row, col)).cloned()
    }

    /// Ensures the logical line containing the row is computed and cached.
    ///
    /// Returns the absolute row index of the logical line start.
    fn ensure_logical_line(&mut self, grid: &Grid, abs_row: usize) -> usize {
        if let Some(&ls) = self.row_to_line.get(&abs_row) {
            return ls;
        }
        let line_start = logical_line_start(grid, abs_row);
        let line_end = logical_line_end(grid, abs_row);

        // Detect URLs across the entire logical line
        let urls = detect_urls_in_logical_line(grid, line_start, line_end);

        // Register all rows in this logical line
        for r in line_start..=line_end {
            self.row_to_line.insert(r, line_start);
        }
        self.lines.insert(line_start, urls);
        line_start
    }

    /// Invalidates the entire cache (call after PTY output, scroll, resize).
    pub fn invalidate(&mut self) {
        self.lines.clear();
        self.row_to_line.clear();
    }
}

/// Walks backwards to find the start of contiguous text for URL detection.
fn logical_line_start(grid: &Grid, abs_row: usize) -> usize {
    grid.logical_line_start(abs_row, WrapDetection::WrapOrFilled)
}

/// Walks forwards to find the end of contiguous text for URL detection.
fn logical_line_end(grid: &Grid, abs_row: usize) -> usize {
    grid.logical_line_end(abs_row, WrapDetection::WrapOrFilled)
}

/// URL regex pattern.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:https?|ftp|file)://[^\s<>\[\]'"]+"#).expect("URL regex is valid")
});

/// Trims trailing punctuation from a URL, preserving balanced parentheses.
fn trim_url_trailing(url: &str) -> &str {
    let mut s = url;
    loop {
        let prev = s;
        s = s.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        // Trim trailing ')' only if it's unbalanced
        if let Some(stripped) = s.strip_suffix(')') {
            let open = s.chars().filter(|&c| c == '(').count();
            let close = s.chars().filter(|&c| c == ')').count();
            if close > open {
                s = stripped;
            }
        }
        if s == prev {
            break;
        }
    }
    s
}

/// Detects URLs across a logical line spanning `line_start..=line_end` (absolute rows).
///
/// Concatenates text from all rows, runs the regex, then maps byte spans
/// back to per-row segments.
#[expect(clippy::string_slice, reason = "char-to-byte offset mapping is validated")]
fn detect_urls_in_logical_line(
    grid: &Grid,
    line_start: usize,
    line_end: usize,
) -> Vec<DetectedUrl> {
    // Build concatenated text + a mapping from char-index to (abs_row, col).
    let mut text = String::new();
    let mut char_to_pos: Vec<(usize, usize)> = Vec::new(); // (abs_row, col) per character

    for abs_row in line_start..=line_end {
        let Some(row) = grid.absolute_row(abs_row) else {
            continue;
        };
        let (row_text, col_map) = extract_row_text(row);
        for (ci, _ch) in row_text.chars().enumerate() {
            let col = col_map.get(ci).copied().unwrap_or(0);
            char_to_pos.push((abs_row, col));
        }
        text.push_str(&row_text);
    }

    let mut urls = Vec::new();

    for m in URL_RE.find_iter(&text) {
        let trimmed = trim_url_trailing(m.as_str());
        if trimmed.len() <= "https://".len() {
            continue;
        }

        // Convert byte offsets to char offsets
        let char_start = text[..m.start()].chars().count();
        let trimmed_char_len = trimmed.chars().count();
        let char_end = char_start + trimmed_char_len - 1; // inclusive

        if char_end >= char_to_pos.len() {
            continue;
        }

        // Check for OSC 8 hyperlinks in the span
        let has_osc8 = (char_start..=char_end).any(|ci| {
            let (ar, col) = char_to_pos[ci];
            grid.absolute_row(ar)
                .is_some_and(|row| col < row.len() && row[col].hyperlink().is_some())
        });
        if has_osc8 {
            continue;
        }

        // Build per-row segments
        let mut segments: Vec<UrlSegment> = Vec::new();
        let mut current_row = char_to_pos[char_start].0;
        let mut seg_start_col = char_to_pos[char_start].1;
        let mut seg_end_col = seg_start_col;

        for &(ar, col) in &char_to_pos[char_start..=char_end] {
            if ar != current_row {
                segments.push((current_row, seg_start_col, seg_end_col));
                current_row = ar;
                seg_start_col = col;
            }
            seg_end_col = col;
        }
        segments.push((current_row, seg_start_col, seg_end_col));

        urls.push(DetectedUrl {
            segments,
            url: trimmed.to_string(),
        });
    }

    urls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn detect_simple_url() {
        let mut grid = Grid::new(80, 1);
        for (i, c) in "Visit https://example.com for info".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }
        let urls = detect_urls_in_logical_line(&grid, 0, 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].segments.len(), 1);
        assert_eq!(urls[0].segments[0], (0, 6, 24));
    }

    #[test]
    fn detect_multiple_urls() {
        let mut grid = Grid::new(80, 1);
        for (i, c) in "see https://a.com and http://b.com/x ok"
            .chars()
            .enumerate()
        {
            grid.goto(0, i);
            grid.put_char(c);
        }
        let urls = detect_urls_in_logical_line(&grid, 0, 0);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://a.com");
        assert_eq!(urls[1].url, "http://b.com/x");
    }

    #[test]
    fn detect_url_with_parens() {
        let mut grid = Grid::new(80, 1);
        for (i, c) in "see https://en.wikipedia.org/wiki/Rust_(language) ok"
            .chars()
            .enumerate()
        {
            grid.goto(0, i);
            grid.put_char(c);
        }
        let urls = detect_urls_in_logical_line(&grid, 0, 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://en.wikipedia.org/wiki/Rust_(language)");
    }

    #[test]
    fn no_urls() {
        let mut grid = Grid::new(80, 1);
        for (i, c) in "just plain text here".chars().enumerate() {
            grid.goto(0, i);
            grid.put_char(c);
        }
        let urls = detect_urls_in_logical_line(&grid, 0, 0);
        assert!(urls.is_empty());
    }

    #[test]
    fn detect_wrapped_url() {
        // 20-col grid: URL wraps to second row
        let mut grid = Grid::new(20, 2);
        let text = "go https://example.com/long/path ok";
        // Write characters — grid auto-wraps at col 20
        for c in text.chars() {
            grid.put_char(c);
        }
        // Row 0 should be wrapped (text overflows 20 cols)
        let urls = detect_urls_in_logical_line(&grid, 0, 1);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/long/path");
        assert_eq!(urls[0].segments.len(), 2);
        // First segment: starts at col 3 on row 0, goes to col 19
        assert_eq!(urls[0].segments[0].0, 0);
        assert_eq!(urls[0].segments[0].1, 3); // "go " = 3 chars
        assert_eq!(urls[0].segments[0].2, 19);
        // Second segment: continues on row 1
        assert_eq!(urls[0].segments[1].0, 1);
    }

    #[test]
    fn url_contains() {
        let url = DetectedUrl {
            segments: vec![(5, 3, 19), (6, 0, 10)],
            url: "https://example.com/long/path".to_string(),
        };
        assert!(url.contains(5, 3));
        assert!(url.contains(5, 19));
        assert!(url.contains(6, 0));
        assert!(url.contains(6, 10));
        assert!(!url.contains(5, 2));
        assert!(!url.contains(5, 20));
        assert!(!url.contains(6, 11));
        assert!(!url.contains(7, 0));
    }
}
