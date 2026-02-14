---
section: 4
title: PTY + Event Loop
status: not-started
tier: 1
goal: Spawn a shell via ConPTY, wire the reader thread, and verify end-to-end I/O through Term<EventProxy>
sections:
  - id: "4.1"
    title: Binary Crate Setup
    status: not-started
  - id: "4.2"
    title: TabId + TermEvent Types
    status: not-started
  - id: "4.3"
    title: PTY Spawning
    status: not-started
  - id: "4.4"
    title: Message Channel
    status: not-started
  - id: "4.5"
    title: EventProxy (EventListener impl)
    status: not-started
  - id: "4.6"
    title: Notifier (Notify impl)
    status: not-started
  - id: "4.7"
    title: PTY Reader Thread
    status: not-started
  - id: "4.8"
    title: Tab Struct
    status: not-started
  - id: "4.9"
    title: End-to-End Verification
    status: not-started
  - id: "4.10"
    title: Section Completion
    status: not-started
---

# Section 04: PTY + Event Loop

**Status:** 📋 Planned
**Goal:** Spawn a real shell, wire PTY I/O through the reader thread, and process shell output through `Term<EventProxy>`. This is the first time terminal emulation runs against a live shell process.

**Crate:** `oriterm` (binary)
**Dependencies:** `oriterm_core`, `portable-pty`, `winit` (for `EventLoopProxy` type — window not created yet)

---

## 4.1 Binary Crate Setup

Set up the `oriterm/` binary crate in the workspace.

- [ ] Create `oriterm/` directory with `Cargo.toml` and `src/main.rs`
  - [ ] `Cargo.toml`: name = `oriterm`, edition = 2024, same lint config
  - [ ] Dependencies: `oriterm_core = { path = "../oriterm_core" }`, all GUI/platform deps from current root Cargo.toml
  - [ ] `[[bin]]` name = `oriterm`, path = `src/main.rs`
- [ ] Move existing `src/main.rs` → `oriterm/src/main.rs`
- [ ] Move `build.rs` → `oriterm/build.rs`
- [ ] Move `assets/` reference in build.rs (update paths)
- [ ] Update workspace root `Cargo.toml`:
  - [ ] `[workspace]` with `members = ["oriterm_core", "oriterm"]`
  - [ ] Remove `[[bin]]` and `[dependencies]` from root (they live in crate-level Cargo.tomls now)
- [ ] Verify: `cargo build --target x86_64-pc-windows-gnu` builds both crates
- [ ] Verify: `cargo build -p oriterm --target x86_64-pc-windows-gnu` builds the binary

---

## 4.2 TabId + TermEvent Types

Newtype for tab identity and the event type for cross-thread communication.

**File:** `oriterm/src/tab.rs` (initial, will grow)

- [ ] `TabId` newtype
  - [ ] `pub struct TabId(pub u64)`
  - [ ] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`
  - [ ] `TabId::next() -> Self` — atomic counter for unique IDs
    - [ ] Use `std::sync::atomic::AtomicU64` static counter
- [ ] `TermEvent` enum — winit user event type
  - [ ] `Terminal { tab_id: TabId, event: oriterm_core::Event }` — event from terminal library
  - [ ] Derive: `Debug`
- [ ] **Tests**:
  - [ ] `TabId::next()` generates unique IDs
  - [ ] `TermEvent` variants can be constructed

---

## 4.3 PTY Spawning

Create a PTY and spawn the default shell.

**File:** `oriterm/src/pty/spawn.rs`

- [ ] `spawn_shell(rows: u16, cols: u16) -> io::Result<PtyHandle>`
  - [ ] Call `portable_pty::native_pty_system()`
  - [ ] `pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`
  - [ ] `CommandBuilder::new_default_prog()` — default shell
  - [ ] `pair.slave.spawn_command(cmd)` — spawn child process
  - [ ] Drop `pair.slave` (reader gets EOF when child exits)
  - [ ] Clone reader: `pair.master.try_clone_reader()`
  - [ ] Take writer: `pair.master.take_writer()`
  - [ ] Return `PtyHandle` containing reader, writer, master, child
- [ ] `PtyHandle` struct
  - [ ] Fields:
    - `reader: Box<dyn Read + Send>` — PTY output (read by reader thread)
    - `writer: Box<dyn Write + Send>` — PTY input (written by Notifier)
    - `master: Box<dyn portable_pty::MasterPty + Send>` — for resize
    - `child: Box<dyn portable_pty::Child + Send + Sync>` — child process handle
  - [ ] `PtyHandle::resize(&self, rows: u16, cols: u16) -> io::Result<()>`
    - [ ] `self.master.resize(PtySize { rows, cols, ... })`
- [ ] `mod.rs`: `pub mod spawn;` re-export `PtyHandle`, `spawn_shell`
- [ ] **Tests**:
  - [ ] Spawning a shell succeeds (integration test, may need `#[cfg(target_os)]` gate)
  - [ ] Reader and writer are valid (not None)

---

## 4.4 Message Channel

Messages from the main thread to the PTY reader thread.

**File:** `oriterm/src/pty/mod.rs`

- [ ] `Msg` enum — commands sent to PTY thread
  - [ ] `Input(Vec<u8>)` — bytes to write to PTY
  - [ ] `Resize { rows: u16, cols: u16 }` — resize the PTY
  - [ ] `Shutdown` — gracefully stop the reader thread
- [ ] Use `std::sync::mpsc::channel::<Msg>()` — unbounded channel
  - [ ] Sender held by `Notifier` (main thread side)
  - [ ] Receiver consumed by reader thread

---

## 4.5 EventProxy (EventListener impl)

Bridges terminal events to the winit event loop.

**File:** `oriterm/src/tab.rs`

- [ ] `EventProxy` struct
  - [ ] Fields:
    - `proxy: winit::event_loop::EventLoopProxy<TermEvent>` — winit's thread-safe event sender
    - `tab_id: TabId`
  - [ ] `impl oriterm_core::EventListener for EventProxy`
    - [ ] `fn send_event(&self, event: oriterm_core::Event)`
      - [ ] `let _ = self.proxy.send_event(TermEvent::Terminal { tab_id: self.tab_id, event });`
      - [ ] Silently ignore send errors (window may have closed)
- [ ] `EventProxy` must be `Send + 'static` (required by `EventListener` bound)

---

## 4.6 Notifier (Notify impl)

Sends input bytes and commands to the PTY reader thread.

**File:** `oriterm/src/tab.rs`

- [ ] `Notifier` struct
  - [ ] Fields:
    - `tx: std::sync::mpsc::Sender<Msg>` — channel sender
  - [ ] `impl oriterm_core::Notify for Notifier`
    - [ ] `fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B)`
      - [ ] `let _ = self.tx.send(Msg::Input(bytes.into().into_owned()));`
  - [ ] `Notifier::resize(&self, rows: u16, cols: u16)`
    - [ ] `let _ = self.tx.send(Msg::Resize { rows, cols });`
  - [ ] `Notifier::shutdown(&self)`
    - [ ] `let _ = self.tx.send(Msg::Shutdown);`

---

## 4.7 PTY Reader Thread

The dedicated thread that reads PTY output, parses VTE, and updates terminal state.

**File:** `oriterm/src/pty/mod.rs`

- [ ] `PtyEventLoop` struct
  - [ ] Fields:
    - `terminal: Arc<oriterm_core::FairMutex<oriterm_core::Term<EventProxy>>>` — shared terminal state
    - `reader: Box<dyn Read + Send>` — PTY read handle
    - `writer: Box<dyn Write + Send>` — PTY write handle
    - `rx: std::sync::mpsc::Receiver<Msg>` — command receiver
    - `pty_master: Box<dyn portable_pty::MasterPty + Send>` — for resize
    - `processor: vte::ansi::Processor` — VTE parser state machine
  - [ ] `PtyEventLoop::new(...)` — constructor, takes all handles
  - [ ] `PtyEventLoop::spawn(self) -> JoinHandle<()>` — start the reader thread
    - [ ] `std::thread::Builder::new().name("pty-reader".into()).spawn(move || self.run())`
  - [ ] `fn run(mut self)` — main loop:
    ```
    loop {
        // 1. Drain command channel (non-blocking)
        self.process_commands();

        // 2. Read from PTY (blocking, with timeout or polling)
        let n = self.reader.read(&mut buf);

        // 3. If data available, lock terminal and parse
        if n > 0 {
            let _lease = self.terminal.lease();
            let mut term = self.terminal.lock_unfair();
            self.processor.advance(&mut *term, &buf[..n]);
            // Collect PTY responses (DA, CPR, etc.) and write back
            // Drop lock
        }

        // 4. Check if child exited
    }
    ```
  - [ ] `fn process_commands(&mut self)` — drain rx:
    - [ ] `Msg::Input(bytes)` → `self.writer.write_all(&bytes)`
    - [ ] `Msg::Resize { rows, cols }` → `self.pty_master.resize(...)` + lock term + `term.resize(cols, rows)`
    - [ ] `Msg::Shutdown` → break out of loop
  - [ ] Read buffer: `[u8; 65536]` (64KB, stack-allocated)
  - [ ] Max locked parse: process up to 64KB under one lock acquisition, then release and re-lock for more
    - [ ] Prevents holding lock for too long on large output bursts
- [ ] **Thread safety**:
  - [ ] PTY reader thread holds `FairMutex` lock only during `processor.advance()` (microseconds to low ms)
  - [ ] Uses `lease()` → `lock_unfair()` pattern from Alacritty
  - [ ] Releases lock between read batches

---

## 4.8 Tab Struct

Owns all per-tab state: terminal, PTY handles, reader thread.

**File:** `oriterm/src/tab.rs`

- [ ] `Tab` struct
  - [ ] Fields:
    - `id: TabId`
    - `terminal: Arc<oriterm_core::FairMutex<oriterm_core::Term<EventProxy>>>`
    - `notifier: Notifier` — send input/resize/shutdown to PTY thread
    - `reader_thread: Option<JoinHandle<()>>` — reader thread handle
    - `title: String` — last known title (updated from Event::Title)
    - `has_bell: bool` — bell badge (cleared on focus)
  - [ ] `Tab::new(id: TabId, rows: u16, cols: u16, scrollback: usize, proxy: EventLoopProxy<TermEvent>) -> io::Result<Self>`
    - [ ] Spawn PTY via `pty::spawn_shell(rows, cols)`
    - [ ] Create `EventProxy` with tab_id and proxy
    - [ ] Create `Term::new(rows, cols, scrollback, event_proxy)`
    - [ ] Wrap in `Arc<FairMutex<...>>`
    - [ ] Create `(tx, rx)` channel
    - [ ] Create `Notifier` with tx
    - [ ] Create `PtyEventLoop` with terminal clone, reader, writer, rx, master
    - [ ] Spawn reader thread: `event_loop.spawn()`
    - [ ] Return Tab
  - [ ] `Tab::write_input(&self, bytes: &[u8])` — send input to PTY via Notifier
  - [ ] `Tab::resize(&self, rows: u16, cols: u16)` — resize PTY + terminal
  - [ ] `Tab::terminal(&self) -> &Arc<FairMutex<Term<EventProxy>>>` — for renderer to lock + snapshot
  - [ ] `impl Drop for Tab`
    - [ ] Send `Msg::Shutdown` to reader thread
    - [ ] Join reader thread (with timeout)

---

## 4.9 End-to-End Verification

At this point there's no window, but we can verify the full PTY → VTE → Term pipeline.

- [ ] Temporary `main.rs` for verification:
  - [ ] Create a winit `EventLoop` (needed for `EventLoopProxy`, even without a window)
  - [ ] Create a Tab
  - [ ] Send `"echo hello\r\n"` via `tab.write_input()`
  - [ ] Wait briefly (100ms)
  - [ ] Lock terminal, read grid, verify "hello" appears in grid cells
  - [ ] Print verification result to log/stderr
  - [ ] Exit
- [ ] Verify thread lifecycle:
  - [ ] Tab creation spawns reader thread
  - [ ] Tab drop sends Shutdown and joins thread
  - [ ] No thread leaks, no panics on drop
- [ ] Verify FairMutex under load:
  - [ ] Send rapid input while reader thread is processing
  - [ ] Neither thread starves (both make progress)
- [ ] Verify resize:
  - [ ] Create tab at 80x24
  - [ ] Resize to 120x40
  - [ ] PTY dimensions updated, terminal grid resized

---

## 4.10 Section Completion

- [ ] All 4.1–4.9 items complete
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` succeeds
- [ ] Tab spawns shell, reader thread processes output into Term
- [ ] Input sent via Notifier arrives at shell
- [ ] Shutdown is clean (no thread leaks, no panics)
- [ ] FairMutex prevents starvation under concurrent access
- [ ] Resize works end-to-end (PTY + terminal grid)
- [ ] No window yet — next section adds GUI

**Exit Criteria:** Live shell output is parsed through VTE into `Term<EventProxy>`. Input flows main thread → Notifier → channel → PTY. Reader thread is clean (proper lifecycle, lock discipline, no starvation). Ready for a window to render the terminal state.
