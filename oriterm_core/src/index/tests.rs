use super::{Column, Direction, Line, Point, Side};

#[test]
fn line_arithmetic() {
    assert_eq!(Line(5) + Line(3), Line(8));
    assert_eq!(Line(5) - Line(3), Line(2));
    assert_eq!(Line(-2) + Line(5), Line(3));
    assert_eq!(Line(0) - Line(1), Line(-1));
}

#[test]
fn line_assign_arithmetic() {
    let mut l = Line(5);
    l += Line(3);
    assert_eq!(l, Line(8));
    l -= Line(2);
    assert_eq!(l, Line(6));
}

#[test]
fn line_conversions() {
    assert_eq!(Line::from(42), Line(42));
    assert_eq!(i32::from(Line(42)), 42);
}

#[test]
fn line_display() {
    assert_eq!(format!("{}", Line(7)), "7");
    assert_eq!(format!("{}", Line(-3)), "-3");
}

#[test]
fn column_arithmetic() {
    assert_eq!(Column(5) + Column(3), Column(8));
    assert_eq!(Column(5) - Column(3), Column(2));
}

#[test]
fn column_assign_arithmetic() {
    let mut c = Column(5);
    c += Column(3);
    assert_eq!(c, Column(8));
    c -= Column(2);
    assert_eq!(c, Column(6));
}

#[test]
fn column_conversions() {
    assert_eq!(Column::from(42_usize), Column(42));
    assert_eq!(usize::from(Column(42)), 42);
}

#[test]
fn column_display() {
    assert_eq!(format!("{}", Column(7)), "7");
}

#[test]
fn point_ordering() {
    let a = Point::new(Line(0), Column(5));
    let b = Point::new(Line(1), Column(0));
    let c = Point::new(Line(0), Column(10));

    // Line takes priority over column.
    assert!(a < b);
    // Same line: column breaks the tie.
    assert!(a < c);
    // Equality.
    assert_eq!(a, Point::new(Line(0), Column(5)));
}

#[test]
fn point_ordering_with_negative_lines() {
    let history = Point::new(Line(-1), Column(0));
    let visible = Point::new(Line(0), Column(0));
    assert!(history < visible);
}

#[test]
fn side_equality() {
    assert_eq!(Side::Left, Side::Left);
    assert_ne!(Side::Left, Side::Right);
}

#[test]
fn direction_equality() {
    assert_eq!(Direction::Left, Direction::Left);
    assert_ne!(Direction::Left, Direction::Right);
}

// --- Additional tests from reference repo gap analysis ---

#[test]
fn point_default_is_origin() {
    let p = Point::<Line>::default();
    assert_eq!(p.line, Line(0));
    assert_eq!(p.column, Column(0));
}

#[test]
fn line_ordering() {
    assert!(Line(-1) < Line(0));
    assert!(Line(0) < Line(1));
    assert_eq!(Line(5).cmp(&Line(5)), std::cmp::Ordering::Equal);
}

#[test]
fn column_ordering() {
    assert!(Column(0) < Column(1));
    assert!(Column(5) < Column(10));
    assert_eq!(Column(5).cmp(&Column(5)), std::cmp::Ordering::Equal);
}

#[test]
fn point_same_line_column_breaks_tie() {
    let a = Point::new(Line(3), Column(5));
    let b = Point::new(Line(3), Column(10));
    let c = Point::new(Line(3), Column(5));
    assert!(a < b);
    assert_eq!(a, c);
    assert!(b > a);
}
