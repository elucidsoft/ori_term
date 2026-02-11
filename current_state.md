# ori_term -- Current State

## What the App Does

ori_term (binary name: `oriterm`) is a terminal emulator written in Rust with
Chrome-style tab tear-off and a custom frameless window chrome. It opens a native
window using winit (with OS decorations disabled), renders a terminal character grid
with a custom title bar using wgpu (GPU-accelerated instanced rendering with glyph
texture atlas) and fontdue (CPU glyph rasterization), and runs shell processes
(configurable, default `cmd.exe`) through ConPTY via the `portable-pty` crate.
It is cross-compiled from WSL targeting `x86_64-pc-windows-gnu` and runs on Windows.

The window supports compositor-backed transparency with DX12 DirectComposition
swapchains (`DxgiFromVisual`), premultiplied alpha blending, and Windows Acrylic
blur via `window-vibrancy`. Background opacity and tab bar opacity are independently
configurable.

The core features under development are Chrome-style tab management (multiple tabs per
window, tab reordering by drag, tearing a tab off into its own floating window) and a
VS Code-style custom window chrome (frameless window with integrated tab bar as title
bar, pixel-drawn window controls, resize borders).

## Architecture Overview

### Module Map

```
src/main.rs                    Entry point: App::run() with #![windows_subsystem = "windows"]

src/
  lib.rs                       Module declarations + log() / log_path() helpers
  app.rs                       App struct (1467 lines), winit ApplicationHandler, event dispatch,
                               frameless window creation, resize handling, keyboard/mouse input,
                               scrollback scroll shortcuts, drag state machine, font zoom,
                               mouse selection, clipboard integration
  window.rs                    TermWindow (winit Window + wgpu Surface + tab list + is_maximized)
  tab.rs                       Tab (dual Grid primary+alt, PTY writer+master, VTE Processor,
                               Palette, TermMode, title, CharsetState, cursor shape,
                               selection, CWD, prompt state); TabId; TermEvent; resize
  tab_bar.rs                   Tab bar rendering and hit-testing (TabBarLayout, TabBarHit),
                               window control buttons (minimize/maximize/close), window border,
                               pixel drawing helpers (draw_x, draw_plus, draw_rect, blend, etc.)
  drag.rs                      DragState / DragPhase -- Chrome-style drag state machine
  cell.rs                      Rich Cell (24 bytes), CellFlags (u16 bitflags), CellExtra (Arc),
                               Hyperlink struct, ANY_UNDERLINE composite flag
  search.rs                    SearchState, find_matches (plain text + regex), cell_match_type
                               with binary search, extract_row_text / byte_span_to_cols helpers
  selection.rs                 Selection model (3-point: anchor/pivot/end), SelectionMode
                               (Char/Word/Line/Block), SelectionPoint with sub-cell Side,
                               word_boundaries, logical_line_start/end, extract_text,
                               contains() for hit-testing
  url_detect.rs                Implicit URL detection: DetectedUrl (multi-row segments),
                               UrlDetectCache (per-logical-line, lazy), regex URL matching
                               across soft-wrapped lines, trim_url_trailing (balanced parens)
  clipboard.rs                 Platform clipboard wrapper: clipboard-win on Windows, no-op stubs
                               on other platforms. get_text() / set_text().
  grid/
    mod.rs                     Grid (1135 lines) -- rows, cursor, scrollback, display_offset,
                               scroll regions, tab stops, all operations, resize with reflow
    row.rs                     Row with occupancy tracking, grow/truncate
    cursor.rs                  Cursor with template Cell, input_needs_wrap
  term_handler.rs              TermHandler -- vte::ansi::Handler impl (~50 methods), SGR,
                               cursor, erase, scroll, modes, alt screen, OSC titles,
                               OSC 52 clipboard (base64), cursor style, charset mapping,
                               device attributes (DA/DA2), mode reports (DECRPM),
                               title push/pop, hyperlinks
  term_mode.rs                 TermMode bitflags (16 terminal modes)
  palette.rs                   270-entry color palette, resolve fg/bg with attributes,
                               bold-bright, dim, inverse, hidden, set/reset color
  config.rs                    Config struct (TOML), FontConfig, TerminalConfig, ColorConfig,
                               WindowConfig (opacity, tab_bar_opacity, blur), BehaviorConfig,
                               platform-specific config_dir/config_path, load/save with serde,
                               sensible defaults, parse_cursor_style() helper
  key_encoding.rs              Key encoding: encode_key() dispatches to Kitty protocol
                               (CSI u) or legacy xterm encoding. Handles all named keys,
                               modifiers, APP_CURSOR/APP_KEYPAD modes, numpad, F1-F12.
                               Pure functions, fully unit-tested (40+ tests).
  render.rs                    FontSet (fontdue) with Regular/Bold/Italic/BoldItalic variants
                               and fallback font chain, render_text() for tab bar labels,
                               render_glyph(), blend_pixel(), FontStyle enum
  gpu/
    mod.rs                     GPU module re-exports (atlas, pipeline, renderer)
    atlas.rs                   GlyphAtlas: 1024x1024 R8Unorm texture, row-based shelf packing,
                               lazy glyph rasterization via FontSet, ASCII pre-cache,
                               HashMap<(char, FontStyle), AtlasEntry> with UV coords
    pipeline.rs                Two WGSL shader pipelines: background (premultiplied alpha
                               quads) and foreground (alpha-blended glyph textures). 80-byte
                               instance stride. Triangle strip topology, instance-driven.
    renderer.rs                GpuState (wgpu init with DX12 DComp for transparency),
                               GpuRenderer (per-window), draw_frame(): tab bar (configurable
                               opacity), grid cells (configurable opacity), cursor,
                               decorations, window border, settings dropdown overlay.
                               InstanceWriter with per-element opacity. ~1680 lines.

  (Library scaffolding modules -- present but not used by the terminal emulator yet)
  color/                       Color types, profile detection (NO_COLOR, CLICOLOR, COLORTERM)
  style/                       Style struct, profile-aware rendering
  text/                        Unicode width, wrapping, truncation, ANSI stripping (stubs)
  terminal/                    Terminal struct, size, raw mode (crossterm-based)
  output/                      Buffered writer, pipe detection (stubs)
  input/                       Key events, async reader (stubs)
  layout/                      Rect, constraints (stubs)
  widgets/                     Spinner, progress, prompt, select (stubs)
```

### Data Flow

1. **Startup** (`App::run`): Sets a panic hook, loads a `FontSet` at default 16px
   from system fonts (with Regular/Bold/Italic/BoldItalic variants and fallback fonts
   for symbols and CJK), creates a winit `EventLoop<TermEvent>`, and calls
   `event_loop.run_app(&mut app)`.

2. **Window + Tab creation** (`resumed` handler): On first resume, creates one
   frameless window (`with_decorations(false)`) and spawns one tab inside it. The
   surface is immediately filled with the background color to prevent a white flash.
   Each tab opens a ConPTY pair, spawns `cmd.exe` on the slave side, and starts a
   background reader thread.

3. **PTY output**: The reader thread sends `TermEvent::PtyOutput(tab_id, data)` through
   the `EventLoopProxy`. The `user_event` handler feeds the data through both a raw
   `vte::Parser` (for OSC 7, OSC 133, XTVERSION) and the high-level VTE `Processor`,
   which parses escape sequences and calls `TermHandler` methods (via
   `vte::ansi::Handler` trait) that mutate the active grid (primary or alternate),
   update the palette, toggle terminal modes, and set tab title. Then the window
   containing the active tab is asked to redraw.

4. **Rendering** (`render_window`): `GpuRenderer::draw_frame()` builds instance
   buffers for the full frame. Background pass renders premultiplied alpha quads
   (cell backgrounds, tab bar, decorations) with per-element opacity control.
   Foreground pass renders alpha-blended glyph textures from the atlas. Overlay
   pass renders the settings dropdown (if open). Tab bar rendered at configurable
   `tab_bar_opacity` (palette-derived colors). Grid cells rendered at configurable
   `opacity` with per-cell color resolution via `Palette::resolve_fg/resolve_bg`
   (bold-bright, dim, inverse, hidden), underline decorations (5 styles),
   strikethrough, cursor shapes, selection highlight, and combining marks. Window
   border (1px, opaque, skipped when maximized). Clear color premultiplied by
   opacity for compositor transparency. Frame presented via wgpu surface.

5. **Input**: Keyboard events are intercepted for built-in shortcuts (Ctrl+T new tab,
   Ctrl+W close tab, Ctrl+Tab / Ctrl+Shift+Tab cycle tabs, Escape cancel drag,
   Shift+PageUp/Down page scroll, Shift+Home/End scroll to top/bottom of scrollback,
   Ctrl+=/+ zoom in, Ctrl+- zoom out, Ctrl+0 reset zoom, Ctrl+C smart copy-or-SIGINT,
   Ctrl+V paste, Ctrl+Shift+C copy, Ctrl+Shift+V paste, Ctrl+Insert copy,
   Shift+Insert paste), then forwarded to the active tab's PTY writer as raw bytes or
   escape sequences. Supported keys: printable chars (UTF-8), Enter, Backspace, Tab,
   Escape, arrows (with APP_CURSOR mode), Home, End, Delete, Insert, PageUp/Down,
   F1-F12.

6. **Mouse / Selection**: Left-click in the grid area starts text selection. Single-click
   for character selection, double-click for word selection, triple-click for line
   selection. Alt+click starts block (rectangular) selection. Shift+click extends an
   existing selection. Dragging updates the selection end point with auto-scroll when
   the cursor is above or below the grid. Selection is auto-copied to clipboard on
   mouse release. Right-click copies if selection exists, pastes if not. Mouse presses
   on the tab bar trigger hit-testing (`TabBarLayout`).

7. **Mouse / Drag**: Clicking a tab selects it; clicking the close button closes it;
   clicking the "+" button creates a new tab. Window control buttons handle minimize,
   maximize, and close. Empty space in the tab bar is a drag area (calls `drag_window()`
   for native OS drag, double-click toggles maximize). A press-and-hold on a tab
   initiates a `DragState` which transitions through `Pending -> DraggingInBar ->
   TornOff`.

8. **Resize borders**: Mouse near window edges (8px) triggers `drag_resize_window()`
   with the appropriate `ResizeDirection`. Cursor icon changes to resize arrows on hover.

## Custom Window Chrome

The app uses frameless windows with a custom-rendered title bar (VS Code / Windows
Terminal style):

- **Tab bar height**: 46px, serves as the window title bar
- **Tab styling**: Catppuccin Mocha colors, rounded top corners (2px cutoff), active
  tab is black (matches terminal BG for seamless blending), inactive tabs are surface0
- **Tab close button**: Pixel-drawn 8x8 X icon in a 24x24 hover square with alpha-blended
  background (~15% white tint) so it's visible on both active and inactive tabs
- **New tab button**: 38px wide with a pixel-drawn 11x11 plus icon (1px thick, centered)
- **Window controls**: Rightmost 138px (3 x 46px buttons) with pixel-drawn Windows 10/11
  style icons -- minimize (horizontal line), maximize (rectangle / overlapping restore
  rects), close (X with red hover background)
- **Window border**: 1px border around entire window in overlay0 color, hidden when maximized
- **Drag area**: Empty space between tabs and controls calls `drag_window()` for native
  OS window dragging with Aero Snap support
- **Internal padding**: 16px left margin for tabs, 8px top margin, 6px grid left padding,
  10px gap between tab bar and grid, 4px bottom padding

## What's Working

- **Custom frameless window chrome**: No OS title bar. Tab bar acts as the title bar with
  integrated tabs, window controls (minimize/maximize/close), and drag area.

- **Window controls**: Pixel-drawn minimize, maximize/restore, and close buttons with
  hover states. Maximize toggles between maximized and restored. Close removes all tabs
  and closes the window.

- **Window dragging**: Dragging empty tab bar area moves the window via native OS drag.
  Double-click toggles maximize. Aero Snap works.

- **Window resizing**: 8px resize borders on all edges and corners with appropriate cursor
  icons and native OS resize via `drag_resize_window()`.

- **No white flash on startup**: GPU surface is pre-filled with background color
  immediately after window creation.

- **Multi-tab support**: Ctrl+T creates new tabs, Ctrl+W closes them, Ctrl+Tab /
  Ctrl+Shift+Tab cycles between them. Each tab has its own Grid, PTY, and VTE parser.

- **Tab bar rendering**: Catppuccin Mocha-themed tab bar with active/inactive/hover
  states, pixel-drawn close buttons (x) with alpha-blended hover highlight, and a
  pixel-drawn "+" new-tab button. Hit-testing correctly distinguishes tab body, close
  button, new-tab, window controls, and drag area regions.

- **Tab tear-off creates new window**: Dragging a tab vertically past the tear-off
  threshold (15 px) removes it from its source window and creates a new frameless
  window positioned at the cursor. The tab's PTY and Grid survive the move because
  tabs live in `App.tabs` (keyed by `TabId`), separate from windows.

- **PTY management with ConPTY handles kept alive**: `portable-pty` opens a ConPTY
  pair; `cmd.exe` is spawned on the slave. The `pty_master` and `_child` handles
  are kept alive in the `Tab` struct -- dropping either would kill the ConPTY or the
  child process. A background thread reads PTY output and sends it to the event loop
  via `EventLoopProxy`.

- **Rich cell model**: 24-byte Cell struct with fg/bg colors (Named/Indexed/Spec),
  CellFlags bitflags (bold, dim, italic, underline variants, blink, inverse, hidden,
  strikeout, wide char, wrapline), and Arc<CellExtra> for rare data (zerowidth chars,
  underline color, hyperlinks). Row wrapper with occupancy tracking. `ANY_UNDERLINE`
  composite flag for efficient underline detection.

- **VTE parsing (comprehensive)**: `term_handler.rs` implements `vte::ansi::Handler`
  trait (~50 methods). Full SGR attribute handling (all codes 0-29 + colors + underline
  color SGR 58), cursor movement (CUP/CUU/CUD/CUF/CUB/CR/LF/NEL/RI), erase operations
  (ED 0/1/2/3, EL 0/1/2, ECH, DCH, ICH), line operations (IL/DL), scroll regions
  (DECSTBM), alternate screen (DECSET 1049), 16 terminal modes, tab stops, DSR 5/6,
  device attributes (DA/DA2), mode reports (DECRPM for both ANSI and private modes),
  OSC title updates (with push/pop stack), OSC 52 clipboard read/write (base64),
  cursor style (DECSCUSR), charset configuration (G0-G3 with DEC Special Graphics
  mapping), hyperlink support (OSC 8), DECALN screen alignment test, keypad
  application mode, text area size queries (CSI 8/4 t), reset state, substitute (SUB).
  A secondary raw `vte::Parser` with `RawInterceptor` (in `tab.rs`) handles OSC 7
  (CWD), OSC 133 (prompt markers), OSC 9/99/777 (notifications), and XTVERSION
  (CSI > q with build number from BUILD_NUMBER file) -- sequences the high-level
  Processor drops.

- **Full color support**: 270-entry palette (16 ANSI Catppuccin Mocha + 216 color
  cube + 24 grayscale + semantic colors). Per-cell foreground/background color
  resolution with bold-as-bright, DIM dimming, INVERSE swap, HIDDEN. Truecolor
  (24-bit RGB) support. OSC 4 palette mutation, OSC 104 reset. OSC 10/11/12
  foreground/background/cursor color queries.

- **Scrollback buffer**: VecDeque-based scrollback with configurable max (10,000 lines).
  display_offset viewport scrolling. Mouse wheel scroll (3 lines/tick). Keyboard
  shortcuts: Shift+PageUp/Down (page scroll), Shift+Home/End (top/bottom). Viewport
  anchoring when scrolled up. Auto-scroll to live on keyboard input. ED 3 clears
  scrollback. Alternate screen has no scrollback.

- **Dynamic resize with text reflow**: Window resize recalculates grid dimensions,
  resizes both primary and alternate grids (Ghostty-style cell-by-cell reflow
  algorithm: smart scrollback integration, cursor preservation), notifies PTY via
  pty_master.resize(). Scroll region resets on resize. Alt screen does not reflow
  (full-screen apps redraw themselves).

- **Font system with style variants and fallback chain**: `FontSet` replaces the old
  `GlyphCache`. Loads Regular, Bold, Italic, and BoldItalic font variants from system
  fonts. Cross-platform font discovery: on Windows, tries CascadiaMonoNF > CascadiaMono
  > Consolas > Courier from `C:\Windows\Fonts\`; on Linux, searches
  `~/.local/share/fonts`, `/usr/share/fonts`, `/usr/local/share/fonts` for JetBrainsMono
  > UbuntuMono > DejaVuSansMono > LiberationMono. Fallback font chain for missing glyphs:
  on Windows, Segoe UI Symbol + MS Gothic (CJK) + Segoe UI; on Linux, NotoSansMono +
  NotoSansSymbols2 + NotoSansCJK + DejaVuSans. Synthetic bold (double-strike at +1px
  offset) is used when no real bold font is available. Glyphs are lazily rasterized and
  cached by (char, FontStyle) key, with ASCII pre-cached at load time. `FontSet::resize()`
  rebuilds at a new size preserving the same font files.

- **Font zoom**: Ctrl+= or Ctrl++ zooms in (+1px), Ctrl+- zooms out (-1px), Ctrl+0
  resets to default (16px). Font size is clamped to 8-32px range. Zoom triggers
  `FontSet::resize()` which recomputes cell metrics and clears the glyph cache, then
  resizes all tabs in the window to match new cell dimensions.

- **Underline decorations**: Renderer draws five underline styles at 2px from cell bottom:
  single (solid line), double (two lines 2px apart), dotted (every other pixel), dashed
  (3px on / 2px off), and undercurl (sine wave, 2px amplitude). Underline color respects
  SGR 58 (per-cell underline color override) or falls back to foreground color.

- **Strikethrough**: Horizontal line at vertical center of the cell, drawn in foreground
  color.

- **Mouse selection and clipboard (Windows Terminal style)**: Single-click for character
  selection, double-click for word selection, triple-click for line selection, Alt+click
  for block (rectangular) selection, Shift+click to extend. Selection is auto-copied to
  clipboard on mouse release. Right-click copies if selection exists, pastes if not.
  Ctrl+C is smart: copies if selection exists, sends ^C otherwise. Ctrl+V pastes.
  Ctrl+Shift+C / Ctrl+Insert copy. Ctrl+Shift+V / Shift+Insert paste. Bracketed paste
  mode supported. Selection uses 3-point model (anchor/pivot/end) with sub-cell Side
  precision.

- **OSC 52 clipboard (base64)**: Applications can read from and write to the system
  clipboard via OSC 52 escape sequences. `clipboard_store` decodes base64 data and
  writes to clipboard. `clipboard_load` reads clipboard, encodes to base64, and sends
  the response back to the PTY.

- **Cursor style rendering**: DECSCUSR (CSI Ps SP q) sets cursor shape (Block/Underline/
  Beam). The `cursor_shape` field is stored per-tab. GPU renderer draws all three
  shapes: Block (filled rectangle with text inversion), Bar (2px vertical at left),
  Underline (2px horizontal at bottom).

- **Keyboard input forwarding**: Comprehensive key encoding via `key_encoding.rs`.
  Legacy xterm-style encoding for all standard keys (Enter, Backspace, Tab, Escape,
  arrows with APP_CURSOR, Home/End, Delete, Insert, PageUp/Down, F1-F12, printable
  text). Kitty keyboard protocol (CSI u) with progressive enhancement flags, event
  type reporting (press/repeat/release), and mode stack management. Application
  keypad mode (numpad via SS3 sequences). All modifier combinations (Ctrl+letter
  C0 codes, Alt ESC prefix, Shift+Tab backtab). 40+ unit tests.

- **Tab reordering**: While dragging within the tab bar (before tear-off), tabs can
  be reordered by horizontal cursor position.

- **Multi-window**: Each window is independently managed with its own surface and tab
  list. Closing the last window exits the application.

- **Escape cancels drag**: Pressing Escape during any drag phase reverts the drag
  and redraws all windows.

- **Panic/error logging**: Panics are caught and written to `oriterm_panic.log`.
  Runtime trace goes to `oriterm_debug.log`. Top-level errors go to
  `oriterm_error.log`.

- **Mouse reporting to applications**: Full mouse reporting for vim, tmux, htop, etc.
  Normal tracking (1000), button-event tracking (1002), any-event tracking (1003).
  SGR encoding (1006), UTF-8 encoding (1005), and default encoding. Modifier bits
  (Shift+4, Alt+8, Ctrl+16). Wheel events as button 64/65. Alternate scroll mode
  converts wheel to arrow keys in alt screen. Shift+click bypasses reporting for
  selection. Motion dedup with `last_mouse_cell`.

- **Focus events**: DECSET 1004 sends `ESC[I` on focus gain and `ESC[O` on focus
  loss to the PTY. Settings window excluded from focus reporting.

- **Hyperlinks (OSC 8)**: OSC 8 hyperlink sequences parsed by vte, stored in
  `CellExtra.hyperlink`. Hyperlinked cells rendered with dotted underline (solid
  on hover). Ctrl+hover shows pointing hand cursor. Ctrl+click opens URL in
  default browser via platform command (cmd /C start, xdg-open, open). URL scheme
  validation: only http/https/ftp/file allowed.

- **Implicit URL detection**: Plain-text URLs (http/https/ftp/file) in terminal
  output are automatically detected and made clickable. Regex-based detection
  runs lazily on Ctrl+hover/click with per-logical-line caching. Handles URLs
  that wrap across soft-wrapped rows (multi-row segment tracking). Ctrl+hover
  shows pointer cursor and solid underline across the full URL span. Ctrl+click
  opens the URL. Handles Wikipedia-style parenthesized URLs and strips trailing
  punctuation. Skips cells that already have OSC 8 hyperlinks.

- **Bell and notifications**: BEL (0x07) sets a timestamp on the tab via
  `TermHandler::bell()`. OSC 9 (iTerm2), OSC 99 (Kitty), and OSC 777
  (rxvt-unicode) notification sequences are intercepted by `RawInterceptor`
  and logged. Inactive tabs that receive a bell show a subtle pulsing
  background animation in the tab bar (0.5 Hz sine lerp between inactive and
  hover colors). The badge clears when the tab becomes active. Configurable
  via `[bell]` section in config (animation, duration_ms, color).

- **Synchronized output**: Mode 2026 handled internally by vte 0.15 Processor.
  Buffers handler calls between BSU/ESU and dispatches as one batch.

- **GPU-accelerated rendering**: Full wgpu rendering pipeline. Two-pass architecture:
  background pipeline (premultiplied alpha quads) then foreground pipeline
  (alpha-blended glyph textures). Instance-driven rendering with 80-byte stride per
  quad. 1024x1024 R8Unorm glyph texture atlas with row-based shelf packing and lazy
  rasterization. `GpuState` (singleton) + `GpuRenderer` (per-window). Settings
  dropdown overlay with theme selector rendered as third pass.

- **Window transparency and blur**: DX12 DirectComposition swapchain
  (`Dx12SwapchainKind::DxgiFromVisual`) provides native `PreMultiplied` alpha mode
  for compositor transparency on Windows. `window-vibrancy` crate applies Acrylic
  blur behind the window. Per-element opacity: tab bar and grid content have
  independently configurable opacity (`tab_bar_opacity` and `opacity`). When opacity
  is < 1.0, background colors are premultiplied by the opacity value so the
  compositor sees transparent pixels and shows the blur effect through them. Fully
  opaque (1.0) skips the DComp path entirely for maximum performance.

- **Configuration (TOML)**: Config loaded from `%APPDATA%\ori_term\config.toml`
  (Windows) or `$XDG_CONFIG_HOME/ori_term/config.toml` (Linux). Sections: font
  (size, family), terminal (shell, scrollback, cursor_style), colors (scheme,
  foreground, background, cursor, selection_foreground, selection_background,
  ansi, bright), window (columns, rows, opacity, tab_bar_opacity, blur),
  behavior (copy_on_select, bold_is_bright), bell (animation, duration_ms,
  color). All fields optional with sensible
  defaults via `#[serde(default)]`. Color overrides apply on top of the active
  scheme via `Palette::apply_overrides()`. Load/save with error fallback to defaults.

- **Color scheme switching**: 7 built-in color schemes (Catppuccin Mocha/Latte,
  One Dark, Solarized Dark/Light, Gruvbox Dark, Tokyo Night). Runtime switching via
  settings dropdown menu. Selected scheme persisted to config file. Tab bar colors
  derived dynamically from palette background. Per-color overrides (foreground,
  background, cursor, selection fg/bg, ANSI 0-15) apply on top of any scheme and
  hot-reload with the config file. Selection uses configurable colors (default: fg/bg swap).

- **Tab title truncation (Unicode-aware)**: Uses `UnicodeWidthChar::width()` for
  proper display width calculation. Handles CJK (width 2) and uses Unicode ellipsis
  character (U+2026).

## What's Not Working Yet / Known Issues

- **Clipboard stubs on non-Windows**: The `clipboard.rs` module uses `clipboard-win`
  on Windows but provides no-op stubs on other platforms. Needs arboard or similar
  for cross-platform clipboard support.

- **No HiDPI / display scaling**: GPU renderer does not account for DPI scaling.

- **No damage tracking**: Full instance buffer rebuild every frame. Works correctly
  but is an optimization opportunity.

- **No cursor blinking**: Cursor shape changes work but blinking is not implemented.

- **Transparency only on Windows**: DX12 DirectComposition path is Windows-only.
  macOS vibrancy and Linux compositor blur are coded but untested.

## Key Data Structures

### `App` (app.rs)

Top-level application state. Owns all windows (`HashMap<WindowId, TermWindow>`),
all tabs (`HashMap<TabId, Tab>`), the shared `FontSet`, the current `DragState`,
per-window cursor positions and hover states, keyboard modifier state, double-click
tracking (`last_click_time`, `last_click_window`), click count tracking for
single/double/triple click detection, selection state (`left_mouse_down`,
`last_grid_click_pos`, `click_count`), and the `EventLoopProxy` for sending
`TermEvent`s from background threads. Implements `ApplicationHandler<TermEvent>`
for the winit event loop.

The separation of tabs from windows (each in their own HashMap) is the key design
that enables tab tear-off: moving a tab between windows is just updating two
`Vec<TabId>` lists without touching the PTY or Grid.

### `TermWindow` (window.rs)

Per-window state: an `Arc<Window>` (winit), a wgpu `Surface` and
`SurfaceConfiguration` for GPU rendering, a `Vec<TabId>` of tabs in display order,
the `active_tab` index, and `is_maximized` for tracking maximize state. Methods:
`add_tab`, `remove_tab`, `active_tab_id`, `tab_index`.

### `Tab` / `TabId` (tab.rs)

Each tab represents a running shell session. Contains:
- `Grid` -- primary and alternate character cell buffers
- `pty_writer` -- `Option<Box<dyn Write + Send>>` for writing to the PTY
- `pty_master` -- `Box<dyn MasterPty>`, must stay alive to keep ConPTY open
- `_child` -- `Box<dyn Child>`, must stay alive to keep the shell process running
- `processor` -- `vte::ansi::Processor` for high-level escape sequence parsing
- `raw_parser` -- `vte::Parser` for intercepting OSC 7/133/XTVERSION
- `title` -- display string (with `title_stack` for push/pop)
- `palette` -- 270-entry color palette
- `mode` -- `TermMode` bitflags
- `cursor_shape` -- `CursorShape` (Block/Underline/Beam)
- `charset` -- `CharsetState` for G0-G3 charset mapping
- `cwd` -- optional current working directory (from OSC 7)
- `prompt_state` -- `PromptState` (from OSC 133)
- `selection` -- optional `Selection` for mouse text selection
- `active_is_alt` -- whether alternate screen is active

`TabId` is a newtype `TabId(u64)`, allocated sequentially by `App::alloc_tab_id()`.

### `TermEvent` (tab.rs)

```rust
enum TermEvent {
    PtyOutput(TabId, Vec<u8>),
}
```

The only user event type. Sent from PTY reader threads to the winit event loop via
`EventLoopProxy`.

### `DragState` / `DragPhase` (drag.rs)

Tracks tab drag operations. Fields: `tab_id`, `source_window`, `origin` position,
`phase`, `original_index` (for revert), `grab_offset` (cursor position within the
torn-off window).

`DragPhase` has three variants:
- `Pending` -- mouse is down but hasn't moved past `DRAG_START_THRESHOLD` (10 px)
- `DraggingInBar` -- reordering within the tab strip
- `TornOff` -- tab is in its own window, following the cursor

Thresholds match Chrome's `tab_drag_controller.cc`: `kMinimumDragDistance` = 10 px,
`kVerticalDetachMagnetism` = 15 px.

### `TabBarLayout` / `TabBarHit` (tab_bar.rs)

`TabBarLayout` computes tab widths (clamped 80--200 px) based on count and available
width, reserving space for the left margin, new-tab button (38px), and window controls
zone (138px). `TabBarHit` is an enum: `Tab(usize)`, `CloseTab(usize)`, `NewTab`,
`Minimize`, `Maximize`, `CloseWindow`, `DragArea`, `None`. Used for both click
dispatch and hover state tracking.

Pixel drawing helpers: `set_pixel`, `fill_rect`, `draw_hline`, `draw_rect`, `draw_x`,
`draw_plus`, `blend` (alpha compositing for hover effects).

### `FontSet` (render.rs)

Replaces the old `GlyphCache`. Contains an array of 4 `fontdue::Font` objects (one per
`FontStyle`: Regular, Bold, Italic, BoldItalic), a `has_variant` array tracking which
styles have real font files (vs. falling back to Regular), a `Vec<fontdue::Font>` of
fallback fonts for missing glyphs (symbols, CJK), the current font `size`, computed
`cell_width`/`cell_height`/`baseline`, and a `HashMap<(char, FontStyle), (Metrics,
Vec<u8>)>` glyph cache.

Glyph rasterization follows a fallback chain: (1) requested style font, (2) Regular
font (style fallback), (3) fallback fonts (Segoe UI Symbol, MS Gothic, etc.), (4)
Unicode replacement character U+FFFD, (5) empty glyph.

`FontSet::load(size)` tries font families in priority order. `FontSet::resize(new_size)`
rebuilds at a new size preserving the same font files, clearing the cache.
`needs_synthetic_bold()` returns true when no real bold font was loaded.

### `Selection` (selection.rs)

3-point selection model: `anchor` (click origin), `pivot` (other end of initial unit),
`end` (current drag position). `SelectionMode` enum: `Char`, `Word`, `Line`, `Block`.
`SelectionPoint` has `row` (absolute), `col`, and `side` (Left/Right for sub-cell
precision). Implements `contains(row, col)` for rendering hit-testing. Helper functions:
`word_boundaries()`, `logical_line_start/end()`, `extract_text()`.

### `Cell` / `CellFlags` / `CellExtra` (cell.rs)

Rich 24-byte Cell struct:
- `c: char` -- the character (4 bytes)
- `fg: vte::ansi::Color` -- foreground (Named/Indexed/Spec)
- `bg: vte::ansi::Color` -- background
- `flags: CellFlags` -- u16 bitflags (BOLD, DIM, ITALIC, UNDERLINE, DOUBLE_UNDERLINE,
  UNDERCURL, DOTTED_UNDERLINE, DASHED_UNDERLINE, BLINK, INVERSE, HIDDEN, STRIKEOUT,
  WIDE_CHAR, WIDE_CHAR_SPACER, WRAPLINE, LEADING_WIDE_CHAR_SPACER)
- `extra: Option<Arc<CellExtra>>` -- rare data (zerowidth chars, underline_color, hyperlink)

Composite flag `ANY_UNDERLINE` combines all underline variants for efficient testing.

### `Grid` / `Row` / `Cursor` (grid/mod.rs, grid/row.rs, grid/cursor.rs)

`Grid` contains:
- `rows: Vec<Row>` -- visible rows
- `cols` / `lines` -- dimensions
- `cursor: Cursor` -- current position + template cell + `input_needs_wrap`
- `saved_cursor: Cursor` -- for DECSC/DECRC
- `scroll_top` / `scroll_bottom` -- scroll region bounds
- `tab_stops: Vec<bool>` -- tab stop positions
- `scrollback: VecDeque<Row>` -- history buffer (O(1) push/pop at both ends)
- `max_scrollback: usize` -- cap (default 10,000)
- `display_offset: usize` -- viewport offset into scrollback

`Row` wraps `Vec<Cell>` with `occ` (occupancy) for efficient reset.
`Cursor` has `col`, `row`, `template: Cell` (current attributes), `input_needs_wrap`.

Operations: put_char, put_wide_char, newline, carriage_return, backspace, scroll_up/down
(in region), erase_display, erase_line, erase_chars, insert_blank_chars, delete_chars,
insert_lines, delete_lines, resize (with Ghostty-style cell-by-cell reflow), visible_row
(viewport rendering), decaln, advance_tab, backward_tab, set_tab_stop, clear_tab_stops.

### `TermHandler` (term_handler.rs)

Implements `vte::ansi::Handler` trait (~50 methods) with mutable access to Grid (primary
+ alt), Palette, TermMode, PTY writer, tab title, active_is_alt flag, cursor_shape,
charset state, and title stack. Uses vte's high-level `Processor` which parses all
SGR/CSI/OSC sequences and calls semantic methods:
- `input(char)` -- put character at cursor with template attributes, charset mapping
- `linefeed`, `carriage_return`, `backspace`, `tab` -- cursor movement
- `goto(line, col)`, `move_up/down/forward/backward` -- CUP/CUU/CUD/CUF/CUB
- `move_down_and_cr`, `move_up_and_cr` -- CNL/CPL
- `terminal_attribute(Attr)` -- SGR: all attributes, colors, underline color (SGR 58)
- `erase_display/erase_line/erase_chars` -- ED/EL/ECH
- `insert_blank_chars/delete_chars/insert_blank_lines/delete_lines` -- ICH/DCH/IL/DL
- `set_scrolling_region` -- DECSTBM
- `scroll_up/scroll_down` -- SU/SD
- `set_mode/unset_mode` -- SM/RM for ANSI modes (Insert, LineFeedNewLine)
- `set_private_mode/unset_private_mode` -- DECSET/DECRST for 15+ private modes
- `swap_alt_screen/restore_primary_screen` -- alternate screen (1049) with cursor save
- `save_cursor_position/restore_cursor_position` -- DECSC/DECRC
- `set_title` -- OSC 0/1/2
- `push_title/pop_title` -- title stack
- `device_status` -- DSR 5 (device OK) and DSR 6 (cursor position report)
- `identify_terminal` -- DA (primary) and DA2 (secondary device attributes)
- `report_mode/report_private_mode` -- DECRPM for ANSI and private modes
- `set_cursor_style/set_cursor_shape` -- DECSCUSR
- `configure_charset/set_active_charset` -- G0-G3 charset configuration
- `dynamic_color_sequence` -- OSC 10/11/12 color queries
- `set_color/reset_color` -- OSC 4/104 palette mutation
- `set_hyperlink` -- OSC 8 hyperlinks
- `clipboard_store/clipboard_load` -- OSC 52 clipboard (base64 encode/decode)
- `text_area_size_chars/text_area_size_pixels` -- CSI 8/4 t
- `decaln` -- screen alignment test
- `reset_state` -- full terminal reset
- `bell`, `substitute` -- BEL, SUB

### `TermMode` (term_mode.rs)

Bitflags u32 tracking terminal modes: SHOW_CURSOR, APP_CURSOR, APP_KEYPAD,
LINE_WRAP, ORIGIN, INSERT, ALT_SCREEN, MOUSE_REPORT, MOUSE_MOTION, MOUSE_ALL,
SGR_MOUSE, FOCUS_IN_OUT, BRACKETED_PASTE, UTF8_MOUSE, ALTERNATE_SCROLL,
LINE_FEED_NEW_LINE.

### `Palette` (palette.rs)

270-entry color palette (256 standard + semantic colors). Methods: resolve(),
resolve_fg() (bold-bright, dim, inverse, hidden), resolve_bg() (inverse),
set_color(), reset_color(), cursor_color(). Default theme: Catppuccin Mocha.

## Event Flow

```
winit EventLoop<TermEvent>
  |
  +-- resumed()                       Create first frameless window + first tab (once)
  |                                   Pre-fill surface with BG to avoid white flash
  |
  +-- user_event(TermEvent)
  |     |
  |     +-- PtyOutput(tab_id, data)   Feed bytes through raw parser (OSC 7/133/XTVERSION)
  |                                   then VTE Processor -> Grid mutations
  |                                   Request redraw if this is the active tab
  |
  +-- window_event(window_id, event)
        |
        +-- RedrawRequested           render_window():
        |                               1. Build FrameParams (opacity, tab_bar_opacity)
        |                               2. GpuRenderer::draw_frame():
        |                                  a. Tab bar (tab_bar_opacity)
        |                                  b. Grid cells (opacity, per-cell colors,
        |                                     selection, cursor, decorations, glyphs)
        |                                  c. Search bar overlay
        |                                  d. Window border (opaque, skipped when maximized)
        |                                  e. Dropdown overlay (if open)
        |                               3. Clear color premultiplied by opacity
        |                               4. frame.present()
        |
        +-- KeyboardInput             Intercept shortcuts:
        |                               Ctrl+=       -> zoom in (change_font_size +1)
        |                               Ctrl+-       -> zoom out (change_font_size -1)
        |                               Ctrl+0       -> reset zoom (reset_font_size)
        |                               Ctrl+T       -> new_tab_in_window()
        |                               Ctrl+W       -> close_tab()
        |                               Ctrl+Tab     -> cycle tabs forward
        |                               Ctrl+S+Tab   -> cycle tabs backward
        |                               Ctrl+C       -> copy if selection, else ^C to PTY
        |                               Ctrl+V       -> paste from clipboard
        |                               Ctrl+S+C     -> copy selection
        |                               Ctrl+S+V     -> paste from clipboard
        |                               Ctrl+Insert  -> copy selection
        |                               Shift+Insert -> paste from clipboard
        |                               Escape       -> cancel drag
        |                               Shift+PgUp   -> scroll up one page
        |                               Shift+PgDn   -> scroll down one page
        |                               Shift+Home   -> scroll to top of scrollback
        |                               Shift+End    -> scroll to bottom (live)
        |                             Otherwise forward to active tab's PTY
        |                             (Enter, BS, Tab, Esc, arrows, F1-F12, etc.)
        |
        +-- MouseInput (pressed)      Check resize borders first (8px edges):
        |                               Resize zone -> drag_resize_window(direction)
        |                             Right-click: copy selection or paste
        |                             Then check tab bar hit-test:
        |                               Tab(idx)      -> start DragState + select tab
        |                               CloseTab(idx) -> close_tab()
        |                               NewTab        -> new_tab_in_window()
        |                               Minimize      -> set_minimized(true)
        |                               Maximize      -> toggle maximized
        |                               CloseWindow   -> close_window()
        |                               DragArea      -> drag_window() or double-click maximize
        |                             Grid area:
        |                               Single click  -> char selection
        |                               Double click  -> word selection
        |                               Triple click  -> line selection
        |                               Alt+click     -> block selection
        |                               Shift+click   -> extend selection
        |
        +-- MouseInput (released)     Finalize selection (auto-copy to clipboard):
        |                               TornOff    -> find_window_at_cursor (stub)
        |                               DraggingInBar -> reorder done
        |                               Pending    -> was just a click
        |
        +-- CursorMoved              Update cursor icon (resize arrows at edges)
        |                            Update hover_hit for tab bar redraw
        |                            Selection drag (update end point, auto-scroll)
        |                            Advance drag state machine:
        |                               Pending -> DraggingInBar (if dist >= 10px)
        |                               DraggingInBar -> TornOff (if vert >= 15px)
        |                               DraggingInBar -> reorder_tab_in_bar()
        |                               TornOff -> adjust window position
        |
        +-- ModifiersChanged          Track Ctrl/Shift/Alt for keyboard shortcuts
        |
        +-- CloseRequested            Remove all tabs in window, remove window,
                                      exit process if no windows remain
```

## Build / Run Instructions

### Cross-compile from WSL for Windows

```bash
cargo build --target x86_64-pc-windows-gnu --release
cp target/x86_64-pc-windows-gnu/release/oriterm.exe /mnt/c/Users/ericm/ori_term/oriterm.exe
```

Launch from Windows: `C:\Users\ericm\ori_term\oriterm.exe`

### Version

Current version: `0.1.0-alpha.1` (Cargo.toml). Build number tracked in `BUILD_NUMBER`
file and included in XTVERSION response.

### Pre-commit hooks (lefthook)

```yaml
pre-commit:
  parallel: true
  commands:
    clippy:
      glob: "*.rs"
      run: cargo clippy --target x86_64-pc-windows-gnu -- -D warnings
    build:
      glob: "*.rs"
      run: cargo build --target x86_64-pc-windows-gnu

commit-msg:
  commands:
    conventional-commit:
      run: .lefthook/commit-msg.sh {1}
```

### Debug logs

The app writes log files next to the executable:
- `oriterm_debug.log` -- runtime trace (PTY events, window creation, drag transitions)
- `oriterm_panic.log` -- panic message if the app crashes
- `oriterm_error.log` -- top-level error if `App::run()` returns `Err`

### Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `winit` | 0.30 | Window creation, event loop, input events |
| `wgpu` | 28 | GPU rendering (DX12 DComp for transparency, Vulkan/Metal fallback) |
| `window-vibrancy` | 0.7 | Compositor blur (Windows Acrylic, macOS Vibrancy) |
| `windows-sys` | 0.60 | Win32 API bindings (DWM, GDI, WM) |
| `fontdue` | 0.9.3 | Font parsing and glyph rasterization |
| `portable-pty` | 0.9.0 | Cross-platform PTY (ConPTY on Windows) |
| `vte` | 0.15 (ansi feature) | ANSI/VT escape sequence parser with high-level Handler trait |
| `bitflags` | 2 | CellFlags and TermMode bitflags |
| `log` | 0.4 | Logging macros |
| `base64` | 0.22 | OSC 52 clipboard payload encoding/decoding |
| `toml` | 0.8 | TOML config file parsing/serialization |
| `serde` | 1 (derive) | Config struct serialization/deserialization |
| `regex` | 1 | URL detection in terminal output, search regex support |
| `clipboard-win` | 5.4 | Windows clipboard access (cfg(windows) only) |
| `unicode-width` | 0.2 | Wide character width detection in term_handler and tab_bar |
| `crossterm` | 0.28 | Library scaffolding modules (not used by emulator yet) |
| `unicode-segmentation` | 1.12 | Library scaffolding (not used by emulator yet) |
| `strip-ansi-escapes` | 0.2 | Library scaffolding (not used by emulator yet) |

## Chrome Tab Drag Reference

| Chrome concept | ori_term equivalent | Value |
|---|---|---|
| `TabDragController` | `DragState` + `drag.rs` | -- |
| `kMinimumDragDistance` | `DRAG_START_THRESHOLD` | 10 px |
| `kVerticalDetachMagnetism` | `TEAR_OFF_THRESHOLD` | 15 px |
| `TabStripModel` | `App.tabs` HashMap | -- |
| `TabStrip` | `TermWindow.tabs` + `tab_bar.rs` | -- |
