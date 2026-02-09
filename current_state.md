# ori_console -- Current State

## What the App Does

ori_console (binary name: `oriterm`) is a terminal emulator written in Rust with
Chrome-style tab tear-off and a custom frameless window chrome. It opens a native
window using winit (with OS decorations disabled), renders a terminal character grid
with a custom title bar using softbuffer (CPU pixel buffer) and fontdue (CPU glyph
rasterization), and runs shell processes (`cmd.exe`) through ConPTY via the
`portable-pty` crate. It is cross-compiled from WSL targeting `x86_64-pc-windows-gnu`
and runs on Windows.

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
  app.rs                       App struct, winit ApplicationHandler, event dispatch,
                               frameless window creation, resize border detection,
                               double-click maximize, initial surface fill (no white flash)
  window.rs                    TermWindow (winit Window + softbuffer Surface + tab list + is_maximized)
  tab.rs                       Tab (Grid + PTY writer + VTE parser + ConPTY handles); TabId; TermEvent
  tab_bar.rs                   Tab bar rendering and hit-testing (TabBarLayout, TabBarHit),
                               window control buttons (minimize/maximize/close), window border,
                               pixel drawing helpers (draw_x, draw_plus, draw_rect, blend, etc.)
  drag.rs                      DragState / DragPhase -- Chrome-style drag state machine
  grid.rs                      Grid / Cell -- terminal character cell buffer
  render.rs                    GlyphCache (fontdue), render_grid(), render_text(), load_font()
  vte_performer.rs             Performer -- vte::Perform impl driving Grid from PTY output

  (Library scaffolding modules -- present but not used by the terminal emulator yet)
  color/                       Color types, profile detection (NO_COLOR, CLICOLOR, COLORTERM)
  style/                       Style struct, profile-aware rendering
  text/                        Unicode width, wrapping, truncation, ANSI stripping
  terminal/                    Terminal struct, size, raw mode (crossterm-based)
  output/                      Buffered writer, pipe detection
  input/                       Key events, async reader
  layout/                      Rect, constraints
  widgets/                     Spinner, progress, prompt, select
```

### Data Flow

1. **Startup** (`App::run`): Sets a panic hook, loads a monospace font from
   `C:\Windows\Fonts\` (CascadiaMono > Consolas > Courier), builds a `GlyphCache`,
   creates a winit `EventLoop<TermEvent>`, and calls `event_loop.run_app(&mut app)`.

2. **Window + Tab creation** (`resumed` handler): On first resume, creates one
   frameless window (`with_decorations(false)`) and spawns one tab inside it. The
   surface is immediately filled with the background color to prevent a white flash.
   Each tab opens a ConPTY pair, spawns `cmd.exe` on the slave side, and starts a
   background reader thread.

3. **PTY output**: The reader thread sends `TermEvent::PtyOutput(tab_id, data)` through
   the `EventLoopProxy`. The `user_event` handler feeds the data through the VTE parser,
   which calls `Performer` methods that mutate the tab's `Grid`. Then the window
   containing the active tab is asked to redraw.

4. **Rendering** (`render_window`): Resizes the softbuffer surface, fills with background
   color (black `#000000`), calls `tab_bar::render_tab_bar()` for the custom title bar
   with tabs and window controls, then `render::render_grid()` for the active tab's grid
   (offset by `GRID_PADDING_LEFT` and `TAB_BAR_HEIGHT + 10`). Finally draws a 1px
   window border (`render_window_border`, skipped when maximized) and calls
   `buffer.present()`.

5. **Input**: Keyboard events are intercepted for built-in shortcuts (Ctrl+T new tab,
   Ctrl+W close tab, Ctrl+Tab / Ctrl+Shift+Tab cycle tabs, Escape cancel drag), then
   forwarded to the active tab's PTY writer as raw bytes or escape sequences.

6. **Mouse / Drag**: Mouse presses on the tab bar trigger hit-testing (`TabBarLayout`).
   Clicking a tab selects it; clicking the close button closes it; clicking the "+"
   button creates a new tab. Window control buttons handle minimize, maximize, and
   close. Empty space in the tab bar is a drag area (calls `drag_window()` for native
   OS drag, double-click toggles maximize). A press-and-hold on a tab initiates a
   `DragState` which transitions through `Pending -> DraggingInBar -> TornOff`.

7. **Resize borders**: Mouse near window edges (8px) triggers `drag_resize_window()`
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
  style icons — minimize (horizontal line), maximize (rectangle / overlapping restore
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

- **No white flash on startup**: Surface is pre-filled with background color immediately
  after window creation.

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
  pair; `cmd.exe` is spawned on the slave. The `_pty_master` and `_child` handles
  are kept alive in the `Tab` struct -- dropping either would kill the ConPTY or the
  child process. A background thread reads PTY output and sends it to the event loop
  via `EventLoopProxy`.

- **VTE parsing**: The `vte` crate parses the PTY byte stream. The `Performer`
  implements cursor movement (CUP, CUU, CUD, CUF, CUB), erase (ED mode 2/3, EL
  mode 0), device status report (DSR 6 -- cursor position response), tab stops, and
  printable character output.

- **Font rendering**: fontdue rasterizes glyphs from system monospace fonts. The
  `GlyphCache` lazily caches rasterized glyphs. Alpha blending is done per-pixel
  against the background.

- **Keyboard input forwarding**: Enter, Backspace, Tab, Escape, arrow keys, Home,
  End, Delete, and printable text are all forwarded to the PTY with correct escape
  sequences. The shell is interactive -- cmd.exe works.

- **Tab reordering**: While dragging within the tab bar (before tear-off), tabs can
  be reordered by horizontal cursor position.

- **Multi-window**: Each window is independently managed with its own surface and tab
  list. Closing the last window exits the application.

- **Escape cancels drag**: Pressing Escape during any drag phase reverts the drag
  and redraws all windows.

- **Panic/error logging**: Panics are caught and written to `ori_console_panic.log`.
  Runtime trace goes to `ori_console_debug.log`. Top-level errors go to
  `ori_console_error.log`.

## What's Not Working Yet / Known Issues

- **Re-attach not implemented**: `find_window_at_cursor()` is a stub that always
  returns `None`. Dropping a torn-off tab over another window's tab bar does not
  merge it. The `reattach_tab()` method exists and is structurally complete, but
  never gets called because the hit-detection logic needs screen-space coordinate
  comparison against each window's outer position and size.

- **Torn-off window drag tracking has positioning bugs**: The `TornOff` phase
  adjusts window position via `set_outer_position` based on the difference between
  cursor position and grab offset. This can cause jitter or drift because
  `CursorMoved` events report positions relative to the window's own client area,
  which shifts as the window moves. A more robust approach would use absolute screen
  coordinates or platform-specific drag APIs.

- **cmd.exe shell works but first tab may need testing**: The first tab spawns
  `cmd.exe` and immediately starts reading. On some systems the initial prompt may
  take a moment to appear or may require a resize event to trigger a full redraw.
  There is no explicit "wait for prompt" logic.

- **No per-cell color attributes**: `Cell` stores only `fg` as a `u32` and always
  uses the global `FG` constant in `put_char`. SGR (Select Graphic Rendition)
  sequences for foreground color, background color, bold, italic, underline, etc.
  are not handled in the `Performer`. All text renders in one color.

- **No scrollback buffer**: `Grid::scroll_up()` discards the top row permanently.
  There is no scrollback history and no scroll UI.

- **No window resize handling**: The grid is fixed at 120 columns x 30 rows.
  Resizing the OS window does not resize the grid or notify the PTY of new
  dimensions.

- **No selection / copy-paste**: No text selection with the mouse and no clipboard
  integration.

- **Hardcoded shell**: Always spawns `cmd.exe`. No configuration for PowerShell,
  WSL, or other shells.

- **Font loading is Windows-only**: `load_font()` looks for fonts at hardcoded
  `C:\Windows\Fonts\` paths. This will panic on Linux or macOS.

- **Limited VTE coverage**: Many sequences are unhandled -- scroll regions,
  insert/delete line, SGR attributes, alternate screen buffer, mouse reporting,
  bracketed paste, OSC sequences (window title, hyperlinks), etc.

- **Tab title truncation uses byte length**: In `tab_bar.rs`, `title.len()` is
  compared against a character count, which is incorrect for multi-byte or wide
  characters.

- **Static tab titles**: Titles are "Tab N" and do not update from OSC title
  sequences.

## Key Data Structures

### `App` (app.rs)

Top-level application state. Owns all windows (`HashMap<WindowId, TermWindow>`),
all tabs (`HashMap<TabId, Tab>`), the shared `GlyphCache`, the current `DragState`,
per-window cursor positions and hover states, keyboard modifier state, double-click
tracking (`last_click_time`, `last_click_window`), and the `EventLoopProxy` for
sending `TermEvent`s from background threads. Implements
`ApplicationHandler<TermEvent>` for the winit event loop.

The separation of tabs from windows (each in their own HashMap) is the key design
that enables tab tear-off: moving a tab between windows is just updating two
`Vec<TabId>` lists without touching the PTY or Grid.

### `TermWindow` (window.rs)

Per-window state: an `Arc<Window>` (winit), a softbuffer `Context` and `Surface`
for pixel rendering, a `Vec<TabId>` of tabs in display order, the `active_tab`
index, and `is_maximized` for tracking maximize state. Methods: `add_tab`,
`remove_tab`, `active_tab_id`, `tab_index`.

### `Tab` / `TabId` (tab.rs)

Each tab represents a running shell session. Contains:
- `Grid` -- the character cell buffer
- `pty_writer` -- `Option<Box<dyn Write + Send>>` for writing to the PTY
- `vte_parser` -- `vte::Parser` for parsing escape sequences
- `title` -- display string
- `_pty_master` -- `Box<dyn MasterPty>`, must stay alive to keep ConPTY open
- `_child` -- `Box<dyn Child>`, must stay alive to keep the shell process running

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

### `GlyphCache` (render.rs)

Wraps a `fontdue::Font` and a `HashMap<char, (Metrics, Vec<u8>)>` of rasterized
glyphs. Provides `ensure(ch)` to lazily rasterize and `get(ch)` to retrieve.
Computes `cell_width`, `cell_height`, and `baseline` from font line metrics at
construction time. Font size is 16.0 px.

### `Grid` / `Cell` (grid.rs)

`Grid` is a flat `Vec<Cell>` of size `cols * rows` with a cursor position
(`cursor_col`, `cursor_row`). Supports `put_char`, `newline`, `carriage_return`,
`backspace`, `scroll_up`, `clear`, and `erase_line_from_cursor`.

`Cell` holds a `char` and an `fg` color as `u32`. Background color is a global
constant (`BG = 0x00000000`, black). Cursor color is `0x00f5e0dc` (Catppuccin
rosewater).

### `Performer` (vte_performer.rs)

Implements `vte::Perform` with mutable references to a `Grid` and the PTY writer.
Translates VTE actions into grid mutations:
- `print(char)` -- put character at cursor
- `execute(byte)` -- LF, CR, BS, HT
- `csi_dispatch` -- CUP/CUF/CUB/CUU/CUD, ED, EL, DSR
- `esc_dispatch`, `osc_dispatch`, `hook`, `put`, `unhook` -- no-ops

## Event Flow

```
winit EventLoop<TermEvent>
  |
  +-- resumed()                       Create first frameless window + first tab (once)
  |                                   Pre-fill surface with BG to avoid white flash
  |
  +-- user_event(TermEvent)
  |     |
  |     +-- PtyOutput(tab_id, data)   Feed bytes through VTE parser -> Grid mutations
  |                                   Request redraw if this is the active tab
  |
  +-- window_event(window_id, event)
        |
        +-- RedrawRequested           render_window():
        |                               1. Resize softbuffer surface
        |                               2. Fill background (black)
        |                               3. render_tab_bar() with hover state + is_maximized
        |                               4. render_grid() for active tab (with padding offsets)
        |                               5. render_window_border() (1px, skipped when maximized)
        |                               6. buffer.present()
        |
        +-- KeyboardInput             Intercept shortcuts:
        |                               Ctrl+T     -> new_tab_in_window()
        |                               Ctrl+W     -> close_tab()
        |                               Ctrl+Tab   -> cycle tabs forward
        |                               Ctrl+S+Tab -> cycle tabs backward
        |                               Escape     -> cancel drag
        |                             Otherwise forward to active tab's PTY
        |
        +-- MouseInput (pressed)      Check resize borders first (8px edges):
        |                               Resize zone -> drag_resize_window(direction)
        |                             Then hit-test tab bar:
        |                               Tab(idx)      -> start DragState + select tab
        |                               CloseTab(idx) -> close_tab()
        |                               NewTab        -> new_tab_in_window()
        |                               Minimize      -> set_minimized(true)
        |                               Maximize      -> toggle maximized
        |                               CloseWindow   -> close_window()
        |                               DragArea      -> drag_window() or double-click maximize
        |
        +-- MouseInput (released)     Finalize drag:
        |                               TornOff    -> find_window_at_cursor (stub)
        |                               DraggingInBar -> reorder done
        |                               Pending    -> was just a click
        |
        +-- CursorMoved              Update cursor icon (resize arrows at edges)
        |                            Update hover_hit for tab bar redraw
        |                            Advance drag state machine:
        |                               Pending -> DraggingInBar (if dist >= 10px)
        |                               DraggingInBar -> TornOff (if vert >= 15px)
        |                               DraggingInBar -> reorder_tab_in_bar()
        |                               TornOff -> adjust window position
        |
        +-- ModifiersChanged          Track Ctrl/Shift for keyboard shortcuts
        |
        +-- CloseRequested            Remove all tabs in window, remove window,
                                      exit process if no windows remain
```

## Build / Run Instructions

### Cross-compile from WSL for Windows

```bash
cargo build --target x86_64-pc-windows-gnu --release
cp target/x86_64-pc-windows-gnu/release/oriterm.exe /mnt/c/Users/ericm/ori_console/oriterm.exe
```

Launch from Windows: `C:\Users\ericm\ori_console\oriterm.exe`

### Debug logs

The app writes log files next to the executable:
- `ori_console_debug.log` -- runtime trace (PTY events, window creation, drag transitions)
- `ori_console_panic.log` -- panic message if the app crashes
- `ori_console_error.log` -- top-level error if `App::run()` returns `Err`

### Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `winit` | 0.30 | Window creation, event loop, input events |
| `softbuffer` | 0.4 | CPU pixel buffer presented to the OS window |
| `fontdue` | 0.9 | Font parsing and glyph rasterization |
| `portable-pty` | 0.9 | Cross-platform PTY (ConPTY on Windows) |
| `vte` | 0.15 | ANSI/VT escape sequence parser |
| `crossterm` | 0.28 | Library modules (not used by emulator yet) |
| `unicode-width` | 0.2 | Library modules (not used by emulator yet) |
| `unicode-segmentation` | 1.12 | Library modules (not used by emulator yet) |
| `strip-ansi-escapes` | 0.2 | Library modules (not used by emulator yet) |

## Chrome Tab Drag Reference

| Chrome concept | ori_console equivalent | Value |
|---|---|---|
| `TabDragController` | `DragState` + `drag.rs` | -- |
| `kMinimumDragDistance` | `DRAG_START_THRESHOLD` | 10 px |
| `kVerticalDetachMagnetism` | `TEAR_OFF_THRESHOLD` | 15 px |
| `TabStripModel` | `App.tabs` HashMap | -- |
| `TabStrip` | `TermWindow.tabs` + `tab_bar.rs` | -- |
