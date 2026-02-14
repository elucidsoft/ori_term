//! Core terminal emulation data structures and logic.
//!
//! This crate provides the foundational types for terminal emulation:
//! cell representation, grid storage, cursor management, scrollback,
//! and all grid editing/navigation operations. It contains no GUI,
//! rendering, or platform-specific code.

#![deny(unsafe_code)]

pub mod cell;
pub mod grid;
pub mod index;

pub use cell::{Cell, CellExtra, CellFlags, Hyperlink};
pub use grid::{Cursor, CursorShape, EraseMode, Grid, Row, TabClearMode};
pub use index::{Boundary, Column, Direction, Line, Point, Side};
