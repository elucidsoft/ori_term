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

    let char_idx_start = char_index_at_byte(text, byte_start);
    let char_idx_end = char_index_containing_byte(text, byte_end);

    // col_map only has entries for non-spacer cells, so char indices
    // from zero-width chars may exceed col_map length.
    let start_col = col_map.get(char_idx_start).copied()?;
    let end_col = col_map.get(char_idx_end).copied()?;
    Some((start_col, end_col))
}

/// Return the char index of the first character starting at or after `byte_offset`.
fn char_index_at_byte(text: &str, byte_offset: usize) -> usize {
    let mut pos = 0;
    for (i, ch) in text.chars().enumerate() {
        if pos >= byte_offset {
            return i;
        }
        pos += ch.len_utf8();
    }
    text.chars().count()
}

/// Return the char index of the character whose encoding contains `byte_offset`
/// (i.e., the last character whose cumulative byte position reaches `byte_offset`).
fn char_index_containing_byte(text: &str, byte_offset: usize) -> usize {
    let mut pos = 0;
    for (i, ch) in text.chars().enumerate() {
        pos += ch.len_utf8();
        if pos >= byte_offset {
            return i;
        }
    }
    text.chars().count().saturating_sub(1)
}
