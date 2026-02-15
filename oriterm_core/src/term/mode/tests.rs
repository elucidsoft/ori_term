//! Tests for terminal mode flags.

use super::TermMode;

#[test]
fn default_has_show_cursor_and_line_wrap() {
    let mode = TermMode::default();
    assert!(mode.contains(TermMode::SHOW_CURSOR));
    assert!(mode.contains(TermMode::LINE_WRAP));
}

#[test]
fn default_does_not_have_other_modes() {
    let mode = TermMode::default();
    assert!(!mode.contains(TermMode::APP_CURSOR));
    assert!(!mode.contains(TermMode::ALT_SCREEN));
    assert!(!mode.contains(TermMode::BRACKETED_PASTE));
    assert!(!mode.contains(TermMode::MOUSE_REPORT_CLICK));
}

#[test]
fn set_and_clear_individual_modes() {
    let mut mode = TermMode::default();

    mode.insert(TermMode::BRACKETED_PASTE);
    assert!(mode.contains(TermMode::BRACKETED_PASTE));

    mode.remove(TermMode::BRACKETED_PASTE);
    assert!(!mode.contains(TermMode::BRACKETED_PASTE));

    // Original defaults still intact.
    assert!(mode.contains(TermMode::SHOW_CURSOR));
    assert!(mode.contains(TermMode::LINE_WRAP));
}

#[test]
fn any_mouse_is_union_of_mouse_modes() {
    let expected = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION;
    assert_eq!(TermMode::ANY_MOUSE, expected);
}

#[test]
fn any_mouse_detects_any_single_mouse_mode() {
    let click_only = TermMode::MOUSE_REPORT_CLICK;
    assert!(click_only.intersects(TermMode::ANY_MOUSE));

    let drag_only = TermMode::MOUSE_DRAG;
    assert!(drag_only.intersects(TermMode::ANY_MOUSE));

    let motion_only = TermMode::MOUSE_MOTION;
    assert!(motion_only.intersects(TermMode::ANY_MOUSE));
}

#[test]
fn empty_mode_has_no_mouse() {
    let mode = TermMode::empty();
    assert!(!mode.intersects(TermMode::ANY_MOUSE));
}

#[test]
fn all_flags_are_distinct() {
    let flags = [
        TermMode::SHOW_CURSOR,
        TermMode::APP_CURSOR,
        TermMode::APP_KEYPAD,
        TermMode::MOUSE_REPORT_CLICK,
        TermMode::MOUSE_DRAG,
        TermMode::MOUSE_MOTION,
        TermMode::MOUSE_SGR,
        TermMode::MOUSE_UTF8,
        TermMode::ALT_SCREEN,
        TermMode::LINE_WRAP,
        TermMode::ORIGIN,
        TermMode::INSERT,
        TermMode::FOCUS_IN_OUT,
        TermMode::BRACKETED_PASTE,
        TermMode::SYNC_UPDATE,
        TermMode::URGENCY_HINTS,
        TermMode::KITTY_KEYBOARD,
        TermMode::CURSOR_BLINKING,
    ];

    // Each individual flag has exactly one bit set (excluding composite ANY_MOUSE).
    for flag in &flags {
        assert!(flag.bits().is_power_of_two(), "{flag:?} is not a single bit");
    }
}
