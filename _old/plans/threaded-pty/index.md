# Threaded PTY Processing Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Shared Terminal State
**File:** `section-01-shared-state.md` | **Status:** Complete

```
Arc, Mutex, RwLock, FairMutex, parking_lot, lock, guard
shared state, terminal protection, thread safety, concurrency
grid lock, read lock, write lock, lease, fairness
```

---

### Section 02: PTY Processing Thread
**File:** `section-02-pty-thread.md` | **Status:** Complete

```
PTY reader, VTE parsing, process_output, TermHandler
reader thread, spawn_reader_thread, background processing
batched parsing, buffer limit, MAX_LOCKED_READ
RawInterceptor, processor, raw_parser, advance
```

---

### Section 03: Lock-Free Input Channel
**File:** `section-03-input-channel.md` | **Status:** Not Started

```
send_pty, pty_writer, keyboard input, mouse report
channel, mpsc, crossbeam, Msg, Input, Resize
Notifier, write queue, PTY stdin, lock-free
paste, bracketed paste, focus in/out
```

---

### Section 04: Rendering Integration
**File:** `section-04-rendering.md` | **Status:** Not Started

```
render_window, draw_frame, FrameParams, GPU
read lock, immutable borrow, grid access
grid_dirty, tab_bar_dirty, damage tracking
about_to_wait, frame budget, coalesce
```

---

### Section 05: App-Level Refactor
**File:** `section-05-app-refactor.md` | **Status:** Not Started

```
App struct, HashMap tabs, event_loop, user_event
tab access, grid access, selection, search
mouse_selection, hover_url, input_keyboard
borrow checker, lock guard, MutexGuard
```

---

### Section 06: Resize and Lifecycle
**File:** `section-06-resize-lifecycle.md` | **Status:** Not Started

```
resize, SIGWINCH, TIOCSWINSZ, ConPTY
shutdown, close_tab, PtyExited, child waiter
spawn, Tab::spawn, pty_master, pty_system
drop, cleanup, RAII
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Shared Terminal State | `section-01-shared-state.md` |
| 02 | PTY Processing Thread | `section-02-pty-thread.md` |
| 03 | Lock-Free Input Channel | `section-03-input-channel.md` |
| 04 | Rendering Integration | `section-04-rendering.md` |
| 05 | App-Level Refactor | `section-05-app-refactor.md` |
| 06 | Resize and Lifecycle | `section-06-resize-lifecycle.md` |
