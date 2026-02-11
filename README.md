# ori-term

A cross-platform terminal emulator written from scratch in Rust.

Built by studying 18 terminal projects from the inside out — Alacritty, Ghostty, WezTerm, Chrome, VS Code, Windows Terminal, and others.

## Features

### Window Management
- **Chrome-style tabs** — tear a tab out of the window, it becomes its own window. Drag it back in. Tabs and windows are fully decoupled; the PTY never drops.
- **Custom window chrome** — no OS title bar. The tab bar is the title bar. Pixel-drawn window controls, resize borders, native Aero Snap.
- **Multi-window** — each window independently managed with its own tab list.

### Terminal Core
- **Full VTE escape sequence handling** — SGR attributes, cursor movement, erase operations, scroll regions, alternate screen, device status reports, OSC title/clipboard/CWD/prompt markers.
- **Text reflow on resize** — cell-by-cell reflow that correctly handles wide characters and wrapped lines.
- **Scrollback** — configurable history buffer with keyboard and mouse wheel scrolling.
- **Cursor styles** — block, underline, and bar cursors.
- **Bracketed paste** — paste content wrapped in escape sequences for aware applications.
- **Mouse reporting** — X10, normal, button-event, and any-event tracking with SGR encoding.
- **Synchronized output** — DCS protocol to prevent flicker during rapid updates.
- **Hyperlinks** *(pending)* — clickable URLs via OSC 8.
- **Focus events** — notify applications when the terminal gains or loses focus.

### Rendering
- **GPU-accelerated** — wgpu-based rendering with glyph atlas texture packing and instanced drawing.
- **Window transparency** — compositor-backed glass effect with DX12 DirectComposition and Windows Acrylic blur. Per-element opacity: tab bar and grid content independently configurable.
- **Text decorations** — underline (single, double, dotted, dashed, curly), strikethrough, with SGR 58 underline color support.
- **Damage tracking** *(pending)* — only redraws changed cells for minimal GPU work.

### Font
- **Font fallback chain** — automatic fallback for missing glyphs across multiple fonts (Segoe UI Symbol, MS Gothic, Noto Sans, etc.).
- **Bold / italic variants** — real font variants with synthetic bold as fallback when no bold font is available.
- **Dynamic font sizing** — Ctrl+=/Ctrl+- to zoom, Ctrl+0 to reset. Size clamped to 8–32px.

### Unicode
- **Grapheme cluster segmentation** *(pending)* — correct handling of combining marks, ZWJ sequences, and emoji.
- **East Asian width** — proper CJK and fullwidth character rendering.
- **Ambiguous width** *(pending)* — configurable handling of ambiguous-width characters.

### Selection & Clipboard
- **Mouse selection** — click and drag to select, double-click for word, triple-click for line.
- **OSC 52 clipboard** — read/write clipboard access for applications that support it.

### Color
- **Truecolor** — 24-bit RGB, 256-color, and 16-color ANSI support.
- **Color schemes** — 7 built-in themes with Catppuccin Mocha as default. Per-color overrides (foreground, background, cursor, selection, ANSI 0-15) on top of any scheme.
- **Color profile detection** *(pending)* — respects NO_COLOR, CLICOLOR, COLORTERM environment variables.

### Search
- **Scrollback search** — plain text and regex search through terminal output and history.
- **Incremental matching** — results highlighted as you type with match count and navigation.

### Keyboard
- **Kitty keyboard protocol** — modern unambiguous key encoding with modifier reporting.
- **Configurable key bindings** — user-definable shortcuts via TOML config with hot reload.
- **IME support** *(pending)* — input method editor for CJK text entry.

### Configuration
- **TOML config file** — font, colors (scheme + per-color overrides), opacity, scrollback size, shell, cursor style, key bindings.
- **Hot reload** — visual settings update without restarting (file watcher + Ctrl+Shift+R).
- **CLI flags** — `--print-config`, `--version`, `--help`.

### Cross-Platform
- **Windows, Linux, macOS** *(pending)* — native PTY, clipboard, font discovery, and GPU backend on each platform.

### Safety
- **Minimal `unsafe`** — the codebase compiles with `unsafe_code = "deny"`, only allowed in targeted platform interop.

## Building

```bash
cargo build --target x86_64-pc-windows-gnu --release
```

## Keyboard Shortcuts

All shortcuts are configurable via the `[[keybind]]` config section.

| Shortcut | Action |
|----------|--------|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+=` / `Ctrl++` | Zoom in |
| `Ctrl+-` | Zoom out |
| `Ctrl+0` | Reset zoom |
| `Ctrl+C` | Copy selection (or send ^C if no selection) |
| `Ctrl+V` | Paste clipboard (or send paste if no clipboard) |
| `Ctrl+Shift+C` | Copy selection |
| `Ctrl+Shift+V` | Paste clipboard |
| `Ctrl+Insert` | Copy selection |
| `Shift+Insert` | Paste clipboard |
| `Ctrl+Shift+F` | Open search bar |
| `Ctrl+Shift+R` | Reload config |
| `Shift+PageUp` | Scroll page up |
| `Shift+PageDown` | Scroll page down |
| `Shift+Home` | Scroll to top |
| `Shift+End` | Scroll to bottom |

When the search bar is open, `Enter` moves to the next match, `Shift+Enter` moves to the previous match, and `Escape` closes the search bar.

## Configuration

Config file location:
- **Windows:** `%APPDATA%\ori_term\config.toml`
- **Linux:** `$XDG_CONFIG_HOME/ori_term/config.toml` (or `~/.config/ori_term/config.toml`)

Run `oriterm --print-config` to see the full default config.

### All Options

```toml
[font]
size = 16.0              # Font size in pixels (8.0 - 32.0)
# family = "Consolas"    # Font family override (default: bundled JetBrains Mono)

[terminal]
# shell = "pwsh.exe"     # Shell command (default: auto-detect)
scrollback = 10000       # Max scrollback lines
cursor_style = "block"   # "block", "bar"/"beam", or "underline"

[colors]
scheme = "Catppuccin Mocha"  # Color scheme name (see below)
# foreground = "#FFFFFF"     # Override foreground color
# background = "#000000"     # Override background color
# cursor = "#FF0000"         # Override cursor color
# selection_foreground = "#FFFFFF"   # Selection text color (default: swap with bg)
# selection_background = "#3A3D5C"   # Selection highlight color (default: swap with fg)

# Override specific ANSI colors by index (0-7)
# [colors.ansi]
# 0 = "#111111"              # Black
# 7 = "#EEEEEE"              # White

# Override specific bright colors by index (0-7 maps to colors 8-15)
# [colors.bright]
# 1 = "#FF0000"              # Bright red

[window]
columns = 120            # Initial columns
rows = 30                # Initial rows
opacity = 1.0            # Background opacity (0.0 - 1.0)
# tab_bar_opacity = 1.0  # Tab bar opacity (omit to match opacity)
blur = true              # Compositor blur behind transparent areas

[behavior]
copy_on_select = true    # Auto-copy to clipboard on mouse selection
bold_is_bright = true    # Promote ANSI 0-7 to bright when bold

# Key binding overrides (repeatable section)
# [[keybind]]
# key = "t"
# mods = "Ctrl"
# action = "NewTab"
```

### Color Schemes

Built-in: `Catppuccin Mocha`, `Catppuccin Latte`, `One Dark`, `Solarized Dark`, `Solarized Light`, `Dracula`, `Tokyo Night`.

Switch at runtime via the dropdown menu (click the `▾` button next to the `+` tab button).

### Key Binding Config

Override or unbind any default shortcut:

```toml
# Rebind Ctrl+T to close tab instead
[[keybind]]
key = "t"
mods = "Ctrl"
action = "CloseTab"

# Unbind a default
[[keybind]]
key = "w"
mods = "Ctrl"
action = "None"

# Send custom escape sequence
[[keybind]]
key = "k"
mods = "Ctrl"
action = "SendText:\\x1b[A"
```

Available actions: `Copy`, `Paste`, `SmartCopy`, `SmartPaste`, `NewTab`, `CloseTab`, `NextTab`, `PrevTab`, `ZoomIn`, `ZoomOut`, `ZoomReset`, `ScrollPageUp`, `ScrollPageDown`, `ScrollToTop`, `ScrollToBottom`, `OpenSearch`, `ReloadConfig`, `SendText:<bytes>`, `None`.

Modifier names: `Ctrl`, `Shift`, `Alt` (combine with `|`, e.g. `Ctrl|Shift`).

### Hot Reload

Edit the config file and save — changes apply immediately for visual settings (colors, font, cursor style, opacity, bold-is-bright, key bindings). Shell and scrollback size only affect new tabs. You can also press `Ctrl+Shift+R` to force a reload.

## CLI

```
oriterm [OPTIONS]

OPTIONS:
    --print-config    Print the default configuration to stdout
    --version, -V     Print version information
    --help, -h        Print this help message
```

## Inspiration

| Project | What inspired us |
|---------|-----------------|
| Ghostty | Cell-by-cell text reflow approach |
| Alacritty | Rich packed cell model, VTE crate |
| WezTerm | Cross-platform PTY abstraction |
| Chrome | Tab drag state machine and thresholds |
| VS Code | Frameless window chrome pattern |
| Windows Terminal | Selection behavior and clipboard UX |
| Catppuccin | Default color palette (Mocha) |
| ratatui | Clippy lint configuration |
| termenv / lipgloss | Color profile detection cascade |

## The Name

**ori** — from the Japanese 折り (folding). Tabs fold between windows the way you fold paper.

## License

MIT
