# Threaded PTY Processing — Overview

## MANDATE

**NO DEFERRALS. NO SHORTCUTS. NO HALF IMPLEMENTATIONS.**

This is the full-blown threading refactor. Every section is mandatory. Every checkbox gets checked. Every call site gets converted. Every feature gets validated. There is no "Phase 2" — there is only done.

- We do NOT ship a "quick batching fix" as a stopgap.
- We do NOT leave any `send_pty` calls unconverted.
- We do NOT leave any direct `tab.grid()` accesses in `src/app/`.
- We do NOT skip the input channel ("we'll do paste later").
- We do NOT skip the FairMutex ("regular Mutex is fine for now").
- We do NOT skip end-to-end validation ("it compiles so it works").
- We do NOT leave dead code from the old architecture.

Every section. Every line. Every thread. Complete.

---

## Problem

All PTY processing (VTE parsing, grid mutation) runs on the main/UI thread. The reader thread only reads raw bytes and sends them as `TermEvent::PtyOutput` to the winit event loop. During output floods (e.g., `cat large_file`), the main thread is saturated with thousands of `PtyOutput` events — each one doing VTE parsing — before `about_to_wait` ever fires to render. Result: the UI freezes during high-throughput output.

## Goal

Move VTE parsing and grid mutation off the main thread entirely. Each tab's PTY reader thread owns parsing. The main thread only reads grid state for rendering. This is the Alacritty/Ghostty/Windows Terminal architecture.

## Architecture: Before vs After

### Before (Current)
```
Reader Thread (per tab)              Main Thread
─────────────────────               ───────────
read PTY bytes                      drain ALL user events:
send TermEvent::PtyOutput ──────►     process_output() [VTE parse + grid mutate]
                                      handle keyboard/mouse
                                      render (about_to_wait)
```

### After (Target)
```
Reader Thread (per tab)              Main Thread
─────────────────────               ───────────
read PTY bytes                      handle keyboard/mouse input
lock terminal (write)               lock terminal (read) for rendering
parse VTE → update grid             build FrameParams from locked state
unlock terminal                     unlock terminal
notify main: "wake up"              send input via channel (lock-free)
```

## Key Design Decisions

1. **`parking_lot::Mutex<TerminalState>`** — Not `RwLock`. Alacritty uses Mutex because lock hold times are short (microseconds for VTE parsing chunks, milliseconds for rendering). RwLock's overhead isn't justified when there's only one writer and one reader. `parking_lot` is faster than `std::sync::Mutex` and doesn't poison.

2. **FairMutex pattern** — PTY thread reserves next slot before reading (lease). Prevents renderer from starving the parser during floods. PTY thread yields if lock is held (`try_lock`), accumulates more data, then locks when buffer is full.

3. **Lock-free input channel** — Keyboard/mouse input to PTY goes through `mpsc::Sender`. No terminal lock needed for sending input. PTY thread drains the channel and writes to PTY stdin.

4. **Terminal state split** — Not the entire `Tab` behind the mutex. Only the state that both threads need: `Grid`, `Palette`, `TermMode`, `CursorShape`, title, bell, etc. Selection and search stay on the main thread (they're UI-only).

5. **No `TermEvent::PtyOutput`** — Reader thread no longer sends raw bytes to the event loop. It parses in-place. Only sends lightweight `TermEvent::Wakeup(TabId)` to trigger a redraw.

## Dependencies

- `parking_lot` crate (already used by `portable-pty` transitively, or add directly)
- No new crate for channels — `std::sync::mpsc` is sufficient

## Execution Order

All six sections are mandatory. No tiers — this is one atomic refactor.

1. **Section 01: Shared State** — Extract `TerminalState`, wrap in `Arc<FairMutex<>>`. Pure refactor, code still compiles and runs single-threaded.
2. **Section 02: PTY Thread** — Reader thread does VTE parsing. `PtyOutput` event eliminated. This is the core change.
3. **Section 03: Input Channel** — All PTY writes go through channel. `pty_writer` moves to reader thread. Every call site converted.
4. **Section 04: Rendering** — `render_window()` acquires lock. `grid_dirty` becomes atomic. Frame budget verified.
5. **Section 05: App Refactor** — Every single `tab.grid()`, `tab.mode`, `tab.palette` access in `src/app/` goes through the lock. Zero direct accesses remain.
6. **Section 06: Lifecycle** — Thread-safe resize, clean shutdown, full E2E validation, dead code removal.

**No section is optional. No section is deferred.**

## MANDATORY: Update Plan After Every Section

**After completing each section, you MUST update the plan files before moving on:**

1. **Set the section's YAML `status` to `complete`** in its frontmatter.
2. **Check every checkbox** (`- [x]`) that was accomplished.
3. **Update the index.md** status for that section.
4. **Record any deviations** — if the implementation differed from the plan (new files, changed approach, additional call sites discovered), update the section file to reflect what actually happened. The plan must be a living document that matches reality, not a stale spec.
5. **Add notes for the next section** if anything was discovered during implementation that affects upcoming work.

**The plan is the source of truth.** If a section says "not started" but the code is done, the plan is WRONG. If the code changed and the plan doesn't reflect it, the plan is WRONG. Keep them in sync. Always.

---

## Risk Assessment

- **Deadlock**: Low. Single mutex per tab, no nested locking across tabs. PTY thread and main thread never hold two locks simultaneously.
- **Stale renders**: Acceptable. If PTY thread is mid-update, renderer sees previous frame's state. At 120 FPS, one-frame latency is 8ms — imperceptible.
- **Borrow checker complexity**: Moderate. Every `self.tabs.get(&id)` becomes `self.tabs.get(&id).unwrap().terminal.lock()`. Lock guards must be scoped carefully.
