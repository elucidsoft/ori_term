---
section: "02"
title: PTY Processing Thread
status: not-started
goal: Move VTE parsing from main thread to per-tab reader thread
sections:
  - id: "02.1"
    title: Implement FairMutex
    status: not-started
  - id: "02.2"
    title: Refactor reader thread to parse VTE
    status: not-started
  - id: "02.3"
    title: Remove PtyOutput event
    status: not-started
---

# Section 02: PTY Processing Thread

**Status:** Not Started
**Goal:** Each tab's reader thread reads PTY bytes, acquires the terminal lock, and does VTE parsing. Main thread never calls `process_output()`.

---

## 02.1 Implement FairMutex

Directly adapted from Alacritty's `sync.rs`. A fair mutex ensures the PTY thread isn't starved by the renderer holding the lock.

```rust
// src/sync.rs
use parking_lot::{Mutex, MutexGuard};

pub struct FairMutex<T> {
    data: Mutex<T>,
    next: Mutex<()>,
}

impl<T> FairMutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Reserve the next lock slot (call before reading PTY).
    pub fn lease(&self) -> MutexGuard<'_, ()> {
        self.next.lock()
    }

    /// Lock with fairness (waits for lease holder first).
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let _next = self.next.lock();
        self.data.lock()
    }

    /// Lock without fairness (fast path for renderer).
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Non-blocking lock attempt (PTY thread yields if locked).
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }
}
```

- [ ] Create `src/sync.rs`
- [ ] Implement `FairMutex<T>` with `lease()`, `lock()`, `lock_unfair()`, `try_lock_unfair()`
- [ ] Add `mod sync` to `src/main.rs` / `src/lib.rs`
- [ ] Update `Tab.terminal` type from `Arc<Mutex<TerminalState>>` to `Arc<FairMutex<TerminalState>>`
- [ ] Unit test: verify fairness (lease holder gets priority)

---

## 02.2 Refactor Reader Thread

The current `spawn_reader_thread` reads bytes and sends `TermEvent::PtyOutput(id, bytes)`. Change it to read bytes, lock the terminal, and parse VTE in-place.

**Current** (`tab/mod.rs:450-476`):
```rust
fn spawn_reader_thread(id: TabId, mut reader: Box<dyn Read + Send>, proxy: EventLoopProxy<TermEvent>) {
    thread::spawn(move || {
        let mut buf = vec![0u8; 65536];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = proxy.send_event(TermEvent::PtyExited(id));
                    break;
                }
                Ok(n) => {
                    let _ = proxy.send_event(TermEvent::PtyOutput(id, buf[..n].to_vec()));
                }
            }
        }
    });
}
```

**After**:
```rust
fn spawn_reader_thread(
    id: TabId,
    mut reader: Box<dyn Read + Send>,
    terminal: Arc<FairMutex<TerminalState>>,
    mut pty_writer: Box<dyn Write + Send>,
    input_rx: Receiver<TabMsg>,
    proxy: EventLoopProxy<TermEvent>,
) {
    thread::spawn(move || {
        let mut buf = vec![0u8; READ_BUFFER_SIZE];  // 1MB
        let mut unprocessed = 0;

        loop {
            // Reserve next lock slot before reading
            let _lease = terminal.lease();

            match reader.read(&mut buf[unprocessed..]) {
                Ok(0) => {
                    let _ = proxy.send_event(TermEvent::PtyExited(id));
                    break;
                }
                Err(e) => {
                    log(&format!("reader error for tab {:?}: {e}", id));
                    let _ = proxy.send_event(TermEvent::PtyExited(id));
                    break;
                }
                Ok(n) => {
                    unprocessed += n;

                    // Try to acquire terminal lock
                    let mut term = match terminal.try_lock_unfair() {
                        Some(t) => t,
                        None if unprocessed >= READ_BUFFER_SIZE => {
                            // Buffer full — force lock
                            terminal.lock_unfair()
                        }
                        None => continue,  // Yield, read more
                    };

                    // Parse VTE under lock
                    term.process_output(&buf[..unprocessed]);
                    unprocessed = 0;

                    // Drop lock before signaling
                    drop(term);
                    drop(_lease);

                    // Drain input channel and write to PTY
                    while let Ok(msg) = input_rx.try_recv() {
                        match msg {
                            TabMsg::Input(bytes) => {
                                let _ = pty_writer.write_all(&bytes);
                                let _ = pty_writer.flush();
                            }
                            TabMsg::Resize(size) => {
                                // Resize handled by main thread via pty_master
                                // (this is just the write-side signal)
                            }
                            TabMsg::Shutdown => return,
                        }
                    }

                    // Signal main thread to redraw
                    let _ = proxy.send_event(TermEvent::Wakeup(id));
                }
            }
        }
    });
}
```

**Constants**:
```rust
const READ_BUFFER_SIZE: usize = 0x10_0000;  // 1MB — max before forced lock
const MAX_LOCKED_READ: usize = 65_536;      // 64KB — max per lock hold
```

- [ ] Update `spawn_reader_thread` signature to accept `Arc<FairMutex<TerminalState>>`, `pty_writer`, `input_rx`
- [ ] Implement lease → try_lock → parse → drop lock → signal pattern
- [ ] Drain input channel after VTE processing (write to PTY stdin)
- [ ] Send `TermEvent::Wakeup(id)` instead of `PtyOutput`
- [ ] Handle `TabMsg::Shutdown` for clean thread termination

---

## 02.3 Remove `PtyOutput` Event

- [ ] Add `TermEvent::Wakeup(TabId)` variant
- [ ] Remove `TermEvent::PtyOutput(TabId, Vec<u8>)` variant
- [ ] Update `event_loop.rs` `user_event`:
  - `Wakeup(tab_id)`: check title changes, bell state, URL invalidation, queue redraw
  - No more `process_output()` call — PTY thread already did it
  - Read title/bell/dirty state through the lock

**New `user_event` handler for Wakeup**:
```rust
TermEvent::Wakeup(tab_id) => {
    self.pty_event_count += 1;
    self.cursor_blink_reset = Instant::now();

    // Read state through lock (brief hold)
    let (title_changed, has_bell, notifications) = {
        if let Some(tab) = self.tabs.get(&tab_id) {
            let term = tab.terminal.lock();
            let new_title = term.effective_title();
            let old = self.cached_title_for(tab_id);
            let changed = old.as_deref() != Some(new_title.as_ref());
            let bell = term.bell_start.is_some();
            let notifs = /* drain needs mut, handle separately */;
            (changed, bell, notifs)
        } else {
            return;
        }
    };

    if title_changed { self.tab_bar_dirty = true; }
    // ... bell badge logic, notifications, queue redraw
}
```

- [ ] Remove all `pty_bytes_received` tracking (bytes no longer pass through event loop)
- [ ] Update stats logging to reflect new architecture

---

## 02.N Completion Checklist

- [ ] FairMutex implemented and tested
- [ ] Reader thread does VTE parsing under lock
- [ ] `TermEvent::PtyOutput` removed
- [ ] `TermEvent::Wakeup` triggers redraw without VTE parsing
- [ ] No `process_output()` calls on main thread
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` passes
- [ ] `cargo test` passes

**Exit Criteria:** PTY data is parsed on the reader thread. Main thread never touches VTE parser. Wakeup event triggers rendering only.

---

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented.
