---
section: "04"
title: Rendering Integration
status: not-started
goal: Rendering acquires terminal lock for read-only grid access
sections:
  - id: "04.1"
    title: Lock-based FrameParams construction
    status: not-started
  - id: "04.2"
    title: Dirty flag coordination
    status: not-started
  - id: "04.3"
    title: Frame budget under contention
    status: not-started
---

# Section 04: Rendering Integration

**Status:** Not Started
**Goal:** `render_window()` acquires the terminal lock, builds `FrameParams`, renders, releases. Clean, correct, complete.

**NO SHORTCUTS.** Every rendering path that touches grid state goes through the lock. No "we'll optimize later." No stale references.

---

## 04.1 Lock-Based `FrameParams` Construction

Currently `render_window()` in `render_coord.rs` builds `FrameParams` by borrowing `tab.grid()`, `tab.palette`, `tab.selection`, `tab.search`, etc. After threading, grid/palette/mode live behind the lock in `TerminalState`, while selection/search remain directly on `Tab`.

**Pattern**:
```rust
fn render_window(&mut self, window_id: WindowId) {
    let active_tab_id = self.active_tab_id(window_id);

    // ... window/surface setup ...

    let frame_params = active_tab_id.and_then(|tab_id| {
        let tab = self.tabs.get(&tab_id)?;
        let term = tab.terminal.lock();  // Acquire lock

        Some(FrameParams {
            grid: term.active_grid(),       // From locked state
            palette: &term.palette,          // From locked state
            mode: term.mode,                 // Copy (no borrow needed)
            cursor_shape: term.cursor_shape, // Copy
            selection: tab.selection.as_ref(), // From Tab (no lock)
            search: tab.search.as_ref(),      // From Tab (no lock)
            // ... rest of params ...
        })
    });

    // Problem: FrameParams borrows from MutexGuard — guard must live
    // long enough. Two approaches:
}
```

**Lifetime challenge**: `FrameParams<'a>` borrows `&'a Grid` and `&'a Palette`. These come from `MutexGuard<TerminalState>`. The guard must outlive the `FrameParams`. Solution: hold the guard in a local variable that outlives `draw_frame()`.

```rust
fn render_window(&mut self, window_id: WindowId) {
    // ... setup ...

    let tab_id = match self.active_tab_id(window_id) {
        Some(id) => id,
        None => return,
    };
    let tab = match self.tabs.get(&tab_id) {
        Some(t) => t,
        None => return,
    };

    // Lock lives for entire render
    let term = tab.terminal.lock();

    let params = FrameParams {
        grid: term.active_grid(),
        palette: &term.palette,
        mode: term.mode,
        cursor_shape: term.cursor_shape,
        selection: tab.selection.as_ref(),
        search: tab.search.as_ref(),
        // ...
    };

    // draw_frame borrows params — term lock held throughout
    if let (Some(gpu), Some(renderer)) = (&self.gpu, &mut self.renderer) {
        // ...
        renderer.draw_frame(gpu, surface, config, &params, ...);
    }

    drop(term);  // Explicit drop after render complete

    // Clear dirty flag — needs write lock (re-acquire)
    if let Some(tab) = self.tabs.get(&tab_id) {
        tab.terminal.lock().grid_dirty = false;
    }
    self.tab_bar_dirty = false;
}
```

- [ ] Restructure `render_window()` to hold `MutexGuard` for the full render
- [ ] Ensure `FrameParams` lifetime is bounded by the guard
- [ ] Post-render: re-lock briefly to clear `grid_dirty`
- [ ] Verify no other borrows of `self.tabs` conflict with the lock guard
- [ ] Handle the case where `self.tabs` and `self.renderer` are borrowed simultaneously (may need to restructure renderer access)

**Borrow checker note**: `self.tabs.get(&tab_id)` borrows `self.tabs` immutably. `self.renderer` is a separate field. Both can be borrowed simultaneously. But `self.gpu` and `self.renderer` together require careful structuring — extract references before the lock scope.

---

## 04.2 Dirty Flag Coordination

**Current flow**:
1. PTY thread: `process_output()` sets `grid_dirty = true`
2. Main thread: `about_to_wait()` reads `grid_dirty`
3. Main thread: `render_window()` reads `grid_dirty` for FrameParams
4. Main thread: post-render sets `grid_dirty = false`

**After threading**:
1. PTY thread: `process_output()` sets `grid_dirty = true` (under write lock)
2. Main thread: `about_to_wait()` needs to check `grid_dirty` — requires lock
3. Main thread: `render_window()` reads `grid_dirty` under same lock as grid access
4. Main thread: post-render sets `grid_dirty = false` under lock

**Optimization**: Use an `AtomicBool` for `grid_dirty` so `about_to_wait()` can check without locking:

```rust
pub struct TerminalState {
    // ... fields ...
    // Atomic dirty flag — set by PTY thread, read/cleared by main thread
    // Avoids locking just to check "do I need to render?"
    grid_dirty: AtomicBool,
}
```

- [ ] Change `grid_dirty` from `bool` to `AtomicBool` in `TerminalState`
- [ ] PTY thread: `term.grid_dirty.store(true, Ordering::Release)` after VTE parsing
- [ ] Main thread `about_to_wait()`: `term.grid_dirty.load(Ordering::Acquire)` — no lock needed
- [ ] Main thread `render_window()`: `term.grid_dirty.store(false, Ordering::Release)` after render
- [ ] Selection/scroll ops that set `grid_dirty`: acquire lock, modify state, set atomic flag

---

## 04.3 Frame Budget Under Contention

When the PTY thread is doing heavy VTE parsing (flood), the main thread may wait on the lock. The current 8ms frame budget still applies — if we can't acquire the lock within the budget, skip this frame.

```rust
// In about_to_wait / render_window:
let term = match tab.terminal.try_lock() {
    Some(guard) => guard,
    None => {
        // PTY thread is parsing — skip this frame, render next time
        // Schedule wake-up at next frame budget
        return;
    }
};
```

**Alternative**: Use `lock()` (blocking) since the PTY thread releases quickly (64KB chunks). The FairMutex ensures the renderer gets its turn. Alacritty blocks here — it's fine because PTY holds the lock for microseconds per chunk.

**Decision**: Use blocking `lock()` for rendering. The FairMutex guarantees the renderer isn't starved. `try_lock` would cause dropped frames during floods, which looks worse than a 100μs wait.

- [ ] Rendering uses `tab.terminal.lock()` (blocking, fair)
- [ ] `about_to_wait()` dirty check uses atomic (no lock)
- [ ] Measure: log lock wait time during flood to verify it's sub-millisecond

---

## 04.N Completion Checklist

- [ ] `render_window()` acquires lock, builds FrameParams, renders, releases
- [ ] `grid_dirty` is `AtomicBool` — no lock needed for dirty check
- [ ] Lock hold time during render is bounded (frame rendering itself doesn't hold the lock — only FrameParams construction)
- [ ] No stale grid references after lock release
- [ ] Rendering works correctly with concurrent PTY updates
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` passes

**Exit Criteria:** Rendering reads grid state through the lock. Dirty flag is atomic. No rendering regressions.

**WAIT — IMPORTANT CORRECTION**: FrameParams holds `&Grid` which borrows from the MutexGuard. The lock IS held during `draw_frame()`. This is correct — Alacritty does the same thing. Lock hold time = render time (a few milliseconds). PTY thread yields via `try_lock_unfair` during this window and accumulates more bytes. The FairMutex lease ensures it gets the next lock after render completes.

---

**MANDATORY POST-COMPLETION:** After finishing this section, update this file: set YAML status to `complete`, check every completed checkbox, update `index.md`, and record any deviations from the plan. The plan must match what was actually implemented.
