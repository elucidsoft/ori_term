# ori_term Rebuild — Overview

## Mandate

Rebuild ori_term from scratch. The old prototype proved the feature set (GPU window, PTY, VTE, tabs, fonts) but the architecture grew organically and became untenable: god objects, single-mutex contention, coupled VTE handler, rendering holding locks during GPU work, circular imports. The rebuild keeps all features but fixes the foundation with a multi-crate workspace, clean threading, and proper separation of concerns.

## Design Principles

1. **Bottom-up, one layer at a time** — Each layer solid and tested before the next begins.
2. **Crate boundary enforces separation** — `oriterm_core` knows nothing about GUI, fonts, PTY, config, or platform.
3. **Lock discipline** — Snapshot terminal state under lock (microseconds), release, then GPU work without lock.
4. **No god objects** — No struct exceeds ~12 fields. Responsibilities are singular.
5. **Term<T: EventListener>** — Generic terminal state machine, testable with `VoidListener`.
6. **Do it properly** — No workarounds, no hacks, no atomics-as-contention-fix. If it feels wrong, stop and redesign.

## Workspace Structure

```
ori_term/                           Workspace root
├── Cargo.toml                      [workspace] members
├── oriterm_core/                   Pure terminal library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── cell.rs                 Cell, CellFlags, CellExtra
│       ├── index.rs                Point, Line, Column newtypes
│       ├── event.rs                Event enum, EventListener trait
│       ├── sync.rs                 FairMutex
│       ├── grid/                   Grid, Row, Cursor, ring, scroll, editing
│       ├── term/                   Term<T>, VTE Handler, TermMode, CharsetState
│       ├── color/                  Palette, color resolution
│       ├── selection/              Selection model, boundaries, text extraction
│       └── search/                 SearchState, find_matches
├── oriterm/                        Binary (GUI, GPU, PTY, platform)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── app/                    App, event loop, input dispatch
│       ├── window.rs               TermWindow (winit + wgpu surface)
│       ├── tab.rs                  Tab (Arc<FairMutex<Term<EventProxy>>>)
│       ├── pty/                    PTY event loop, shell spawning
│       ├── gpu/                    GpuState, renderer, atlas, pipelines
│       ├── font/                   FontCollection, shaping, discovery
│       ├── chrome/                 Tab bar, drag, context menu
│       ├── config/                 TOML config, file watcher
│       ├── key_encoding/           Kitty + legacy encoding
│       └── clipboard.rs
├── _old/                           Old prototype (reference only)
├── assets/
└── plans/
```

## Dependency Graph

```
oriterm (binary) ──depends──> oriterm_core (lib)
     │                              │
     ├── winit                      ├── vte
     ├── wgpu                       ├── bitflags
     ├── swash                      ├── parking_lot
     ├── rustybuzz                  ├── unicode-width
     ├── portable-pty               ├── base64
     ├── serde, toml, notify        ├── log
     ├── window-vibrancy            └── regex
     ├── clipboard-win / arboard
     └── oriterm_core
```

Strictly one-way. `oriterm_core` has zero knowledge of GUI, fonts, PTY, config, or platform APIs.

## Threading Model

| Thread | Per | Owns | Lock Holds |
|--------|-----|------|------------|
| Main (UI) | process | winit EventLoop, windows, GpuState, GpuRenderer, FontCollection | microseconds (snapshot) |
| PTY Reader | tab | PTY read handle, read buffer, VTE Processor | microseconds (parse chunk) |

| Primitive | Per | Purpose |
|-----------|-----|---------|
| `FairMutex<Term<EventProxy>>` | tab | Terminal state |
| `mpsc::channel<Msg>` | tab | Main → PTY thread commands |
| `EventLoopProxy<TermEvent>` | process | PTY thread → main thread wakeup |

**Critical pattern:** Lock → snapshot `RenderableContent` → unlock → GPU work (no lock held).

## Section Overview (28 Sections, 8 Tiers)

### Tier 0 — Core Library + Cross-Platform Architecture
| Section | Title | What |
|---------|-------|------|
| 01 | Cell + Grid | Cell, Row, Grid, Cursor, scrollback, editing, navigation |
| 02 | Term + VTE | Terminal state machine, VTE Handler, modes, palette, SGR |
| 03 | Cross-Platform | Platform abstractions for PTY, fonts, clipboard, GPU, window (day one) |

### Tier 1 — Process Layer
| Section | Title | What |
|---------|-------|------|
| 04 | PTY + Event Loop | PTY spawning, reader thread, event proxy, lock discipline |

### Tier 2 — Rendering Foundation
| Section | Title | What |
|---------|-------|------|
| 05 | Window + GPU | winit window, wgpu pipeline (Vulkan/DX12/Metal), staged render pipeline (Extract→Prepare→Render), atlas, offscreen targets |
| 06 | Font Pipeline | Multi-face loading, shaping, ligatures, fallback, built-in glyphs, emoji |
| 07 | 2D UI Framework | Drawing primitives, layout engine, widgets, overlay system (oriterm_ui crate) |

### Tier 3 — Interaction
| Section | Title | What |
|---------|-------|------|
| 08 | Keyboard Input | Legacy + Kitty encoding, keyboard dispatch, IME |
| 09 | Selection & Clipboard | 3-point selection, word/line/block modes, clipboard, paste filtering |
| 10 | Mouse Input & Reporting | Mouse reporting modes, selection state machine, auto-scroll |
| 11 | Search | Plain text + regex search, search UI overlay, match highlighting |
| 12 | Resize & Reflow | Window resize, grid reflow, PTY resize notification |
| 13 | Config & Keybindings | TOML config, hot reload, file watcher, keybinding system |
| 14 | URL Detection | Implicit URL detection, hover underline, Ctrl+click open |

### Tier 4 — Multi-Tab + Chrome (Feature Parity)
| Section | Title | What |
|---------|-------|------|
| 15 | Tab Struct & Management | Tab lifecycle, spawn, shutdown, CWD inheritance, mode cache |
| 16 | Tab Bar & Chrome | Layout, rendering, hit testing, bell pulse, tab hover preview |
| 17 | Drag & Drop | Chrome-style drag, tear-off, OS drag, merge detection |
| 18 | Multi-Window & Lifecycle | Window creation, DPI, Aero Snap, ConPTY-safe cleanup |
| 19 | Event Routing & Scheduling | Coordinate systems, dispatch, frame budget, cursor blink |
| 20 | Shell Integration | Shell detection, injection, OSC 7/133, prompt state, two-parser |
| 21 | Context Menu & Controls | GPU-rendered menus, config reload, settings UI, window controls |

### Tier 5 — Hardening
| Section | Title | What |
|---------|-------|------|
| 22 | Terminal Modes | Comprehensive DECSET/DECRST table, mode interactions, image protocol |
| 23 | Performance & Damage Tracking | Damage tracking, ring buffer, parsing optimization, benchmarks |

### Tier 6 — Polish
| Section | Title | What |
|---------|-------|------|
| 24 | Visual Polish | Cursor blink, hide-while-typing, smooth scroll, background images |
| 25 | Theme System | 100+ themes, TOML theme files, discovery, light/dark auto-switch |

### Tier 7 — Advanced
| Section | Title | What |
|---------|-------|------|
| 26 | Split Panes | Binary tree layout, pane navigation, drag resize, zoom |
| 27 | Command Palette & Quick Terminal | Fuzzy search palette, global hotkey dropdown, notifications |
| 28 | Extensibility | Lua scripting, custom shaders, smart paste, undo close tab |

## Milestones

| Milestone | Section | What You See |
|-----------|---------|-------------|
| **M1: Lib compiles** | 01-02 complete | `cargo test -p oriterm_core` passes, Grid + VTE verified |
| **M2: Cross-platform foundations** | 03 complete | Platform abstractions defined for PTY, fonts, clipboard, GPU |
| **M3: Shell runs** | 04 complete | PTY spawns shell, I/O relayed (logged, no window) |
| **M4: Terminal renders** | 05 complete | Window opens, staged render pipeline, terminal grid visible, shell works |
| **M5: Full font pipeline** | 06 complete | Ligatures, emoji, fallback chains, box drawing, text decorations |
| **M6: UI framework** | 07 complete | Drawing primitives, layout engine, widgets, overlay system |
| **M7: Interactive** | 08-14 complete | Keyboard, mouse, selection, clipboard, search, config, resize, URLs |
| **M8: Feature parity** | 15-21 complete | Multi-tab, tab bar, drag, multi-window, shell integration, menus |
| **M9: Hardened** | 22-23 complete | All terminal modes, performance optimized, damage tracking |
| **M10: Polished** | 24-25 complete | Cursor blink, smooth scroll, 100+ themes, light/dark auto |
| **M11: Advanced** | 26-28 complete | Split panes, command palette, Lua scripting |

## Key References

| What | Where |
|------|-------|
| Alacritty Term<T> pattern | `~/projects/reference_repos/console_repos/alacritty/alacritty_terminal/src/term/mod.rs` |
| Alacritty EventListener | `~/projects/reference_repos/console_repos/alacritty/alacritty_terminal/src/event.rs` |
| Alacritty FairMutex | `~/projects/reference_repos/console_repos/alacritty/alacritty_terminal/src/sync.rs` |
| Alacritty PTY event loop | `~/projects/reference_repos/console_repos/alacritty/alacritty_terminal/src/event_loop.rs` |
| Old Cell/CellFlags | `_old/src/cell.rs` |
| Old GPU renderer | `_old/src/gpu/renderer.rs` |
| Old Grid | `_old/src/grid/mod.rs` |
| Old VTE handler | `_old/src/term_handler/mod.rs` |

## Anti-Patterns (explicitly forbidden)

1. **No god objects** — Max ~12 fields per struct. Split responsibilities.
2. **No lock during GPU work** — Snapshot under lock, render without lock.
3. **No separate VTE handler struct** — `Term<T>` implements `Handler` directly.
4. **No atomic workarounds** — Lock is held microseconds, no shadow state needed.
5. **No circular imports** — Crate boundary prevents it. Within binary, renderer receives data not domain objects.
6. **No rendering constants in grid** — Grid knows nothing about pixels.
7. **No `unwrap()` in library code** — Return `Result` or provide defaults.
