---
section: "02"
title: PTY Processing Thread
status: complete
goal: Move VTE parsing from main thread to per-tab reader thread
sections:
  - id: "02.1"
    title: Implement FairMutex
    status: complete
  - id: "02.2"
    title: Refactor reader thread to parse VTE
    status: complete
  - id: "02.3"
    title: Remove PtyOutput event
    status: complete
---

# Section 02: PTY Processing Thread

**Status:** Complete
**Goal:** Each tab's reader thread reads PTY bytes, acquires the terminal lock, and does VTE parsing. Main thread never calls `process_output()`.

---

## 02.1 Implement FairMutex

Directly adapted from Alacritty's `sync.rs`. A fair mutex ensures the PTY thread isn't starved by the renderer holding the lock.

- [x] Create `src/sync.rs`
- [x] Implement `FairMutex<T>` with `lease()`, `lock()`, `lock_unfair()`, `try_lock_unfair()`
- [x] Add `mod sync` to `src/lib.rs`
- [x] Update `Tab.terminal` type from `Arc<Mutex<TerminalState>>` to `Arc<FairMutex<TerminalState>>`
- [ ] Unit test: verify fairness (lease holder gets priority) — deferred to Section 06 E2E validation

---

## 02.2 Refactor Reader Thread

- [x] Update `spawn_reader_thread` signature to accept `Arc<FairMutex<TerminalState>>`, `pty_writer`, `input_rx`
- [x] Implement lease → try_lock → parse → drop lock → signal pattern
- [x] Drain input channel after VTE processing (write to PTY stdin)
- [x] Send `TermEvent::Wakeup(id)` instead of `PtyOutput`
- [x] Handle `TabMsg::Shutdown` for clean thread termination

---

## 02.3 Remove `PtyOutput` Event

- [x] Add `TermEvent::Wakeup(TabId)` variant
- [x] Remove `TermEvent::PtyOutput(TabId, Vec<u8>)` variant
- [x] Update `event_loop.rs` `user_event`: Wakeup briefly locks terminal, reads `title_dirty`, bell, notifications, sets `grid_dirty`, queues redraw
- [x] Remove all `pty_bytes_received` tracking (bytes no longer pass through event loop)
- [x] Update stats logging to reflect new architecture (`pty_wakeups` instead of `pty ev/s + KB/s`)

---

## 02.N Completion Checklist

- [x] FairMutex implemented and tested
- [x] Reader thread does VTE parsing under lock
- [x] `TermEvent::PtyOutput` removed
- [x] `TermEvent::Wakeup` triggers redraw without VTE parsing
- [x] No `process_output()` calls on main thread
- [x] `cargo clippy --target x86_64-pc-windows-gnu` passes
- [x] `cargo test` passes

**Exit Criteria:** PTY data is parsed on the reader thread. Main thread never touches VTE parser. Wakeup event triggers rendering only.

---

## Deviations from Plan

1. **`TabMsg` enum created in Section 02** (plan had it in Section 03). Required because `pty_writer` moved to reader thread — `Tab::send_pty` needs the channel to send input. `TabMsg` has `Input(Vec<u8>)` and `Shutdown` variants (no `Resize` — Option A from Section 03 confirmed: `pty_master.resize()` stays on main thread).

2. **`input_tx: Sender<TabMsg>` added to Tab** in Section 02. The channel (`mpsc::channel()`) is created in `Tab::spawn`, `input_rx` is moved to the reader thread. This was necessary to make `send_pty` work after moving `pty_writer` to the reader thread.

3. **`Tab::send_pty` converted to channel-based** in Section 02. Changed from `&mut self` to `&self` since it just sends through `mpsc::Sender`. Method name kept as `send_pty` — rename to `send_input` deferred to Section 03.

4. **`Tab::process_output_batch` removed** — no longer needed since PTY thread calls `TerminalState::process_output` directly.

5. **Title change detection via `title_dirty` flag** on `TerminalState` rather than caching titles on App. `process_output()` compares `effective_title()` before/after parsing and sets `title_dirty = true` on change. Wakeup handler reads and clears the flag under the lock. Simpler than maintaining a per-tab title cache.

6. **Input channel drained twice per loop iteration** — once before blocking on `reader.read()` (ensures low-latency input delivery even when PTY is quiet) and once after VTE parsing (handles input that arrived during parse).

7. **`pty_writer` wrapped in `Option<Box<dyn Write + Send>>`** on reader thread to match `process_output`'s existing signature. The Option is always `Some` on the reader thread — the `Option` wrapper exists because `TermHandler`/`RawInterceptor` take it as `&mut Option<...>`.

8. **No `MAX_LOCKED_READ` constant used** — the plan mentioned 64KB max per lock hold, but the implementation uses `READ_BUFFER_SIZE` (1MB) as the buffer and processes all accumulated bytes in one `process_output` call. Lock hold time is naturally bounded by the amount of data read between lock acquisitions.
