use super::DirtyTracker;

#[test]
fn new_tracker_is_clean() {
    let tracker = DirtyTracker::new(10);
    assert!(!tracker.is_any_dirty());
    for i in 0..10 {
        assert!(!tracker.is_dirty(i));
    }
}

#[test]
fn mark_single_line() {
    let mut tracker = DirtyTracker::new(10);
    tracker.mark(5);

    assert!(tracker.is_dirty(5));
    assert!(tracker.is_any_dirty());

    // Other lines remain clean.
    assert!(!tracker.is_dirty(0));
    assert!(!tracker.is_dirty(4));
    assert!(!tracker.is_dirty(6));
    assert!(!tracker.is_dirty(9));
}

#[test]
fn mark_all_makes_everything_dirty() {
    let mut tracker = DirtyTracker::new(10);
    tracker.mark_all();

    assert!(tracker.is_any_dirty());
    for i in 0..10 {
        assert!(tracker.is_dirty(i));
    }
}

#[test]
fn drain_returns_dirty_lines() {
    let mut tracker = DirtyTracker::new(10);
    tracker.mark(2);
    tracker.mark(7);
    tracker.mark(7); // duplicate mark is idempotent

    let indices: Vec<usize> = tracker.drain().collect();
    assert_eq!(indices, vec![2, 7]);
}

#[test]
fn drain_resets_to_clean() {
    let mut tracker = DirtyTracker::new(10);
    tracker.mark(3);
    tracker.mark(8);

    // Consume all dirty lines.
    let _: Vec<usize> = tracker.drain().collect();

    // Everything should be clean now.
    assert!(!tracker.is_any_dirty());
    for i in 0..10 {
        assert!(!tracker.is_dirty(i));
    }
}

#[test]
fn drain_mark_all_yields_every_line() {
    let mut tracker = DirtyTracker::new(5);
    tracker.mark_all();

    let indices: Vec<usize> = tracker.drain().collect();
    assert_eq!(indices, vec![0, 1, 2, 3, 4]);

    // Clean after drain.
    assert!(!tracker.is_any_dirty());
}

#[test]
fn resize_marks_all_dirty() {
    let mut tracker = DirtyTracker::new(5);
    assert!(!tracker.is_any_dirty());

    tracker.resize(8);
    assert!(tracker.is_any_dirty());
    for i in 0..8 {
        assert!(tracker.is_dirty(i));
    }

    // Drain and verify 8 lines.
    let indices: Vec<usize> = tracker.drain().collect();
    assert_eq!(indices, vec![0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn drain_drop_clears_remaining() {
    let mut tracker = DirtyTracker::new(10);
    tracker.mark(1);
    tracker.mark(5);
    tracker.mark(9);

    // Only consume the first dirty line, then drop the iterator.
    {
        let mut iter = tracker.drain();
        assert_eq!(iter.next(), Some(1));
        // Drop iter here — lines 5 and 9 should still be cleared.
    }

    // Tracker should be fully clean despite partial iteration.
    assert!(!tracker.is_any_dirty());
    assert!(!tracker.is_dirty(5));
    assert!(!tracker.is_dirty(9));
}

#[test]
fn mark_out_of_bounds_is_safe() {
    let mut tracker = DirtyTracker::new(5);
    tracker.mark(100); // no panic, no effect
    assert!(!tracker.is_any_dirty());
    assert!(!tracker.is_dirty(100));
}
