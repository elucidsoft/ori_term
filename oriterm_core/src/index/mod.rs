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
mod tests;
