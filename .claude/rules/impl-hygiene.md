---
paths:
  - "**/src/**"
---

# Implementation Hygiene Rules

**Implementation hygiene is NOT architecture** (design decisions are made) **and NOT code hygiene** (surface style). It's about whether the implementation faithfully and cleanly realizes the architecture — tight joints, correct flow, no leaks.

## Module Boundary Discipline

- **One-way data flow**: Data flows downstream only. Rendering never calls back into VTE parsing. Input handling doesn't reach into GPU internals.
- **No circular imports**: Module dependencies must be acyclic. `gpu/` never imports `tab_bar.rs` internals. `grid/` never imports `app.rs`.
- **Minimal boundary types**: Only pass what the next layer needs. Render params: `(&Grid, &Palette, &FontSet)`, not the entire `Tab`.
- **Clean ownership transfer**: Move at boundaries, borrow within modules. No unnecessary `.clone()` at layer transitions.
- **No layer bleeding**: Grid doesn't render, renderer doesn't parse VTE, input handler doesn't mutate grid directly.

## Data Flow

- **Zero-copy where possible**: Grid cells referenced by position, not by owned copies. Borrow `&Row`, don't clone rows for rendering.
- **No allocation in hot paths**: Render loop, VTE handler input path, and key encoding are hot. No `String::from()`, no `Vec::new()`, no `Box::new()` per cell/frame.
- **Newtypes for IDs**: `TabId(u64)`, not bare `u64`. Prevents cross-boundary ID confusion.
- **Instance buffers reused**: GPU instance buffers should grow but never shrink per frame. Reuse allocations across frames.

## Error Handling at Boundaries

- **No panics on user input**: Malformed escape sequences, invalid UTF-8 from PTY, unexpected key events — all must be handled gracefully, never `panic!` or `unwrap()`.
- **PTY errors are recoverable**: Reader thread errors close the tab, don't crash the app.
- **GPU errors surfaced**: Surface lost, device lost — recover or report, don't silently fail.
- **Config errors fall back to defaults**: Invalid TOML, missing fields, bad values — log a warning, use defaults.

## Rendering Discipline

- **Frame building is pure computation**: `draw_frame()` reads state, builds instance buffers. No side effects on Grid, Tab, or App state.
- **No state mutation during render**: Rendering borrows immutably. If render needs to change state (e.g., cursor blink toggle), send a message/event instead.
- **Opacity and color are resolved once**: Per-cell color resolution (bold-bright, dim, inverse) happens once per frame, not per-pipeline-pass.
- **Atlas misses are deferred**: If a glyph isn't cached, rasterize and cache it, but don't block the frame. Pre-cache ASCII at load time.

## Event Flow Discipline

- **Events flow through the event loop**: PTY output → `TermEvent` → `user_event` handler. No direct function calls bypassing the event loop.
- **Input dispatch is a decision tree, not a cascade**: Each input event is handled by exactly one handler. No fallthrough to multiple handlers.
- **State transitions are explicit**: Drag state machine (`Pending → DraggingInBar → TornOff`) uses enum variants, not boolean flags.
- **Redraw requests are coalesced**: Multiple state changes in one event batch should produce one redraw, not N redraws.

## Platform Abstraction

- **`#[cfg()]` at module level, not inline**: Platform differences go in dedicated files (`clipboard.rs` with `#[cfg(windows)]`/`#[cfg(not(windows))]`), not scattered `#[cfg()]` blocks inside functions.
- **Shared interface, platform implementation**: Common trait or function signature, platform-specific body.
- **No `cfg` in business logic**: Grid, VTE handler, selection, search — these must be platform-independent.
