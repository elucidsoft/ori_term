---
section: "06"
title: Resize and Lifecycle
status: not-started
goal: Thread-safe resize, clean shutdown, and end-to-end validation
sections:
  - id: "06.1"
    title: Thread-safe resize
    status: not-started
  - id: "06.2"
    title: Clean shutdown
    status: not-started
  - id: "06.3"
    title: End-to-end validation
    status: not-started
  - id: "06.4"
    title: Remove dead code
    status: not-started
---

# Section 06: Resize and Lifecycle

**Status:** Not Started
**Goal:** Resize is thread-safe. Shutdown is clean (no leaked threads, no panics). Full validation. Dead code eliminated.

**NO HALF MEASURES.** Every lifecycle edge case handled. Every thread joined or signaled. Every resource cleaned up.

---

## 06.1 Thread-Safe Resize

**Current flow**: Main thread calls `tab.resize(cols, rows, pw, ph)` which resizes both grids and calls `pty_master.resize()`.

**After threading**: Grid resize mutates `TerminalState` — needs the lock. PTY resize is a separate OS call on `pty_master` (which stays on Tab).

```rust
impl Tab {
    pub fn resize(&self, cols: usize, rows: usize, pixel_width: u16, pixel_height: u16) {
        // Lock terminal, resize grids
        {
            let mut term = self.terminal.lock();
            term.primary_grid.resize(cols, rows, true);  // reflow
            term.alt_grid.resize(cols, rows, false);       // no reflow
        }
        // PTY resize (no lock needed — pty_master is on Tab)
        let _ = self.pty_master.resize(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width,
            pixel_height,
        });
    }
}
```

**Race condition analysis**: PTY thread might be mid-VTE-parse when resize happens. After resize, the cursor position might be out of bounds. This is the same race every terminal has — VTE parsers handle it by clamping cursor to grid dimensions. Alacritty has the same pattern. The resize lock acquisition will wait for the current VTE parse chunk to finish (≤64KB), then resize, then the next parse chunk sees the new dimensions.

- [ ] `Tab::resize()` acquires terminal lock for grid resize
- [ ] `pty_master.resize()` called outside the lock (separate OS call)
- [ ] Verify: cursor clamping after resize (already in `Grid::resize()`)
- [ ] Verify: display_offset clamped to scrollback length after resize
- [ ] Verify: scroll regions reset on resize (already in `Grid::resize()`)

---

## 06.2 Clean Shutdown

**Current flow**: `Tab::shutdown()` closes `pty_writer` and kills child. Reader thread sees EOF and exits.

**After threading**: Reader thread owns `pty_writer` and `input_rx`. Shutdown must:
1. Send `TabMsg::Shutdown` through the channel (signals reader thread to exit)
2. Drop `input_tx` (closes channel — reader thread exits if it's blocked on recv)
3. Kill child process (backup in case shell ignores EOF)
4. Reader thread exits, releasing its `Arc<FairMutex<TerminalState>>`
5. Tab is dropped, releasing its `Arc` — TerminalState is freed

```rust
impl Tab {
    pub fn shutdown(&mut self) {
        // Signal reader thread to stop
        let _ = self.input_tx.send(TabMsg::Shutdown);
        // Kill child (backup — ConPTY may not deliver EOF)
        let _ = self.child.kill();
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
        // Ensure shutdown was called
        let _ = self.input_tx.send(TabMsg::Shutdown);
    }
}
```

**Reader thread shutdown**:
```rust
// In reader thread loop:
match msg {
    TabMsg::Shutdown => {
        // Close writer (child sees EOF on stdin)
        drop(pty_writer);
        return;
    }
    // ...
}

// Also exit on channel disconnect:
if input_rx.try_recv() == Err(TryRecvError::Disconnected) {
    return;
}
```

- [ ] `Tab::shutdown()` sends `TabMsg::Shutdown` and kills child
- [ ] Reader thread handles `Shutdown` message (drops writer, returns)
- [ ] Reader thread exits on channel disconnect (sender dropped)
- [ ] `Drop` impl on Tab sends shutdown as safety net
- [ ] Verify: no leaked threads after closing all tabs
- [ ] Verify: no deadlock on shutdown (reader thread doesn't hold terminal lock when receiving Shutdown)
- [ ] Verify: `PtyExited` event still works (child waiter thread still functions)

---

## 06.3 End-to-End Validation

Build, run, and verify every user-facing feature:

- [ ] **Basic output**: `echo "hello world"` renders correctly
- [ ] **High throughput**: `seq 1 1000000` or `cat /dev/urandom | base64` — UI stays responsive
- [ ] **Keyboard input**: Type characters, they appear in shell
- [ ] **Special keys**: Ctrl+C, Ctrl+D, arrow keys, function keys
- [ ] **Paste**: Ctrl+V pastes from clipboard (including bracketed paste)
- [ ] **Mouse**: Click, drag selection, scroll wheel
- [ ] **Mouse reporting**: `htop`, `vim` — mouse events reach applications
- [ ] **Resize**: Drag window border — grid reflows, no corruption
- [ ] **Scrollback**: Scroll up/down through history
- [ ] **Search**: Ctrl+F search works, highlights matches
- [ ] **Selection**: Single click, double-click (word), triple-click (line)
- [ ] **Tab management**: New tab, close tab, switch tabs
- [ ] **Multiple windows**: Tabs in different windows
- [ ] **Bell**: Visual bell flashes, badge on inactive tab
- [ ] **Title**: Shell-set title shows in tab bar
- [ ] **CWD**: Working directory updates in tab bar
- [ ] **Alt screen**: `vim`, `less`, `htop` switch to alt screen correctly
- [ ] **URL detection**: URLs highlighted on hover
- [ ] **Color schemes**: Colors render correctly
- [ ] **Focus events**: Focus in/out sequences sent to apps that request them
- [ ] **Shell integration**: Prompt navigation (Ctrl+Shift+Up/Down)
- [ ] **Config reload**: Hot reload applies changes without restart
- [ ] **Clean exit**: Close window, all threads terminate, no panics

### Performance validation:
- [ ] **Flood test**: `time seq 1 10000000` — completes without UI freezing, time is similar to Alacritty
- [ ] **Latency**: Typing feels instant (no perceptible delay from lock contention)
- [ ] **Stats logging**: Check periodic stats — render rate stays at ~120fps during normal use
- [ ] **Memory**: No growing memory from leaked buffers or unjoined threads

---

## 06.4 Remove Dead Code

After the full refactor, clean up:

- [ ] Remove `pty_buffers: HashMap<TabId, Vec<u8>>` from App (no longer needed)
- [ ] Remove `TermEvent::PtyOutput` variant
- [ ] Remove `pty_bytes_received` stat (bytes don't flow through event loop)
- [ ] Remove any `process_output` method from Tab (it's on TerminalState now)
- [ ] Remove `pub pty_writer` from Tab struct
- [ ] `grep -rn "PtyOutput" src/` → ZERO hits
- [ ] `grep -rn "pty_buffers" src/` → ZERO hits
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` passes with zero warnings (beyond pre-existing clipboard.rs warning)
- [ ] `cargo test` passes
- [ ] No dead code warnings

---

## 06.N Completion Checklist

- [ ] Resize is thread-safe (grids resized under lock, PTY resized separately)
- [ ] Shutdown is clean (no leaked threads, no panics, no deadlocks)
- [ ] Every feature validated end-to-end
- [ ] Performance validated (flood test, latency, stats)
- [ ] Dead code removed
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes
- [ ] `./build-all.sh` succeeds

**Exit Criteria:** The threaded PTY architecture is complete, correct, performant, and clean. Every feature works. Every thread terminates cleanly. Zero dead code.

---

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented. This is the final section — when it's done, update `00-overview.md` with a completion summary.
