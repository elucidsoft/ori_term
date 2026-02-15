//! Tests for the event system.

use std::sync::Arc;

use super::{ClipboardType, Event, EventListener, Rgb, VoidListener};

#[test]
fn void_listener_implements_event_listener() {
    let listener = VoidListener;
    // Should compile and not panic — the default no-op body runs.
    listener.send_event(Event::Wakeup);
    listener.send_event(Event::Bell);
}

#[test]
fn void_listener_is_send_and_static() {
    fn assert_send_static<T: Send + 'static>() {}
    assert_send_static::<VoidListener>();
}

#[test]
fn event_wakeup() {
    let event = Event::Wakeup;
    assert_eq!(format!("{event:?}"), "Wakeup");
}

#[test]
fn event_bell() {
    let event = Event::Bell;
    assert_eq!(format!("{event:?}"), "Bell");
}

#[test]
fn event_title() {
    let event = Event::Title("hello".to_string());
    assert_eq!(format!("{event:?}"), "Title(hello)");
}

#[test]
fn event_reset_title() {
    let event = Event::ResetTitle;
    assert_eq!(format!("{event:?}"), "ResetTitle");
}

#[test]
fn event_clipboard_store() {
    let event = Event::ClipboardStore(ClipboardType::Clipboard, "data".to_string());
    assert_eq!(format!("{event:?}"), "ClipboardStore(Clipboard, data)");
}

#[test]
fn event_clipboard_load() {
    let formatter = Arc::new(|s: &str| format!("formatted:{s}"));
    let event = Event::ClipboardLoad(ClipboardType::Selection, formatter);
    assert_eq!(format!("{event:?}"), "ClipboardLoad(Selection)");
}

#[test]
fn event_color_request() {
    let formatter = Arc::new(|rgb: Rgb| format!("rgb({},{},{})", rgb.r, rgb.g, rgb.b));
    let event = Event::ColorRequest(42, formatter);
    assert_eq!(format!("{event:?}"), "ColorRequest(42)");
}

#[test]
fn event_pty_write() {
    let event = Event::PtyWrite("\x1b[6n".to_string());
    assert_eq!(format!("{event:?}"), "PtyWrite(\x1b[6n)");
}

#[test]
fn event_cursor_blinking_change() {
    let event = Event::CursorBlinkingChange;
    assert_eq!(format!("{event:?}"), "CursorBlinkingChange");
}

#[test]
fn event_mouse_cursor_dirty() {
    let event = Event::MouseCursorDirty;
    assert_eq!(format!("{event:?}"), "MouseCursorDirty");
}

#[test]
fn event_child_exit() {
    let event = Event::ChildExit(0);
    assert_eq!(format!("{event:?}"), "ChildExit(0)");

    let event = Event::ChildExit(1);
    assert_eq!(format!("{event:?}"), "ChildExit(1)");
}

#[test]
fn clipboard_type_variants() {
    assert_ne!(ClipboardType::Clipboard, ClipboardType::Selection);

    let c = ClipboardType::Clipboard;
    let s = ClipboardType::Selection;
    assert_eq!(c, ClipboardType::Clipboard);
    assert_eq!(s, ClipboardType::Selection);
}

#[test]
fn event_clone() {
    let event = Event::Title("test".to_string());
    let cloned = event.clone();
    assert_eq!(format!("{cloned:?}"), "Title(test)");
}

#[test]
fn all_event_variants_constructible() {
    // Verify every variant can be constructed without panic.
    let _events = [
        Event::Wakeup,
        Event::Bell,
        Event::Title(String::new()),
        Event::ResetTitle,
        Event::ClipboardStore(ClipboardType::Clipboard, String::new()),
        Event::ClipboardLoad(ClipboardType::Selection, Arc::new(|s: &str| s.to_string())),
        Event::ColorRequest(0, Arc::new(|_| String::new())),
        Event::PtyWrite(String::new()),
        Event::CursorBlinkingChange,
        Event::MouseCursorDirty,
        Event::ChildExit(0),
    ];
}
