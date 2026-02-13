//! Row text extraction utilities for search and URL detection.

use crate::cell::CellFlags;

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
