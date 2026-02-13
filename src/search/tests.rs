//! Tests for search functionality.

use crate::grid::Grid;

use super::find::find_matches;
use super::*;

#[test]
fn search_basic() {
    let mut grid = Grid::new(20, 2);
    for (i, c) in "hello world".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }
    for (i, c) in "foo hello bar".chars().enumerate() {
        grid.goto(1, i);
        grid.put_char(c);
    }

    let matches = find_matches(&grid, "hello", false, false);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].start_col, 0);
    assert_eq!(matches[0].end_col, 4);
    assert_eq!(matches[0].start_row, 0);
    assert_eq!(matches[1].start_col, 4);
    assert_eq!(matches[1].end_col, 8);
    assert_eq!(matches[1].start_row, 1);
}

#[test]
fn search_case_insensitive() {
    let mut grid = Grid::new(20, 1);
    for (i, c) in "Hello HELLO hello".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }

    let matches = find_matches(&grid, "hello", false, false);
    assert_eq!(matches.len(), 3);
}

#[test]
fn search_case_sensitive() {
    let mut grid = Grid::new(20, 1);
    for (i, c) in "Hello HELLO hello".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }

    let matches = find_matches(&grid, "hello", true, false);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].start_col, 12);
}

#[test]
fn search_regex() {
    let mut grid = Grid::new(20, 1);
    for (i, c) in "abc 123 def 456".chars().enumerate() {
        grid.goto(0, i);
        grid.put_char(c);
    }

    let matches = find_matches(&grid, r"\d+", false, true);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].start_col, 4);
    assert_eq!(matches[0].end_col, 6);
    assert_eq!(matches[1].start_col, 12);
    assert_eq!(matches[1].end_col, 14);
}

#[test]
fn search_invalid_regex() {
    let grid = Grid::new(20, 1);
    let matches = find_matches(&grid, r"[invalid", false, true);
    assert!(matches.is_empty());
}

#[test]
fn search_empty_query() {
    let mut state = SearchState::new();
    let grid = Grid::new(20, 1);
    state.query = String::new();
    state.update_query(&grid);
    assert!(state.matches.is_empty());
}

#[test]
fn search_next_prev() {
    let mut state = SearchState::new();
    state.matches = vec![
        SearchMatch {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 2,
        },
        SearchMatch {
            start_row: 1,
            start_col: 0,
            end_row: 1,
            end_col: 2,
        },
        SearchMatch {
            start_row: 2,
            start_col: 0,
            end_row: 2,
            end_col: 2,
        },
    ];
    state.focused = 0;

    state.next_match();
    assert_eq!(state.focused, 1);
    state.next_match();
    assert_eq!(state.focused, 2);
    state.next_match();
    assert_eq!(state.focused, 0); // wrap

    state.prev_match();
    assert_eq!(state.focused, 2); // wrap back
    state.prev_match();
    assert_eq!(state.focused, 1);
}

#[test]
fn cell_match_type_check() {
    let mut state = SearchState::new();
    state.matches = vec![
        SearchMatch {
            start_row: 0,
            start_col: 5,
            end_row: 0,
            end_col: 9,
        },
        SearchMatch {
            start_row: 2,
            start_col: 0,
            end_row: 2,
            end_col: 3,
        },
    ];
    state.focused = 0;

    assert_eq!(state.cell_match_type(0, 5), MatchType::FocusedMatch);
    assert_eq!(state.cell_match_type(0, 7), MatchType::FocusedMatch);
    assert_eq!(state.cell_match_type(0, 4), MatchType::None);
    assert_eq!(state.cell_match_type(0, 10), MatchType::None);
    assert_eq!(state.cell_match_type(2, 1), MatchType::Match);
    assert_eq!(state.cell_match_type(1, 0), MatchType::None);
}
