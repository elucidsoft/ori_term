//! Type-safe index newtypes for grid coordinates.
//!
//! `Line` and `Column` prevent mixing up row/column values at compile time.
//! `Point` combines them into a grid coordinate. `Side`, `Direction`, and
//! `Boundary` encode semantic meanings used by selection and navigation.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Generate arithmetic and conversion impls for a newtype index wrapper.
macro_rules! index_ops {
    ($ty:ident, $inner:ty) => {
        impl From<$inner> for $ty {
            fn from(val: $inner) -> Self {
                Self(val)
            }
        }

        impl From<$ty> for $inner {
            fn from(val: $ty) -> Self {
                val.0
            }
        }

        impl Add for $ty {
            type Output = Self;

            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl AddAssign for $ty {
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }

        impl Sub for $ty {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl SubAssign for $ty {
            fn sub_assign(&mut self, rhs: Self) {
                self.0 -= rhs.0;
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

/// Signed line index. Negative values refer to scrollback history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Line(pub i32);

index_ops!(Line, i32);

/// Unsigned column index (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Column(pub usize);

index_ops!(Column, usize);

/// A grid coordinate combining a line and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point<L = Line> {
    pub line: L,
    pub column: Column,
}

impl<L> Point<L> {
    /// Create a new point at the given line and column.
    pub fn new(line: L, column: Column) -> Self {
        Self { line, column }
    }
}

impl<L: Ord> Ord for Point<L> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.line.cmp(&other.line) {
            Ordering::Equal => self.column.cmp(&other.column),
            ord => ord,
        }
    }
}

impl<L: Ord> PartialOrd for Point<L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Which half of a cell the cursor is on (for selection granularity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Horizontal direction for search and movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// Semantic boundary for clamping grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Restrict to visible grid area (cursor movement).
    Grid,
    /// Restrict to cursor's valid range of motion.
    Cursor,
    /// Restrict to line-wrap boundaries (selection).
    Wrap,
}

#[cfg(test)]
mod tests {
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
}
