# Prior Art Reference

Last updated: 2026-02-12
Repos: alacritty, wezterm, ghostty, crossterm, ratatui, termenv, lipgloss, bubbletea
Base path: `~/projects/reference_repos/console_repos/`

---

## 1. Grid & Scrollback Buffer

### Alacritty — Ring Buffer with Contiguous Storage
- `Storage<T>`: ring buffer backed by `Vec<Row<T>>` with wrap-around indexing
- Fixed-size visible region; scrollback grows up to configurable max, then wraps
- `Row<T>`: `inner: Vec<Cell>` with `occ: usize` tracking occupied length (avoids scanning empty tails)
- Grid generic over `<T: GridCell>`: same grid type for primary and alt screen
- Resize via dedicated `resize.rs` module: reflow wraps/unwraps lines, preserves cursor semantics
- Display offset: `display_offset` shifts viewport into scrollback; 0 = bottom (live terminal)
- Absolute indexing: `Line` (signed) for visible, raw index for scrollback; `grid[Line(i)]` maps through display_offset
- Cursor stored as `Point { line: Line, column: Column }` in grid coordinates
- Saved cursor for alt screen swap (DECSC/DECRC)
- **Key files:** `alacritty_terminal/src/grid/mod.rs`, `grid/storage.rs`, `grid/row.rs`, `grid/resize.rs`

### Ghostty — Page-Based Linked List
- `PageList`: doubly-linked list of `Page` nodes; each page is a contiguous memory block
- Page: fixed-size arena containing rows + cells + grapheme data; pages allocated/freed as units
- Memory-mapped pages: `mmap`/`munmap` for large scrollback without RSS pressure
- `Screen` wraps `PageList` with viewport tracking; `Terminal` owns primary + alt `Screen`
- Row metadata: `Row.Header` with flags (dirty, wrap, semantic prompt, kitty virtual placeholder)
- Grapheme storage: variable-width graphemes stored in page-local arena; cell stores offset + length
- Pin system: `Pin` = stable reference to a cell across page compaction/reallocation
- Active/inactive tracking: pages track active area within their rows; enables partial page use
- Circular reuse: when scrollback full, oldest page recycled (zero allocation in steady state)
- Style interning: `style_id` per cell → page-local style lookup table (saves 20+ bytes per cell)
- **Key files:** `src/terminal/PageList.zig` (481KB), `src/terminal/Screen.zig` (352KB), `src/terminal/page.zig` (140KB)

### WezTerm — Cluster-Based Lines
- `Line`: stores cells as `CellCluster` runs (consecutive cells with same attributes clustered)
- `CellCluster`: `{ first_cell_idx, attrs, text: String }` — text stored as UTF-8 string per cluster
- Compression: idle lines compressed to save memory; decompressed on access
- `Screen`: `lines: Vec<Line>` for visible + `scrollback: VecDeque<Line>` separate allocation
- StableRowIndex: monotonic counter for absolute row identity across scrollback operations
- Logical vs physical lines: `is_double_wide_line`, `wrap` flag for line continuation
- Cell storage: `CellAttributes` (64-bit packed) + `SmallVec<[u8; 4]>` for grapheme bytes
- Hyperlink interning: `Arc<Hyperlink>` shared across cells; hyperlink ID for matching
- Image protocol: `ImageCell` overlay system for Sixel/iTerm2/Kitty images
- **Key files:** `term/src/screen.rs` (41KB), `wezterm-surface/src/line/line.rs` (42KB), `wezterm-cell/src/lib.rs` (41KB)

### Cross-Cutting Patterns
- **Dual screen**: all three maintain primary + alt screen with swap on DECSET 1049
- **Occupied tracking**: Alacritty `occ`, WezTerm cluster compression — avoid processing empty cells
- **Stable references**: Ghostty Pin, WezTerm StableRowIndex — identity survives scrollback mutation
- **Separate visible/scrollback**: visible region fixed-size; scrollback grows/wraps independently
- **Reflow on resize**: all handle column-count changes by wrapping/unwrapping lines

---

## 2. GPU Rendering

### Alacritty — OpenGL with Damage Tracking
- Dual renderer: GLES2 (compatibility) + GLSL3 (modern); runtime selection
- Glyph atlas: `Atlas` struct with shelf packing; multiple atlas pages when full
- `GlyphCache`: `HashMap<GlyphKey, Glyph>` with `GlyphKey = { character, flags, size }`
- Instance rendering: one draw call per batch of cells with same texture atlas page
- Damage tracking: `display/damage.rs` — track dirty regions, only re-render changed rectangles
- `RenderableContent`: iterator over grid producing `RenderableCell` with resolved colors/decorations
- Cursor rendered as separate pass (blinking state, hollow vs block vs beam)
- Built-in box drawing: `builtin_font.rs` generates box-drawing/powerline glyphs programmatically
- Visual bell: overlay flash rendered as screen-covering transparent quad
- Rect rendering: `rects.rs` handles underlines, strikethrough, selection highlight as colored rects
- **Key files:** `alacritty/src/renderer/mod.rs`, `renderer/text/atlas.rs`, `renderer/text/glyph_cache.rs`, `display/damage.rs`, `display/content.rs`

### Ghostty — Multi-Backend with SIMD
- Three backends: Metal (primary), OpenGL (fallback), WebGL (browser via Wasm)
- `generic.zig`: shared rendering logic parameterized over backend; backend provides primitives
- Cell rendering: SIMD-accelerated cell attribute processing (8 cells at a time on ARM NEON)
- Separate passes: background (cells) → text (glyphs) → decorations (underlines) → cursor → images
- Font atlas: `font/Atlas.zig` — shelf packing with multiple pages; greyscale + color (emoji) separate
- Render thread: `renderer/Thread.zig` — dedicated thread, communicates via ring buffer with terminal thread
- Damage tracking: per-row dirty flags; skip unchanged rows entirely
- GPU-side color resolution: cell stores palette index; shader resolves to RGB (theme changes = zero CPU work)
- Image rendering: Kitty/Sixel images stored as GPU textures; composited in separate pass
- Custom shaders: user-loadable post-processing shaders for effects (CRT, bloom, etc.)
- **Key files:** `src/renderer/generic.zig` (141KB), `src/renderer/Metal.zig`, `src/renderer/cell.zig`, `src/renderer/row.zig`

### WezTerm — WebGPU/OpenGL Hybrid
- Primary: OpenGL; optional WebGPU via `wgpu` (same codebase as ori_term)
- `shader.wgsl`: WGSL shader for wgpu backend; GLSL shaders for OpenGL
- `GlyphCache`: two-tier — in-memory `HashMap` + GPU atlas; eviction by LRU
- `ShapeCache`: caches shaped text runs (not just individual glyphs) for complex scripts
- Screen line rendering: `render/screen_line.rs` processes one line at a time, batching quads
- Custom glyphs: `customglyph.rs` (280KB!) — extensive programmatic generation of box drawing, braille, powerline, nerd font symbols
- Quad-based rendering: each cell = positioned textured quad; batched into single draw call
- Render layers: background → cells → cursor → selection → scrollbar → tab bar
- Pane system: multiple panes per tab, each rendered independently with split borders
- **Key files:** `wezterm-gui/src/renderstate.rs`, `wezterm-gui/src/glyphcache.rs`, `wezterm-gui/src/shader.wgsl`, `wezterm-gui/src/termwindow/render/`

### Cross-Cutting Patterns
- **Atlas-based glyph storage**: all three use shelf-packed texture atlases; multiple pages when full
- **Batched draw calls**: minimize GPU state changes by grouping cells with same atlas page
- **Separate render passes**: background → text → decorations → cursor → overlays
- **Damage tracking**: dirty-region tracking to skip unchanged content (Alacritty rects, Ghostty rows)
- **Built-in box drawing**: all generate box-drawing glyphs programmatically rather than from fonts
- **Render thread separation**: Ghostty dedicates a thread; Alacritty/WezTerm render on event loop

---

## 3. Input & Key Encoding

### Alacritty — Binding Table + Winit Integration
- `input/keyboard.rs`: maps `winit::event::KeyEvent` to terminal bytes
- Binding system: `KeyBinding { trigger, mods, action, mode }` with configurable overrides
- Mode-sensitive: different bindings when alt screen active, vi mode, search mode
- Legacy encoding: builds byte sequences from virtual keycode + modifier state
- Kitty keyboard protocol: progressive enhancement levels (disambiguate, report events, report alternates, report all keys as escapes)
- Mouse encoding: X10, SGR, UTF-8 mouse modes with configurable button mapping
- Clipboard integration: OSC 52 for programmatic clipboard access
- **Key files:** `alacritty/src/input/mod.rs`, `alacritty/src/input/keyboard.rs`

### Ghostty — Comprehensive Kitty Implementation
- `key_encode.zig`: full Kitty keyboard protocol implementation (77KB — the most thorough)
- Key mapping: platform-native keycode → `Key` enum → encoded bytes; multi-layer translation
- Binding system: `Binding.zig` (164KB) — trigger + mods + consumed mods + action; supports sequences (chords)
- Modifier encoding: `key_mods.zig` — bitfield with Super, Hyper, Meta, Caps distinct from Shift/Ctrl/Alt
- Compose/dead key handling: accumulates compose sequences before encoding
- performAction pattern: binding resolution → action dispatch → terminal write
- Mouse: SGR pixel mode, extended coordinates, button/motion/release tracking
- **Key files:** `src/input/Binding.zig` (164KB), `src/input/key_encode.zig` (77KB), `src/input/kitty.zig`

### WezTerm — Input Map + Lua Scripting
- `inputmap.rs`: input mapping with Lua-configurable bindings
- `keyevent.rs`: processes raw key events through binding resolution → leader key → terminal encoding
- Leader key sequences: multi-key chord support (like vim's `<leader>` key)
- Key assignment: `KeyAssignment` enum (100+ variants) for all possible actions
- Dead key handling: `DeadKeyStatus` state machine for compose sequences
- IME integration: full Input Method Editor support for CJK input
- Mouse: `mouseevent.rs` (38KB) handles selection, click-through, drag, scroll with zone-based dispatch
- **Key files:** `wezterm-gui/src/inputmap.rs` (30KB), `wezterm-gui/src/termwindow/keyevent.rs` (30KB)

### Cross-Cutting Patterns
- **Kitty protocol**: all three implement progressive enhancement; Ghostty is the gold standard
- **Mode-sensitive dispatch**: different behavior based on terminal mode (alt screen, mouse mode, app cursor)
- **Binding → Action → Encode pipeline**: separate concerns of keybind matching, action resolution, byte encoding
- **Compose/dead key state machines**: all handle multi-keystroke input sequences
- **Mouse encoding modes**: X10, Normal, Button, Any, SGR, SGR-Pixel — fallback chain based on terminal mode

---

## 4. PTY Management

### Alacritty — Platform-Abstracted TTY
- `tty/mod.rs`: `Pty` trait with `read`, `write`, `resize` + platform impls
- Windows ConPTY: `tty/windows/conpty.rs` — `Conpty` struct wrapping `HPCON` handle
- Child process: `tty/windows/child.rs` — `ChildExitWatcher` polls process handle
- Event loop: `event_loop.rs` — dedicated thread, `mio`-based polling, reads PTY into parser
- Resize: `Pty::on_resize()` propagates new dimensions to ConPTY/pty
- Non-blocking I/O: PTY fd set to non-blocking; mio epoll/kqueue for readiness
- Error handling: I/O errors on PTY read → terminal close (not crash)
- **Key files:** `alacritty_terminal/src/tty/mod.rs`, `tty/windows/conpty.rs`, `tty/windows/child.rs`, `event_loop.rs`

### WezTerm — Portable PTY Crate
- Separate `portable-pty` crate: reusable across projects
- `CommandBuilder`: fluent API for configuring shell process (env vars, cwd, args)
- `PtyPair { master, slave }`: master for terminal, slave for child process
- `MasterPty` trait: `read`, `write`, `resize`, `get_size`, `try_clone_reader`
- Reader/Writer split: `MasterPty::try_clone_reader()` returns independent `Read` impl for threading
- Child process: `Child` trait with `wait`, `kill`, `process_id`; async-compatible
- ConPTY: `win/conpty.rs` — handles pseudoconsole lifecycle, pipe plumbing
- Serial port: `serial.rs` — same `MasterPty` trait for serial connections (!)
- **Key files:** `pty/src/lib.rs`, `pty/src/win/conpty.rs`, `pty/src/cmdbuilder.rs` (28KB)

### Ghostty — Zig PTY with Eventfd
- `pty.zig`: POSIX PTY via `openpty()` / `forkpty()`; Windows ConPTY
- `Command.zig`: child process management with env inheritance
- File descriptor management: PTY master fd polled via epoll/kqueue
- Resize: `TIOCSWINSZ` ioctl on Unix, `ResizePseudoConsole` on Windows
- Signal handling: SIGCHLD for child exit detection
- **Key files:** `src/pty.zig`, `src/Command.zig`

### Cross-Cutting Patterns
- **Trait/interface abstraction**: all abstract PTY behind trait/interface for platform portability
- **Reader/Writer separation**: PTY read on dedicated thread; write from event loop
- **ConPTY lifecycle**: create pseudoconsole → create pipes → start process → poll → close
- **Resize propagation**: window resize → PTY resize → child process SIGWINCH
- **Child exit detection**: poll/wait for child; trigger cleanup on exit

---

## 5. VTE Handling (Escape Sequence Processing)

### Alacritty — Handler Trait on Term
- `term/mod.rs`: `Term` implements `vte::Perform` trait directly (112KB file)
- Handler methods: `print()`, `execute()`, `hook()`, `put()`, `unhook()`, `osc_dispatch()`, `csi_dispatch()`, `esc_dispatch()`
- CSI dispatch: massive match on `(params, intermediates, action)` tuples
- Mode handling: `TermMode` bitflags for DECSET/DECRST modes (50+ flags)
- Cursor save/restore: `SavedCursor { point, rendition, origin_mode, ... }`
- Scroll regions: `scroll_up_relative` / `scroll_down_relative` respect DECSTBM margins
- Tab stops: `TabStops` bitset for HT/CHT/CBT handling
- Title stack: `Vec<String>` for XTPUSHCOLORS/XTPOPCOLORS
- Synchronized output: Mode 2026 — buffer updates, flush on end
- **Key files:** `alacritty_terminal/src/term/mod.rs` (112KB)

### WezTerm — Performer + TerminalState Split
- `terminalstate/mod.rs`: `TerminalState` struct holds all state (105KB)
- `performer.rs`: `Performer` implements VTE perform methods, delegates to `TerminalState`
- Kitty protocol: `terminalstate/kitty.rs` (34KB) — full Kitty graphics + keyboard protocol
- Custom VTE parser: `vtparse/src/lib.rs` (39KB) — state machine with transition tables
- CSI dispatch: categorized by function (`sgr()`, `cup()`, `ed()`, etc.) rather than monolithic match
- Image protocol support: Sixel, iTerm2, Kitty graphics — stored as `ImageData` associated with cells
- Unicode version tracking: per-terminal Unicode version setting affects width calculation
- **Key files:** `term/src/terminalstate/mod.rs` (105KB), `term/src/terminalstate/performer.rs` (49KB), `vtparse/src/lib.rs`

### Ghostty — Stream + Terminal Split
- `stream.zig`: state machine parser, emits typed actions to handler
- `Terminal.zig`: handler impl (412KB) — the most comprehensive VTE implementation
- Typed actions: parser emits `Action` union type; terminal matches on concrete action variants
- SGR handling: `sgr.zig` (32KB) — dedicated module for Select Graphic Rendition
- DCS/OSC separation: `dcs.zig`, `osc.zig` — each sequence family in own module
- Mode tracking: packed bitfield for 100+ DECSET modes
- Synchronized output: reference implementation — buffers operations, atomic flush
- Conformance testing: extensive test suite against vttest and other conformance suites
- **Key files:** `src/terminal/Terminal.zig` (412KB), `src/terminal/stream.zig` (122KB), `src/terminal/Parser.zig`

### Cross-Cutting Patterns
- **State machine parser**: all use table-driven VTE state machine (Paul Williams model or variant)
- **Handler trait/callback**: parser separated from terminal state; parser calls handler methods
- **CSI as core complexity**: CSI dispatch is the largest single handler in every implementation
- **Mode bitflags**: DECSET/DECRST modes stored as bitfield; checked in hot paths
- **Synchronized output (Mode 2026)**: buffer terminal updates during sync; atomic render on end
- **Scroll region discipline**: all operations respect DECSTBM top/bottom margins

---

## 6. Selection

### Alacritty — Side-Anchored Selection
- `SelectionType`: Simple (character) | Semantic (word) | Lines (full line) | Block (rectangular)
- `SelectionRange { start: Point, end: Point, is_block: bool }`: normalized range
- Side tracking: `Side::Left | Side::Right` — which side of cell the click landed on
- Semantic selection: expand to word boundaries using `semantic_escape_chars` config
- Selection across scrollback: uses absolute coordinates that survive scroll
- Selection rendering: resolved to `SelectionRange` on each frame; highlighted during render
- Clipboard: primary selection (middle-click) + clipboard (ctrl+shift+c) on X11
- **Key files:** `alacritty_terminal/src/selection.rs`

### Ghostty — Pin-Based Selection
- `Selection.zig`: selection anchored by `Pin` (stable cell reference across scrollback mutation)
- Types: `normal` (character) | `semantic` (word) | `line` | `block` (rectangular)
- Pin survives page compaction: when scrollback is recycled, pins are updated or invalidated
- Rectangle tracking: `Rectangle { top_left, bottom_right }` for block selection
- Ordered iteration: `ordered()` normalizes start/end regardless of selection direction
- Mouse tracking integration: selection suppressed when mouse mode active
- Drag state: tracks whether selection is being extended via shift-click
- **Key files:** `src/terminal/Selection.zig` (52KB)

### WezTerm — Range + Drag State Machine
- Selection stored as `SelectionRange { start: StableRowIndex, start_col, end: StableRowIndex, end_col }`
- Rectangular selection: block mode with column clamping
- Selection via `StableRowIndex`: survives scrollback because indices are monotonic
- Multi-click: single-click (character) → double-click (word) → triple-click (line) with timeout
- Drag handling: `mouseevent.rs` tracks drag start → extend → finish
- Copy-on-select: configurable auto-copy to clipboard on selection finish
- **Key files:** `wezterm-gui/src/selection.rs`

### Cross-Cutting Patterns
- **Three+ selection types**: character, word, line (and often block/rectangular)
- **Stable anchoring**: selections survive scrollback via absolute/stable indices
- **Side-aware character selection**: track which side of cell was clicked for precise boundary
- **Multi-click escalation**: single → double (word) → triple (line) with timeout reset
- **Semantic word boundaries**: configurable "word" characters for double-click expansion

---

## 7. Font Rendering & Glyph Management

### Alacritty — crossfont + Atlas
- `crossfont` crate: platform font loading (Core Text on macOS, FreeType on Linux, DirectWrite on Windows)
- `GlyphCache`: `HashMap<GlyphKey, Glyph>` where `GlyphKey = { id, size, font_key }`
- Atlas: shelf packing into OpenGL texture; grows by adding new texture pages
- Built-in font: `builtin_font.rs` — programmatic box-drawing, block elements, powerline symbols
- Font metrics: cell width/height derived from font metrics; consistent grid alignment
- Bold/italic: separate font loaded per style; fallback to synthetic bold (stroke widening)
- Subpixel rendering: platform-dependent (LCD on Linux, none on macOS Retina)
- **Key files:** `alacritty/src/renderer/text/glyph_cache.rs`, `renderer/text/atlas.rs`, `renderer/text/builtin_font.rs`

### Ghostty — Collection + SharedGrid
- `font/Collection.zig`: ordered list of font sources (primary → fallback chain)
- `font/Atlas.zig`: shelf-packed texture atlas; greyscale + color (emoji) as separate atlases
- `SharedGrid.zig`: thread-safe glyph grid shared between shaper thread and render thread
- Shaper: HarfBuzz-based with multiple shaper backends (`font/shaper/`)
- Discovery: `font/discovery.zig` — platform font enumeration (fontconfig, Core Text, DirectWrite)
- Metrics: `font/Metrics.zig` — cell dimensions, baseline, underline position/thickness
- Presentation: separate color emoji atlas from text atlas; composited in shader
- Sprite font: programmatic generation for box drawing (like Alacritty's builtin_font)
- **Key files:** `src/font/Collection.zig` (50KB), `src/font/Atlas.zig` (31KB), `src/font/face.zig`, `src/font/shaper/`

### WezTerm — FreeType/HarfBuzz + Massive Custom Glyphs
- `wezterm-font/src/lib.rs`: font loading via FreeType; shaping via HarfBuzz
- `wezterm-font/src/shaper/harfbuzz.rs`: text shaping with cluster tracking (41KB)
- `GlyphCache`: two-tier — CPU-side `HashMap` + GPU atlas; eviction by frame age
- `ShapeCache`: caches shaped text runs (not individual glyphs) for ligature/complex script efficiency
- `customglyph.rs` (280KB!): programmatic generation of box drawing, braille, powerline, nerd font, progress bars, block elements
- Font matching: `ParsedFont` system with scoring for family/style matching
- Fallback: cascading font list with per-codepoint search through fallback chain
- **Key files:** `wezterm-font/src/lib.rs` (38KB), `wezterm-font/src/shaper/harfbuzz.rs` (41KB), `wezterm-gui/src/glyphcache.rs`, `wezterm-gui/src/customglyph.rs`

### Cross-Cutting Patterns
- **Atlas texture with shelf packing**: universal approach — pack glyphs into rows in GPU texture
- **Fallback font chain**: primary font → style fallback → emoji fallback → built-in glyphs
- **Programmatic box drawing**: all generate box-drawing/block elements rather than loading from font
- **Cell metrics from font**: cell width/height derived from primary font metrics; grid alignment guaranteed
- **Separate text vs emoji**: emoji (color) stored in separate atlas or with color flag
- **Shape caching**: WezTerm caches shaped runs; Ghostty uses shared grid — avoid reshaping unchanged text

---

## 8. Color System

### Alacritty — Theme-Aware Palette
- `term/color.rs`: `Colors` struct with 270-entry array (16 named + 256 indexed palette)
- `Rgb` type: 3-byte RGB; no alpha in terminal colors
- Color resolution: cell stores `Color` enum (Named | Indexed | Spec(Rgb)); resolved at render time
- Theme colors: `display/color.rs` maps semantic names (foreground, background, cursor) to RGB
- Dynamic colors: OSC 10/11/12 change fg/bg/cursor at runtime; stored as overrides
- Dim colors: 30% brightness reduction for `SGR 2` (faint) attribute
- Config-based: colors fully configurable via TOML/YAML; live reload on config change
- **Key files:** `alacritty_terminal/src/term/color.rs`, `alacritty/src/display/color.rs`, `alacritty/src/config/color.rs`

### Ghostty — Style Interning + GPU Resolution
- `terminal/color.zig`: `Color` union (palette index, RGB) — 4 bytes
- Style interning: `style.zig` — cell stores `style_id` (u16); style table maps to full attributes
- GPU-side palette resolution: cell's palette index sent to shader; shader looks up RGB from palette uniform
- Theme changes: update palette uniform → all cells re-resolve → zero CPU work per cell
- SGR handling: `sgr.zig` — comprehensive SGR parser, builds `Style` struct
- 256-color + truecolor: seamless handling; palette indices 0-15 are theme-customizable
- **Key files:** `src/terminal/color.zig` (22KB), `src/terminal/style.zig` (38KB), `src/terminal/sgr.zig` (32KB)

### Crossterm — Environment-Aware Color Detection
- `style/types/color.rs`: `Color` enum — Reset | Black | DarkGrey | ... | Rgb{r,g,b} | AnsiValue(u8)
- Color detection: checks `COLORTERM`, `TERM`, `NO_COLOR`, `CLICOLOR_FORCE` environment variables
- Downgrade chain: TrueColor → ANSI256 → ANSI16 → None; nearest-color matching for downgrade
- Platform output: Windows uses Console API for legacy; VT sequences for modern terminals
- `Stylize` trait: fluent API `.red().bold().on_blue()` for composable styling
- `ContentStyle`: fully resolved style (fg + bg + underline color + attributes) applied atomically
- **Key files:** `src/style/types/color.rs` (16KB), `src/style/types/colored.rs`, `src/style/stylize.rs`

### termenv — Color Profile Detection (Reference Implementation)
- `ColorProfile`: Ascii | ANSI | ANSI256 | TrueColor — detected from environment
- Detection priority: `NO_COLOR` → `CLICOLOR_FORCE` → `COLORTERM` → `TERM` → TTY check
- `Environ` interface: injectable environment for testing (not `os.Getenv` directly)
- `HasDarkBackground()`: queries terminal via OSC 11; caches result for theme-aware defaults
- Adaptive colors: `AdaptiveColor { Light, Dark }` — auto-selects based on background
- `CompleteColor { TrueColor, ANSI256, ANSI }` — best color for each profile level
- Profile-aware downgrade: `color.convert(profile)` finds nearest color at target depth
- **Key files:** `termenv/` (Go library — patterns apply universally)

### Cross-Cutting Patterns
- **Palette as array**: 16 named + 240 extended = 256 indexed; plus truecolor
- **Deferred resolution**: cell stores color enum (not RGB); resolved at render or output time
- **Dynamic palette**: OSC 10/11/12 + OSC 4 for runtime palette modification
- **Environment detection**: NO_COLOR > CLICOLOR_FORCE > COLORTERM > TERM priority chain
- **Graceful downgrade**: TrueColor → ANSI256 → ANSI16 with nearest-color matching
- **GPU palette uniforms**: Ghostty's key insight — palette in GPU uniform avoids per-cell CPU resolution

---

## 9. Tab & Window Architecture

### WezTerm — Multiplexer Architecture
- `termwindow/mod.rs` (135KB): `TermWindow` manages tabs, panes, overlays, key state
- Tab model: `Tab` contains `Vec<Pane>` in split layout; pane = terminal instance
- Tab bar: `tabbar.rs` — renders tab strip with draggable tabs, close buttons, new-tab button
- Pane splits: horizontal/vertical splits within a tab; tree-based layout
- Mux (multiplexer): `mux/src/` — central process managing tab/pane lifecycle across windows
- Domain concept: `LocalDomain`, `RemoteDomain`, `SshDomain` — tabs can connect to different hosts
- Commands: `commands.rs` (84KB) — 100+ bindable actions (`SpawnTab`, `CloseCurrentTab`, `ActivateTabRelative`, etc.)
- Overlay system: search, copy mode, debug overlay rendered above terminal content
- **Key files:** `wezterm-gui/src/termwindow/mod.rs` (135KB), `wezterm-gui/src/tabbar.rs`, `wezterm-gui/src/commands.rs`

### Ghostty — Surface Abstraction
- `Surface.zig` (242KB): platform-agnostic terminal surface (one per tab equivalent)
- Surface owns: terminal state, font grid, renderer, PTY, input state
- Platform integration: macOS (AppKit), Linux (GTK), web (Wasm) — surface adapted per platform
- Tab management: delegated to platform's native tab system (macOS tab bar, GTK notebook)
- Split support: surfaces can be split within platform's native split view
- Action dispatch: `performAction()` routes user actions; surface handles terminal-local, app handles global
- Inspector: built-in terminal inspector surface for debugging (separate surface type)
- **Key files:** `src/Surface.zig` (242KB)

### Cross-Cutting Patterns
- **Tab = container for terminal state**: each tab owns its own terminal, PTY, and render state
- **Command/action dispatch**: centralized action enum routes keybindings to tab/window operations
- **Tab bar as custom UI**: WezTerm renders its own; Ghostty uses native platform tabs
- **Split architecture**: both WezTerm and Ghostty support pane splits within tabs
- **Overlay system**: search, copy mode, and debug views rendered as overlays above terminal content

---

## Quick Reference: ori_term's Primary Influences

| Domain | Primary Source | Secondary | ori_term Pattern |
|--------|---------------|-----------|-----------------|
| Grid buffer | Alacritty (ring buffer) | Ghostty (pages) | Absolute row indexing + scrollback Vec |
| GPU rendering | Ghostty (multi-backend) | WezTerm (wgpu) | wgpu + WGSL shaders + shelf atlas |
| Input encoding | Ghostty (Kitty impl) | Alacritty (binding table) | Kitty + legacy in key_encoding.rs |
| PTY management | Alacritty (event loop) | WezTerm (portable-pty) | ConPTY in pty.rs |
| VTE handling | Alacritty (handler trait) | WezTerm (performer split) | term_handler.rs (~50 methods) |
| Selection | Alacritty (side-anchored) | Ghostty (pin-based) | 3-point selection model |
| Font rendering | WezTerm (custom glyphs) | Ghostty (atlas) | fontdue + shelf-packed atlas |
| Color system | Ghostty (style intern) | Alacritty (palette array) | 270-entry palette.rs |
| Tab architecture | WezTerm (tab bar) | Ghostty (surface) | HashMap<TabId, Tab> + custom tab bar |
