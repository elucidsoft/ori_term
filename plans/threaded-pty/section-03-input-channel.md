---
section: "03"
title: Lock-Free Input Channel
status: not-started
goal: Replace direct pty_writer calls with a lock-free mpsc channel
sections:
  - id: "03.1"
    title: Define TabMsg enum
    status: not-started
  - id: "03.2"
    title: Replace all send_pty calls
    status: not-started
  - id: "03.3"
    title: PTY thread drains input channel
    status: not-started
---

# Section 03: Lock-Free Input Channel

**Status:** Not Started
**Goal:** Main thread sends input bytes through an mpsc channel. PTY thread drains the channel and writes to PTY stdin. Zero lock contention for input.

**NO DEFERRALS.** Every single `send_pty` call site gets converted. No "we'll handle paste later" or "mouse reporting can wait." Every input path goes through the channel.

---

## 03.1 Define `TabMsg` Enum

```rust
// src/tab/types.rs (extend existing)
pub enum TabMsg {
    /// Raw bytes to write to PTY stdin (keyboard, mouse report, paste, focus).
    Input(Vec<u8>),
    /// Resize the PTY (cols, rows, pixel dimensions).
    Resize { cols: u16, rows: u16, pixel_width: u16, pixel_height: u16 },
    /// Shut down the reader thread cleanly.
    Shutdown,
}
```

- [ ] Add `TabMsg` enum to `src/tab/types.rs`
- [ ] Export from `src/tab/mod.rs`

---

## 03.2 Replace ALL `send_pty()` Call Sites

The current `tab.send_pty(&bytes)` writes directly to `pty_writer`. Replace with `tab.send_input(&bytes)` which pushes to the channel.

**New method on `Tab`**:
```rust
impl Tab {
    pub fn send_input(&self, data: &[u8]) {
        if !data.is_empty() {
            let _ = self.input_tx.send(TabMsg::Input(data.to_vec()));
        }
    }
}
```

**Every call site** (exhaustive list — NO OMISSIONS):

| # | File | Line | Context | Change |
|---|------|------|---------|--------|
| 1 | `event_loop.rs` | 354 | Keyboard input → PTY | `tab.send_pty(&bytes)` → `tab.send_input(&bytes)` |
| 2 | `event_loop.rs` | 378-380 | Focus in/out sequence | `tab.send_pty(seq)` → `tab.send_input(seq)` |
| 3 | `mouse_report.rs` | ~48 | SGR mouse report | `tab.send_pty(&buf[..=pos])` → `tab.send_input(&buf[..=pos])` |
| 4 | `mouse_report.rs` | ~81 | Normal mouse report | Same pattern |
| 5 | `mouse_report.rs` | ~88 | UTF-8 mouse report | Same pattern |
| 6 | `mod.rs` | 391-395 | Paste (bracketed) | `tab.send_pty(...)` × 3 → `tab.send_input(...)` × 3 |
| 7 | `input_keyboard.rs` | ~134 | Text input | `tab.send_pty(text.as_bytes())` → `tab.send_input(text.as_bytes())` |
| 8 | `input_mouse.rs` | ~395 | Mouse wheel alt-screen | `tab.send_pty(seq)` → `tab.send_input(seq)` |

- [ ] Add `input_tx: std::sync::mpsc::Sender<TabMsg>` to `Tab`
- [ ] Implement `Tab::send_input(&self, data: &[u8])`
- [ ] Convert call site #1: keyboard input (event_loop.rs)
- [ ] Convert call site #2: focus in/out (event_loop.rs)
- [ ] Convert call site #3: SGR mouse report (mouse_report.rs)
- [ ] Convert call site #4: normal mouse report (mouse_report.rs)
- [ ] Convert call site #5: UTF-8 mouse report (mouse_report.rs)
- [ ] Convert call site #6: paste (mod.rs)
- [ ] Convert call site #7: text input (input_keyboard.rs)
- [ ] Convert call site #8: mouse wheel (input_mouse.rs)
- [ ] Remove `Tab::send_pty()` method entirely
- [ ] Remove `pub pty_writer: Option<Box<dyn Write + Send>>` from `Tab`
- [ ] `grep -rn "send_pty\|pty_writer" src/` returns zero hits outside of PTY thread code

---

## 03.3 PTY Thread Drains Input Channel

The reader thread (Section 02) already drains `input_rx` after VTE processing. This section ensures:

- [ ] Input channel is drained on every loop iteration (not just after successful reads)
- [ ] `TabMsg::Resize` calls `pty_master.resize()` (pty_master moved to reader thread)
- [ ] `TabMsg::Shutdown` causes clean thread exit
- [ ] Channel is bounded or unbounded? Use unbounded — input bursts are small and rare
- [ ] On channel disconnect (sender dropped), reader thread exits

**Resize path**: Currently `Tab::resize()` calls `self.pty_master.resize()`. After this change, `pty_master` stays on `Tab` (only the writer moves to the thread). Resize goes through the channel as a coordination message, but the actual ioctl is done by whoever holds `pty_master`. Two options:

**Option A** (simpler): Keep `pty_master` on Tab, main thread calls `pty_master.resize()` directly. Grid resize happens under the terminal lock. PTY resize is a separate OS call.

**Option B** (cleaner): Move `pty_master` to the reader thread. Send `TabMsg::Resize` through channel. Reader thread calls `pty_master.resize()`.

**Decision**: Option A. `pty_master.resize()` is an OS ioctl — it doesn't need the terminal lock and doesn't race with VTE parsing. The grids are resized under the terminal lock by the main thread. This avoids moving `pty_master` ownership.

- [ ] `Tab::resize()` acquires terminal lock, resizes grids, then calls `pty_master.resize()` separately
- [ ] No `TabMsg::Resize` needed (simplification)

---

## 03.N Completion Checklist

- [ ] `TabMsg` enum defined
- [ ] `Tab::send_input()` uses channel
- [ ] ALL 8+ call sites converted (zero remaining `send_pty` calls)
- [ ] `pty_writer` moved to reader thread (not on Tab or TerminalState)
- [ ] `grep -rn "send_pty" src/` → zero hits
- [ ] `grep -rn "pty_writer" src/` → only in reader thread code
- [ ] Input works: keyboard, paste, mouse reporting, focus events
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` passes
- [ ] `cargo test` passes

**Exit Criteria:** All input flows through the channel. PTY writer is owned by the reader thread. Zero direct PTY writes from the main thread.

---

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented.
