use super::{Cursor, CursorShape};
use crate::index::Column;

#[test]
fn default_cursor_at_origin_with_block_shape() {
    let cursor = Cursor::new();
    assert_eq!(cursor.line(), 0);
    assert_eq!(cursor.col(), Column(0));
    assert_eq!(cursor.shape, CursorShape::Block);
}

#[test]
fn set_line_and_col() {
    let mut cursor = Cursor::new();
    cursor.set_line(5);
    cursor.set_col(Column(10));
    assert_eq!(cursor.line(), 5);
    assert_eq!(cursor.col(), Column(10));
}

#[test]
fn default_shape_is_block() {
    assert_eq!(CursorShape::default(), CursorShape::Block);
}

// --- Additional tests from reference repo gap analysis ---

#[test]
fn template_defaults_to_empty_cell() {
    let cursor = Cursor::new();
    assert!(cursor.template.is_empty());
}

#[test]
fn cursor_clone_preserves_all_fields() {
    let mut cursor = Cursor::new();
    cursor.set_line(5);
    cursor.set_col(Column(10));
    cursor.shape = CursorShape::Bar;
    cursor.template.ch = 'X';

    let cloned = cursor.clone();
    assert_eq!(cloned.line(), 5);
    assert_eq!(cloned.col(), Column(10));
    assert_eq!(cloned.shape, CursorShape::Bar);
    assert_eq!(cloned.template.ch, 'X');
}

#[test]
fn cursor_shape_all_variants_distinct() {
    let shapes = [
        CursorShape::Block,
        CursorShape::Underline,
        CursorShape::Bar,
        CursorShape::HollowBlock,
    ];
    for (i, a) in shapes.iter().enumerate() {
        for (j, b) in shapes.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}
