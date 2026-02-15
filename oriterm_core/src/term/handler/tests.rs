//! Tests for VTE handler (Print + Execute).
//!
//! Feed raw bytes through `vte::ansi::Processor` → `Term<RecordingListener>`
//! and verify grid state and events.

use std::sync::{Arc, Mutex};

use vte::ansi::Processor;

use crate::event::{Event, EventListener};
use crate::index::Column;
use crate::term::Term;

/// Event listener that records all events for assertions.
#[derive(Clone)]
struct RecordingListener {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingListener {
    fn new() -> Self {
        Self { events: Arc::new(Mutex::new(Vec::new())) }
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("lock poisoned").clone()
    }
}

impl EventListener for RecordingListener {
    fn send_event(&self, event: Event) {
        self.events.lock().expect("lock poisoned").push(format!("{event:?}"));
    }
}

/// Create a Term with 24 lines, 80 columns, and a recording listener.
fn term_with_recorder() -> (Term<RecordingListener>, RecordingListener) {
    let listener = RecordingListener::new();
    let term = Term::new(24, 80, 0, listener.clone());
    (term, listener)
}

/// Create a Term with VoidListener (when events don't matter).
fn term() -> Term<crate::event::VoidListener> {
    Term::new(24, 80, 0, crate::event::VoidListener)
}

/// Feed raw bytes through the VTE processor.
fn feed(term: &mut impl vte::ansi::Handler, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    processor.advance(term, bytes);
}

// --- Print (input) tests ---

#[test]
fn hello_places_cells_and_advances_cursor() {
    let mut t = term();
    feed(&mut t, b"hello");

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'h');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'e');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'o');
    assert_eq!(grid.cursor().col(), Column(5));
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn hello_newline_world() {
    let mut t = term();
    feed(&mut t, b"hello\nworld");

    let grid = t.grid();
    // "hello" on line 0.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'h');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'o');
    // LF only moves down, column stays at 5. "world" starts at col 5 on line 1.
    assert_eq!(grid[crate::index::Line(1)][Column(5)].ch, 'w');
    assert_eq!(grid[crate::index::Line(1)][Column(9)].ch, 'd');
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(10));
}

#[test]
fn carriage_return_overwrites() {
    let mut t = term();
    feed(&mut t, b"hello\rworld");

    let grid = t.grid();
    // "world" overwrites "hello" on line 0.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'w');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'o');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'r');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'd');
    assert_eq!(grid.cursor().col(), Column(5));
}

#[test]
fn tab_advances_to_column_8() {
    let mut t = term();
    feed(&mut t, b"\t");

    // Tab stops are at 0, 8, 16, ... — from col 0, next stop is col 8.
    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn tab_from_midline() {
    let mut t = term();
    feed(&mut t, b"ab\t");

    // From col 2, next tab stop is col 8.
    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn backspace_moves_left() {
    let mut t = term();
    feed(&mut t, b"abc\x08");

    // "abc" puts cursor at col 3; backspace moves to col 2.
    assert_eq!(t.grid().cursor().col(), Column(2));
}

#[test]
fn backspace_at_col_zero_is_noop() {
    let mut t = term();
    feed(&mut t, b"\x08");

    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn bell_triggers_event() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x07");

    let events = listener.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], "Bell");
}

#[test]
fn linefeed_moves_down() {
    let mut t = term();
    feed(&mut t, b"A\n");

    let grid = t.grid();
    assert_eq!(grid.cursor().line(), 1);
    // LF does not change column (unlike CR+LF).
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn vertical_tab_same_as_lf() {
    let mut t = term();
    feed(&mut t, b"A\x0B");

    // VT (0x0B) is treated identically to LF.
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(1));
}

#[test]
fn form_feed_same_as_lf() {
    let mut t = term();
    feed(&mut t, b"A\x0C");

    // FF (0x0C) is treated identically to LF.
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(1));
}

#[test]
fn so_activates_g1_charset() {
    let mut t = term();
    // SO = 0x0E activates G1.
    feed(&mut t, b"\x0E");

    assert_eq!(*t.charset().active(), vte::ansi::CharsetIndex::G1);
}

#[test]
fn si_activates_g0_charset() {
    let mut t = term();
    // SO then SI should restore G0.
    feed(&mut t, b"\x0E\x0F");

    assert_eq!(*t.charset().active(), vte::ansi::CharsetIndex::G0);
}

#[test]
fn crlf_moves_to_start_of_next_line() {
    let mut t = term();
    feed(&mut t, b"hello\r\n");

    let grid = t.grid();
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn multiple_linefeeds() {
    let mut t = term();
    feed(&mut t, b"\n\n\n");

    assert_eq!(t.grid().cursor().line(), 3);
}

#[test]
fn substitute_writes_space() {
    let mut t = term();
    feed(&mut t, b"A\x1AB");

    let grid = t.grid();
    // SUB (0x1A) writes a space.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'B');
}
