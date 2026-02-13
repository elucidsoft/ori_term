//! Text shaping via `rustybuzz` — segments grid rows into runs, shapes each run,
//! and maps shaped glyphs back to grid columns.

use crate::cell::{Cell, CellFlags};
use crate::gpu::builtin_glyphs;
use crate::render::FontStyle;

use super::{FaceIdx, FontCollection, ShapedGlyph};

/// Shape a line of grid cells into positioned glyphs.
///
/// Call `prepare_line()` first (needs `&mut FontCollection` for lazy face loading),
/// then `shape_prepared()` (needs only `&FontCollection` + the rustybuzz faces).
/// This two-phase design avoids the borrow conflict between `create_shaping_faces()`
/// (borrows `FontCollection` data immutably) and `find_face_for_char()` (needs `&mut`).
pub fn shape_line(
    row: &[Cell],
    cols: usize,
    collection: &mut FontCollection,
) -> Vec<ShapedGlyph> {
    // Phase 1: segment runs (may trigger lazy font loading via &mut)
    let runs = segment_runs(row, cols, collection);

    // Phase 2: create transient faces and shape (only &self borrows)
    let faces = collection.create_shaping_faces();
    let mut result = Vec::new();

    for run in &runs {
        shape_run(run, &faces, collection, &mut result);
    }

    result
}

/// A contiguous run of characters sharing the same face and style.
struct ShapingRun {
    /// Text to shape (base chars + combining marks).
    text: String,
    /// Face index for this run.
    face_idx: FaceIdx,
    /// Starting grid column of this run.
    col_start: usize,
    /// Mapping from byte offset in `text` to grid column index.
    byte_to_col: Vec<usize>,
}

/// Segment a row of cells into shaping runs.
///
/// Built-in glyph characters, spaces, and nulls are run boundaries.
/// Wide char spacers are skipped. Combining marks are appended to the
/// current run's text at the same column as their base character.
fn segment_runs(
    row: &[Cell],
    cols: usize,
    collection: &mut FontCollection,
) -> Vec<ShapingRun> {
    let mut runs: Vec<ShapingRun> = Vec::new();
    let mut col = 0;

    while col < cols {
        let cell = &row[col];

        // Skip wide char spacers
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            col += 1;
            continue;
        }

        // Run boundaries: space, null, built-in glyphs
        if cell.c == ' ' || cell.c == '\0' || builtin_glyphs::is_builtin_glyph(cell.c) {
            col += 1;
            continue;
        }

        let style = FontStyle::from_cell_flags(cell.flags);
        let face_idx = collection.find_face_for_char(cell.c, style);

        // Check if we can extend the current run (same face)
        let extend = runs.last().is_some_and(|r: &ShapingRun| r.face_idx == face_idx);

        if extend {
            let run = runs.last_mut().expect("checked above");
            run.text.push(cell.c);
            run.byte_to_col.push(col);
            for _ in 1..cell.c.len_utf8() {
                run.byte_to_col.push(col);
            }
            for &zw in cell.zerowidth() {
                run.text.push(zw);
                for _ in 0..zw.len_utf8() {
                    run.byte_to_col.push(col);
                }
            }
        } else {
            let mut text = String::new();
            let mut byte_to_col = Vec::new();

            text.push(cell.c);
            for _ in 0..cell.c.len_utf8() {
                byte_to_col.push(col);
            }
            for &zw in cell.zerowidth() {
                text.push(zw);
                for _ in 0..zw.len_utf8() {
                    byte_to_col.push(col);
                }
            }

            runs.push(ShapingRun {
                text,
                face_idx,
                col_start: col,
                byte_to_col,
            });
        }

        col += if cell.flags.contains(CellFlags::WIDE_CHAR) { 2 } else { 1 };
    }

    runs
}

/// Shape a single run and append results to the output vec.
fn shape_run(
    run: &ShapingRun,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<ShapedGlyph>,
) {
    let face_i = run.face_idx.0 as usize;
    let Some(face) = faces.get(face_i).and_then(|f| f.as_ref()) else {
        emit_unshaped_fallback(run, output);
        return;
    };

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(&run.text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);

    let glyph_buffer = rustybuzz::shape(face, &collection.features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let upem = face.units_per_em() as f32;
    let scale = collection.size / upem;
    let cell_w = collection.cell_width as f32;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        let cluster = info.cluster as usize;

        // Map cluster (byte offset) to grid column
        let col = run.byte_to_col.get(cluster).copied().unwrap_or(run.col_start);

        // Compute advance in cell units to determine col_span
        let advance_px = pos.x_advance as f32 * scale;
        let col_span = (advance_px / cell_w).round().max(1.0) as usize;

        let x_offset = pos.x_offset as f32 * scale;
        let y_offset = pos.y_offset as f32 * scale;

        output.push(ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            face_idx: run.face_idx,
            col_start: col,
            col_span,
            x_offset,
            y_offset,
        });
    }
}

/// Fallback for when no rustybuzz face is available — emit one glyph per char.
fn emit_unshaped_fallback(
    run: &ShapingRun,
    output: &mut Vec<ShapedGlyph>,
) {
    let mut col = run.col_start;
    for ch in run.text.chars() {
        // Skip combining marks (zero-width) for column counting
        if unicode_width::UnicodeWidthChar::width(ch) == Some(0) {
            continue;
        }

        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        output.push(ShapedGlyph {
            glyph_id: 0,
            face_idx: run.face_idx,
            col_start: col,
            col_span: w,
            x_offset: 0.0,
            y_offset: 0.0,
        });
        col += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::FONT_SIZE;

    fn make_cells(text: &str) -> Vec<Cell> {
        text.chars()
            .map(|c| Cell {
                c,
                ..Cell::default()
            })
            .collect()
    }

    #[test]
    fn shape_hello() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[]);
        let cells = make_cells("Hello");
        let shaped = shape_line(&cells, cells.len(), &mut fc);
        assert_eq!(shaped.len(), 5, "5 glyphs for 'Hello'");
        for g in &shaped {
            assert_eq!(g.col_span, 1);
            assert_ne!(g.glyph_id, 0, "glyph ID should not be .notdef for ASCII");
        }
    }

    #[test]
    fn shape_skips_spaces() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[]);
        let cells = make_cells("A B");
        let shaped = shape_line(&cells, cells.len(), &mut fc);
        assert_eq!(shaped.len(), 2);
        assert_eq!(shaped[0].col_start, 0);
        assert_eq!(shaped[1].col_start, 2);
    }

    #[test]
    fn shape_empty_line() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[]);
        let cells = make_cells("   ");
        let shaped = shape_line(&cells, cells.len(), &mut fc);
        assert!(shaped.is_empty(), "spaces produce no shaped glyphs");
    }
}
