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
- **Mouse reporting** *(pending)* — X10, normal, button-event, and any-event tracking with SGR encoding.
- **Synchronized output** *(pending)* — DCS protocol to prevent flicker during rapid updates.
- **Cursor styles** *(pending)* — block, underline, and bar cursors with blinking support.
- **Hyperlinks** *(pending)* — clickable URLs via OSC 8.
- **Bracketed paste** — paste content wrapped in escape sequences for aware applications.
- **Focus events** *(pending)* — notify applications when the terminal gains or loses focus.

### Rendering
- **GPU-accelerated** *(pending)* — wgpu-based rendering with glyph atlas texture packing and instanced drawing.
- **Damage tracking** *(pending)* — only redraws changed cells for minimal GPU work.
- **Text decorations** — underline (single, double, dotted, dashed, curly), strikethrough, with SGR 58 underline color support.

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
- **Color schemes** — 7 built-in themes with Catppuccin Mocha as default.
- **Color profile detection** *(pending)* — respects NO_COLOR, CLICOLOR, COLORTERM environment variables.

### Search
- **Scrollback search** *(pending)* — plain text and regex search through terminal output and history.
- **Incremental matching** *(pending)* — results highlighted as you type with match count and navigation.

### Keyboard
- **Kitty keyboard protocol** *(pending)* — modern unambiguous key encoding with modifier reporting.
- **IME support** *(pending)* — input method editor for CJK text entry.
- **Configurable key bindings** *(pending)* — user-definable shortcuts.

### Configuration
- **TOML config file** *(pending)* — font, colors, opacity, scrollback size, shell, cursor style, key bindings.
- **Hot reload** *(pending)* — visual settings update without restarting.

### Cross-Platform
- **Windows, Linux, macOS** *(pending)* — native PTY, clipboard, font discovery, and GPU backend on each platform.

### Safety
- **Zero `unsafe`** — the entire codebase compiles with `unsafe_code = "forbid"`.

## Building

```bash
cargo build --target x86_64-pc-windows-gnu --release
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Cycle tabs |
| `Ctrl+=` / `Ctrl+-` | Zoom in / out |
| `Ctrl+0` | Reset zoom |
| `Ctrl+C` | Copy selection (or send ^C if no selection) |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+Shift+C` / `Ctrl+Insert` | Copy selection |
| `Ctrl+Shift+V` / `Shift+Insert` | Paste from clipboard |
| `Shift+PageUp` / `Shift+PageDown` | Scroll page |
| `Shift+Home` / `Shift+End` | Scroll to top / bottom |

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
