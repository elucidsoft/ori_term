---
section: "01"
title: Shared Terminal State
status: complete
goal: Extract thread-shared state from Tab into a Mutex-protected TerminalState struct
sections:
  - id: "01.1"
    title: Add parking_lot dependency
    status: complete
  - id: "01.2"
    title: Define TerminalState struct
    status: complete
  - id: "01.3"
    title: Refactor Tab to own Arc<Mutex<TerminalState>>
    status: complete
---

# Section 01: Shared Terminal State

**Status:** Complete
**Goal:** Split Tab into thread-shared state (behind Mutex) and main-thread-only state.

---

## 01.1 Add `parking_lot` Dependency

- [x] Add `parking_lot = "0.12"` to `Cargo.toml` dependencies
- [x] Verify it compiles with `--target x86_64-pc-windows-gnu`

---

## 01.2 Define `TerminalState` Struct

Create `src/tab/terminal_state.rs` containing all state that both the PTY thread and main thread need access to.

**Moves INTO `TerminalState`** (shared between threads):
```rust
pub struct TerminalState {
    // Grids
    pub primary_grid: Grid,
    pub alt_grid: Grid,
    pub active_is_alt: bool,

    // VTE parsers (only PTY thread uses these, but they live with grid state)
    processor: vte::ansi::Processor,
    raw_parser: vte::Parser,
    grapheme_state: GraphemeState,

    // Terminal state that VTE parsing mutates AND rendering reads
    pub palette: Palette,
    pub mode: TermMode,
    pub cursor_shape: CursorShape,
    pub charset: CharsetState,
    pub title: String,
    pub title_stack: Vec<String>,
    pub has_explicit_title: bool,
    pub suppress_title: bool,
    pub keyboard_mode_stack: Vec<KeyboardModes>,
    pub inactive_keyboard_mode_stack: Vec<KeyboardModes>,

    // Bell state (PTY sets, renderer reads)
    pub bell_start: Option<Instant>,

    // Shell integration (PTY sets, main thread reads)
    pub cwd: Option<String>,
    pub prompt_state: PromptState,
    pub pending_notifications: Vec<Notification>,
    prompt_mark_pending: bool,

    // Dirty flag (PTY sets, main thread reads and clears)
    pub grid_dirty: bool,
}
```

**Stays on `Tab`** (main-thread only):
```rust
pub struct Tab {
    pub id: TabId,
    pub terminal: Arc<Mutex<TerminalState>>,

    // PTY handles (main thread for shutdown, channel for input)
    pub pty_master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    input_tx: Sender<TabMsg>,  // Lock-free input channel

    // UI state (main thread only — never accessed by PTY thread)
    pub selection: Option<Selection>,
    pub search: Option<SearchState>,
    pub has_bell_badge: bool,
}
```

- [x] Create `src/tab/terminal_state.rs`
- [x] Define `TerminalState` with all shared fields
- [x] Implement `TerminalState::new()` constructor
- [x] Move `process_output()` method to `TerminalState`
- [x] Move `grid()` / `grid_mut()` accessors to `TerminalState`
- [x] Add convenience methods: `active_grid(&self) -> &Grid`, `active_grid_mut(&mut self) -> &mut Grid`

**Design note**: `pty_writer` does NOT go into `TerminalState`. It moves to the PTY thread (see Section 02). The main thread sends input bytes through a channel instead.

---

## 01.3 Refactor `Tab` to Own `Arc<Mutex<TerminalState>>`

- [x] Change `Tab` struct to hold `pub terminal: Arc<Mutex<TerminalState>>`
- [x] Remove moved fields from `Tab`
- [x] Update `Tab::spawn()` to create `TerminalState` and wrap in `Arc<Mutex<>>`
- [x] Clone `Arc` for the reader thread (passed at spawn time)
- [x] Update all `Tab` methods that delegate to terminal state:
  - `tab.grid()` → `tab.terminal.lock().active_grid()` (but callers should hold the lock themselves)
  - `tab.process_output()` → moved entirely to PTY thread
  - `tab.scroll_*()` → lock terminal, modify display_offset
  - `tab.resize()` → lock terminal, resize grids + PTY
  - `tab.effective_title()` → lock terminal, read title/cwd
  - `tab.apply_color_config()` → lock terminal, modify palette
  - `tab.set_cursor_shape()` → lock terminal, modify cursor_shape
  - `tab.navigate_to_*_prompt()` → lock terminal, modify display_offset

- [x] `Tab::shutdown()` stays as-is (closes PTY handles, kills child)

---

## 01.N Completion Checklist

- [x] `TerminalState` defined with correct field set
- [x] `Tab` holds `Arc<Mutex<TerminalState>>`
- [x] `process_output()` is a method on `TerminalState` (not `Tab`)
- [x] All Tab methods that access grid/palette/mode go through the lock
- [x] Code compiles (not necessarily correct yet — threading comes in Section 02)
- [x] `cargo clippy --target x86_64-pc-windows-gnu` passes

**Exit Criteria:** Tab's state is split, TerminalState is behind Arc<Mutex<>>, everything compiles. PTY thread is not yet changed — process_output() still called from event loop. This is a pure refactor step.

---

## Deviations from Plan

1. **`pty_writer` stays on Tab for now** (not moved to channel yet). `TerminalState::process_output()` takes `pty_writer: &mut Option<Box<dyn Write + Send>>` as a parameter so the VTE handler can write responses back to the PTY.
2. **`input_tx` channel not added yet** — planned for Section 03. Tab still holds `pty_writer` directly.
3. **`effective_title()` returns `String` instead of `Cow<'_, str>`** — can't borrow through MutexGuard across the API boundary.
4. **Convenience methods on Tab** (e.g., `grid()`, `mode()`, `bell_start()`, `cwd()`, `grid_dirty()`) lock internally and return owned/copied values, keeping the API ergonomic for callers.
5. **`tab.grid()` returns `MappedMutexGuard<'_, Grid>`** — callers pass `&tab.grid()` to functions expecting `&Grid` (deref coercion through the borrow).
6. **Removed dead `pty_buffers` field from App** (Broken Window Policy — pre-existing dead code).

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented.
