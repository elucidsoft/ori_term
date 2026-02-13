//! Text shaping via `rustybuzz` — segments grid rows into runs, shapes each run,
//! and maps shaped glyphs back to grid columns.

use crate::cell::{Cell, CellFlags, is_builtin_glyph};
use crate::render::FontStyle;

use super::{FaceIdx, FontCollection, ShapedGlyph};

/// A shaped glyph for UI text rendering (not tied to grid columns).
#[derive(Debug, Clone, Copy)]
pub struct UiShapedGlyph {
    /// Glyph ID within the font face (0 for space-advance-only entries).
    pub glyph_id: u16,
    /// Which face this glyph comes from.
    pub face_idx: FaceIdx,
    /// Pixel advance for cursor positioning.
    pub x_advance: f32,
    /// Shaper X offset from glyph origin.
    pub x_offset: f32,
    /// Shaper Y offset from baseline.
    pub y_offset: f32,
}

/// Shape a line of grid cells into positioned glyphs (convenience wrapper).
///
/// For hot-path usage, prefer the two-phase API: call `prepare_line()` then
/// `shape_prepared_runs()` with pre-created faces and reusable scratch buffers.
pub fn shape_line(
    row: &[Cell],
    cols: usize,
    collection: &mut FontCollection,
) -> Vec<ShapedGlyph> {
    collection.ensure_all_loaded();

    let mut runs = Vec::new();
    prepare_line(row, cols, collection, &mut runs);

    let faces = collection.create_shaping_faces();
    let mut result = Vec::new();
    shape_prepared_runs(&runs, &faces, collection, &mut result);
    result
}

/// Phase 1: Segment a row of cells into shaping runs (immutable).
///
/// Requires `FontCollection::ensure_all_loaded()` to have been called first.
/// Clears and fills `runs_out`. Call once per line before `shape_prepared_runs()`.
pub fn prepare_line(
    row: &[Cell],
    cols: usize,
    collection: &FontCollection,
    runs_out: &mut Vec<ShapingRun>,
) {
    runs_out.clear();
    segment_runs(row, cols, collection, runs_out);
}

/// Phase 2: Shape pre-segmented runs using pre-created faces.
///
/// Clears and fills `output`. Faces should be created once per frame via
/// `FontCollection::create_shaping_faces()`.
pub fn shape_prepared_runs(
    runs: &[ShapingRun],
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<ShapedGlyph>,
) {
    output.clear();
    for run in runs {
        shape_run(run, faces, collection, output);
    }
}

/// A contiguous run of characters sharing the same face and style.
pub struct ShapingRun {
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
    collection: &FontCollection,
    runs: &mut Vec<ShapingRun>,
) {
    let mut col = 0;

    while col < cols {
        let cell = &row[col];

        // Skip wide char spacers
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            col += 1;
            continue;
        }

        // Run boundaries: space, null, built-in glyphs
        if cell.c == ' ' || cell.c == '\0' || is_builtin_glyph(cell.c) {
            col += 1;
            continue;
        }

        let style = FontStyle::from_cell_flags(cell.flags);
        let face_idx = collection.find_face_loaded(cell.c, style);

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

    let features = collection.features_for_face(run.face_idx);
    let glyph_buffer = rustybuzz::shape(face, features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let upem = face.units_per_em() as f32;
    let effective_size = collection.effective_size(run.face_idx);
    let scale = effective_size / upem;
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
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
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

/// Shape a plain text string for UI rendering (tab titles, overlays).
///
/// Requires `FontCollection::ensure_all_loaded()` to have been called.
/// Clears and fills `output`.
pub fn shape_text_string(
    text: &str,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<UiShapedGlyph>,
) {
    output.clear();

    if text.is_empty() {
        return;
    }

    // Segment text into runs by face. Spaces are emitted directly as
    // advance-only glyphs (no shaping needed — consistent with grid path).
    let mut run_start = 0;
    let mut run_face = None;
    let mut run_chars = String::new();

    for ch in text.chars() {
        if ch == ' ' {
            // Flush pending run before the space
            if !run_chars.is_empty() {
                if let Some(fi) = run_face {
                    shape_ui_run(&run_chars, fi, faces, collection, output);
                }
                run_chars.clear();
                run_face = None;
            }
            // Emit space as advance-only glyph
            output.push(UiShapedGlyph {
                glyph_id: 0,
                face_idx: FaceIdx(0),
                x_advance: collection.char_advance(' '),
                x_offset: 0.0,
                y_offset: 0.0,
            });
            run_start += 1;
            continue;
        }

        let face_idx = collection.find_face_loaded(ch, FontStyle::Regular);

        if run_face.is_some_and(|f| f != face_idx) {
            // Face changed — flush the current run
            if let Some(fi) = run_face {
                shape_ui_run(&run_chars, fi, faces, collection, output);
            }
            run_chars.clear();
        }

        run_face = Some(face_idx);
        run_chars.push(ch);
        run_start += 1;
    }

    // Flush the last run
    if !run_chars.is_empty() {
        if let Some(fi) = run_face {
            shape_ui_run(&run_chars, fi, faces, collection, output);
        }
    }

    let _ = run_start; // Consumed by iteration
}

/// Shape a single UI text run and append results.
fn shape_ui_run(
    text: &str,
    face_idx: FaceIdx,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<UiShapedGlyph>,
) {
    let fi = face_idx.0 as usize;
    let Some(face) = faces.get(fi).and_then(|f| f.as_ref()) else {
        // No face available — emit unshaped advances
        for ch in text.chars() {
            output.push(UiShapedGlyph {
                glyph_id: 0,
                face_idx,
                x_advance: collection.char_advance(ch),
                x_offset: 0.0,
                y_offset: 0.0,
            });
        }
        return;
    };

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);

    let features = collection.features_for_face(face_idx);
    let glyph_buffer = rustybuzz::shape(face, features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let upem = face.units_per_em() as f32;
    let effective_size = collection.effective_size(face_idx);
    let scale = effective_size / upem;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        output.push(UiShapedGlyph {
            glyph_id: info.glyph_id as u16,
            face_idx,
            x_advance: pos.x_advance as f32 * scale,
            x_offset: pos.x_offset as f32 * scale,
            y_offset: pos.y_offset as f32 * scale,
        });
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
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
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
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let cells = make_cells("A B");
        let shaped = shape_line(&cells, cells.len(), &mut fc);
        assert_eq!(shaped.len(), 2);
        assert_eq!(shaped[0].col_start, 0);
        assert_eq!(shaped[1].col_start, 2);
    }

    #[test]
    fn shape_empty_line() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let cells = make_cells("   ");
        let shaped = shape_line(&cells, cells.len(), &mut fc);
        assert!(shaped.is_empty(), "spaces produce no shaped glyphs");
    }

    #[test]
    fn two_phase_api_matches_convenience() {
        let mut fc = FontCollection::load(FONT_SIZE, None, &[], &[], 400);
        let cells = make_cells("Hello World");

        let one_shot = shape_line(&cells, cells.len(), &mut fc);

        let mut runs = Vec::new();
        let mut output = Vec::new();
        prepare_line(&cells, cells.len(), &mut fc, &mut runs);
        let faces = fc.create_shaping_faces();
        shape_prepared_runs(&runs, &faces, &fc, &mut output);

        assert_eq!(one_shot.len(), output.len());
        for (a, b) in one_shot.iter().zip(output.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id);
            assert_eq!(a.col_start, b.col_start);
            assert_eq!(a.col_span, b.col_span);
        }
    }
}
