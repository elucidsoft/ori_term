<img src="assets/icon.svg" width="128" height="128" alt="ori-term">

# ori-term

A GPU-accelerated terminal emulator written from scratch in Rust. Cross-platform (Windows, Linux, macOS) from day one.

Built by studying 18 terminal projects from the inside out — Alacritty, Ghostty, WezTerm, Chrome, VS Code, Windows Terminal, and others.

## Status

**Rebuilding.** The original prototype proved the feature set but the architecture grew organically and became untenable. This is a ground-up rebuild with a clean multi-crate workspace, proper staged render pipeline, and cross-platform architecture baked in from the start.

The rebuild is tracked by a [28-section roadmap](plans/roadmap/index.md) spanning 8 tiers, from core grid/VTE through GPU rendering, UI framework, tabs, and extensibility.

The prototype code lives in `_old/` for reference.

## Architecture

```
ori_term/                           Workspace root
├── oriterm_core/                   Pure terminal library (no GUI, no PTY, no platform)
│   └── src/
│       ├── cell.rs                 Cell, CellFlags, CellExtra
│       ├── grid/                   Grid, Row, Cursor, scrollback, editing, reflow
│       ├── term/                   Term<T>, VTE Handler, TermMode
│       ├── color/                  Palette, color resolution
│       ├── selection/              Selection model, text extraction
│       └── search/                 SearchState, find_matches
├── oriterm_ui/                     2D UI framework (widgets, layout, drawing primitives)
├── oriterm/                        Binary (window, GPU, PTY, platform, chrome)
│   └── src/
│       ├── app/                    App, event loop, input dispatch
│       ├── gpu/                    Staged render pipeline (Extract → Prepare → Render)
│       ├── font/                   FontCollection, shaping, fallback, atlas
│       ├── chrome/                 Tab bar, drag, context menu
│       └── ...
└── _old/                           Prototype (reference only)
```

Strictly one-way dependencies. `oriterm_core` has zero knowledge of GUI, fonts, PTY, config, or platform APIs.

## Planned Features

### Window & Rendering
- **GPU-accelerated** — wgpu with Vulkan + DX12 on Windows, Vulkan on Linux, Metal on macOS
- **Staged render pipeline** — Extract (lock+snapshot+unlock) → Prepare (pure CPU, testable) → Render (GPU)
- **Custom window chrome** — frameless window, tab bar as title bar, pixel-drawn window controls
- **Window transparency** — compositor-backed glass (Mica/Acrylic on Windows, vibrancy on macOS)
- **Offscreen render targets** — for tab previews, headless testing, thumbnails

### Terminal Core
- **Full VTE escape sequence handling** — SGR, cursor, erase, scroll regions, alternate screen, OSC, DCS
- **Text reflow on resize** — cell-by-cell reflow handling wide characters and wrapped lines
- **Scrollback** — ring buffer with configurable history
- **Mouse reporting** — X10, normal, button-event, any-event with SGR encoding
- **Synchronized output** — Mode 2026 for flicker-free rendering
- **Hyperlinks** — OSC 8 + implicit URL detection
- **Kitty keyboard protocol** — progressive enhancement with modifier reporting

### Font
- **Font shaping** — rustybuzz (HarfBuzz) for ligatures and complex scripts
- **Multi-face fallback** — Regular/Bold/Italic/BoldItalic + fallback chain
- **Built-in glyphs** — box drawing, block elements, braille, powerline
- **Color emoji** — RGBA atlas pages, VS15/VS16 presentation selectors
- **Text decorations** — underline (single, double, dotted, dashed, curly), strikethrough

### 2D UI Framework
- **GPU-rendered widgets** — buttons, sliders, text inputs, dropdowns, panels
- **Layout engine** — flexbox-style with Row/Column containers
- **Overlay system** — modals, context menus, tooltips, terminal previews
- **Animation** — easing functions, transitions, property animation
- **Theming** — dark/light themes derived from terminal palette

### Tabs & Chrome
- **Chrome-style tabs** — tear off into new window, drag back in, reorder
- **Tab hover preview** — scaled-down live terminal thumbnail on hover
- **Tab width lock** — close buttons don't shift during rapid close clicks
- **Bell animation** — pulsing background on inactive tabs

### Selection & Clipboard
- **3-point selection** — anchor/pivot/end with sub-cell precision
- **Word/line/block modes** — double-click, triple-click, Alt+click
- **Bracketed paste** — escape sequence wrapping for aware applications
- **OSC 52 clipboard** — application read/write access

### Color
- **Truecolor** — 24-bit RGB, 256-color, and 16-color ANSI
- **100+ built-in themes** — Catppuccin, Dracula, Nord, Gruvbox, Solarized, Tokyo Night, ...
- **Light/dark auto-switch** — follows system appearance
- **Color profile detection** — NO_COLOR, CLICOLOR, COLORTERM

### Configuration
- **TOML config** — fonts, colors, keybindings, behavior, window
- **Hot reload** — visual settings update without restarting
- **Configurable keybindings** — user-definable shortcuts

### Advanced
- **Split panes** — horizontal/vertical splits with drag-to-resize
- **Command palette** — fuzzy-search action picker
- **Quick terminal** — global hotkey dropdown (Quake-style)
- **Lua scripting** — event hooks, custom commands, extensibility
- **Custom shaders** — post-processing effects via WGSL

### Cross-Platform
- **Windows** — ConPTY, DirectWrite, Vulkan + DX12, frameless with Aero Snap
- **Linux** — PTY, fontconfig, Vulkan, X11 + Wayland
- **macOS** — PTY, CoreText, Metal, native vibrancy

All three platforms are equal first-class targets. No platform is primary, no platform is an afterthought.

## Building

```bash
# Debug
cargo build --target x86_64-pc-windows-gnu

# Release
cargo build --target x86_64-pc-windows-gnu --release

# Checks
./clippy-all.sh
./test-all.sh
./build-all.sh
```

Cross-compiled from WSL targeting `x86_64-pc-windows-gnu`.

## Roadmap

The rebuild is organized into 28 sections across 8 tiers:

| Tier | Sections | Theme |
|------|----------|-------|
| 0 | 01-03 | Core library + cross-platform architecture |
| 1 | 04 | Process layer (PTY, threads) |
| 2 | 05-07 | Rendering foundation (window, GPU, fonts, UI framework) |
| 3 | 08-14 | Interaction (keyboard, mouse, selection, search, config) |
| 4 | 15-21 | Multi-tab + chrome (feature parity with prototype) |
| 5 | 22-23 | Hardening (terminal modes, performance) |
| 6 | 24-25 | Polish (visual refinements, themes) |
| 7 | 26-28 | Advanced (split panes, command palette, extensibility) |

See [plans/roadmap/index.md](plans/roadmap/index.md) for the full keyword-searchable index.

## Inspiration

| Project | What inspired us |
|---------|-----------------|
| Ghostty | Cell-by-cell text reflow approach |
| Alacritty | Term\<T\> architecture, FairMutex, VTE crate, strict clippy |
| WezTerm | Cross-platform PTY abstraction |
| Chrome | Tab drag state machine, GPU-rendered UI, tab previews |
| VS Code | Frameless window chrome pattern |
| Windows Terminal | Selection behavior and clipboard UX |
| Bevy | Staged render pipeline (Extract → Prepare → Render) |
| Catppuccin | Default color palette (Mocha) |
| Ratatui | Clippy lint configuration, testing patterns |
| termenv / lipgloss | Color profile detection cascade |

## The Name

**ori** — from the Japanese 折り (folding). Tabs fold between windows the way you fold paper.

## License

MIT
