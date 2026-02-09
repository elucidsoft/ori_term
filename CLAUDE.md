# ori_console

A cross-platform GPU-accelerated terminal emulator written in Rust — in the same
category as WezTerm, Alacritty, and Ghostty. Built by studying what those projects
(plus crossterm, ratatui, bubbletea, lipgloss, termenv, rich, textual, ink, and fzf)
do well — then fixing what they don't.

This is a **terminal emulator application**, not just a TUI library. It opens a window,
renders a terminal grid, handles input, and runs shell processes. Think WezTerm, not
ratatui.

## Why This Exists

Most terminal emulators either get the basics wrong or are too complex to extend.
ori_console aims to be a correct, fast, cross-platform terminal emulator that respects
standards (NO_COLOR, CLICOLOR, Unicode width) by design, not by convention.

## Project Structure

```
ori_console/
  src/
    lib.rs              # Public API re-exports
    terminal/
      mod.rs            # Terminal struct — the single entry point
      capabilities.rs   # Color profile + feature detection
      size.rs           # Terminal size + resize handling (SIGWINCH)
      raw.rs            # Raw mode enter/exit with RAII guard
    color/
      mod.rs            # Color types and profile-aware rendering
      profile.rs        # None / ANSI(4-bit) / ANSI256(8-bit) / TrueColor(24-bit)
      detect.rs         # Detect profile from env (NO_COLOR, COLORTERM, TERM, etc.)
      adaptive.rs       # Colors that adapt to light/dark backgrounds
    style/
      mod.rs            # Style struct — bold, italic, underline, fg, bg, etc.
      render.rs         # Render styled text with profile downgrading
    text/
      mod.rs            # Text measurement and manipulation
      width.rs          # Unicode display width (UnicodeWidthStr + emoji/CJK)
      wrap.rs           # Word wrapping that respects Unicode width
      truncate.rs       # Truncation with ellipsis, width-aware
      strip.rs          # Strip ANSI escape sequences
    output/
      mod.rs            # Output target abstraction
      writer.rs         # Buffered writer that flushes atomically
      pipe.rs           # Detect and handle piped/redirected stdout
    input/
      mod.rs            # Input reading abstraction
      key.rs            # Key event types
      reader.rs         # Async key reader (crossterm-based)
    layout/
      mod.rs            # Layout primitives
      rect.rs           # Rectangular regions
      constraint.rs     # Min/max/percentage/fixed constraints
    widgets/
      mod.rs            # Widget trait
      spinner.rs        # Animated spinners
      progress.rs       # Progress bars (width-aware)
      prompt.rs         # Text input prompts
      select.rs         # Selection lists
  tests/
    integration/        # Tests that need a real or mock terminal
  examples/
    hello.rs            # Minimal example
    colors.rs           # Color profile demo
    input.rs            # Input handling demo
```

## The Rules

These are non-negotiable. Every one comes from a real bug observed across the
reference repos. The library must enforce these by design, not by convention.

### 1. Respect NO_COLOR (https://no-color.org/)

```
if env::var("NO_COLOR").is_ok() → color profile = None
```

- `NO_COLOR` being set to ANY value (even empty string) disables color
- This takes priority over everything: `COLORTERM`, `FORCE_COLOR`, `CLICOLOR`
- When color is disabled, `Style::render()` must emit plain text with zero escape sequences
- Reference: termenv, chalk, rich, console crate all implement this

### 2. Respect CLICOLOR / CLICOLOR_FORCE

```
NO_COLOR set             → no color (highest priority)
CLICOLOR_FORCE != "0"    → force color even if not a TTY
CLICOLOR == "0"          → no color
otherwise                → color if is_terminal()
```

- Reference: termenv's `ColorProfile()` and console crate's `colors_enabled()`

### 3. Detect Color Profile Correctly

Detection order (after NO_COLOR/CLICOLOR checks):

```
COLORTERM == "truecolor" || COLORTERM == "24bit"  → TrueColor
COLORTERM == "256color" || TERM contains "256color" → ANSI256
TERM is set and not "dumb"                         → ANSI (16 color)
TERM == "dumb" || not a TTY                        → None
```

Additional signals:
- `TERM_PROGRAM`: iTerm2/WezTerm/Ghostty → TrueColor
- Windows Terminal → always TrueColor
- `heuristic_only`: never query the terminal with escape sequences unless explicitly opted in

Color profile is an enum with 4 levels:
```rust
enum ColorProfile { None, Ansi, Ansi256, TrueColor }
```

Colors must **downgrade gracefully**: TrueColor → nearest ANSI256 → nearest ANSI → stripped.
Reference: lipgloss/termenv `Profile` enum, rich's `Console._color_system`

### 4. Measure Width with Unicode, Not `len()`

**Never use `str.len()` or `str.chars().count()` for display width.**

- Use `unicode-width` crate's `UnicodeWidthStr::width()`
- CJK characters are width 2
- Combining marks are width 0
- Emoji with variation selectors need special handling
- ANSI escape sequences have width 0 — strip them before measuring
- Tab characters: expand to next tab stop (default 8), or make configurable

```rust
fn display_width(s: &str) -> usize {
    strip_ansi(s).width()  // unicode-width after stripping escapes
}
```

Reference: lipgloss uses `displaywidth.String()`, ratatui uses `unicode-width`,
console crate has `measure_text_width()`, xterm.js has full wcwidth implementation

### 5. Wrap and Truncate by Display Width, Not Bytes

- Word wrap must break on display width, not byte count
- Truncation must account for wide characters — don't split a CJK char in half
- Ellipsis (`…`) is width 1, not 3 — use the Unicode character, not `...`
- When truncating, the result + ellipsis must fit within the target width

Reference: lipgloss's wrapping, rich's `Console.print()` overflow handling

### 6. Handle Piped/Redirected Output

```rust
if !stdout().is_terminal() {
    // No colors (unless CLICOLOR_FORCE)
    // No cursor manipulation
    // No progress bars / spinners
    // No \r carriage returns
    // No raw mode
    // Just plain text with newlines
}
```

- `is_terminal()` must check the actual output fd, not just stdin
- Reference: rich checks `Console.is_terminal`, console crate checks `is_term()`

### 7. Handle Terminal Resize

- Listen for `SIGWINCH` on Unix
- Re-query terminal size after signal
- **Never cache terminal size** without a way to invalidate it
- Size query returns `Option<(u16, u16)>` — handle the `None` case (no TTY)
- Default fallback: 80x24 (the universal safe assumption)
- ratatui's `autoresize()` pattern: check size before every draw, resize if changed

```rust
struct TerminalSize { width: u16, height: u16 }

impl Default for TerminalSize {
    fn default() -> Self { Self { width: 80, height: 24 } }
}
```

Reference: ratatui's `Terminal::autoresize()`, bubbletea's `WindowSizeMsg`,
crossterm's `terminal::size()`

### 8. Buffer Output, Flush Atomically

- Never write character-by-character to stdout
- Buffer the full frame, then flush once
- Use synchronized output when supported (DCS sequences)
- This prevents flicker and partial renders

```
Begin sync: ESC P = 1 s ESC \
  ... write frame ...
End sync: ESC P = 2 s ESC \
```

Reference: ghostty supports `Sync` terminfo capability, ratatui double-buffers
and diffs, bubbletea batches writes

### 9. Clean Up on Exit — Always

- Raw mode must use RAII guards (Drop impl)
- On panic, the terminal must be restored
- Set a panic hook that restores terminal state before printing the panic
- Catch SIGINT/SIGTERM and restore
- Alternate screen: if you enter it, you must leave it

```rust
struct RawModeGuard { /* restores on drop */ }

// In main:
let _guard = terminal.enter_raw_mode()?;
// Even if we panic, Drop runs and restores the terminal
```

Reference: ratatui's `restore()`, crossterm's `disable_raw_mode()`,
bubbletea's `restoreTerminalState()`

### 10. Don't Assume Terminal Width

- **All layout must be relative to current terminal width**
- Never hardcode widths
- Progress bars, tables, borders — all must query and respect actual width
- Content wider than terminal must be truncated or wrapped, never allowed to
  cause line wrapping artifacts

Reference: indicatif recalculates bar width on every tick, lipgloss's
`GetWidth()` for content width

### 11. Support the Alternate Screen Correctly

- Enter alternate screen for full-screen TUI
- Return to alternate screen on exit (so the user sees their previous output)
- Inline TUI (like fzf, bubbletea's `WithAltScreen(false)`) should NOT use
  alternate screen — render inline and clean up below cursor

Reference: bubbletea has explicit `WithAltScreen` option, ratatui has
`Viewport::Inline` vs `Viewport::Fullscreen`

### 12. Don't Break on Dumb Terminals

When `TERM=dumb` or no TERM is set:
- No escape sequences at all
- No cursor movement
- No colors
- Plain text only
- Still functional — the app should degrade, not crash

Reference: console crate checks `is_a_dumb_terminal()`, rich has `no_color` mode

## Architecture Decisions

### Use crossterm as the backend

- Cross-platform (Windows + Unix)
- Well-maintained, widely used
- Handles raw mode, key events, terminal size, cursor, screen
- Don't reinvent terminal I/O — wrap crossterm and add the rules above

### Elm Architecture for Interactive Apps

Following bubbletea's proven pattern:

```rust
trait Model {
    type Msg;
    fn update(&mut self, msg: Self::Msg) -> Command;
    fn view(&self) -> String;  // returns the full rendered output
}
```

- `update()` handles messages (key events, resize, ticks, async results)
- `view()` is a pure function from state → string
- The runtime handles input, output, and the event loop
- This separates logic from rendering completely

### Profile-Aware Color Types

Colors carry their ideal representation but render according to the detected profile:

```rust
let c = Color::rgb(255, 100, 50);
// On TrueColor terminal: ESC[38;2;255;100;50m
// On 256-color terminal: ESC[38;5;209m (nearest)
// On 16-color terminal:  ESC[91m (nearest ANSI)
// On dumb terminal:      (nothing)
```

### Width-Safe Text Primitives

Every text operation in the library must go through width-aware functions.
There should be no way to accidentally use byte length for layout.

## Dependencies

Core (required):
- `crossterm` — terminal I/O backend
- `unicode-width` — character display width
- `unicode-segmentation` — grapheme cluster iteration

Optional:
- `strip-ansi-escapes` — stripping ANSI from text for measurement

Minimize dependency tree. Every dependency must justify its existence.

## Environment Variables Reference

| Variable | Values | Effect |
|---|---|---|
| `NO_COLOR` | any | Disable all color output |
| `CLICOLOR` | `0` | Disable color |
| `CLICOLOR_FORCE` | non-`0` | Force color even when not a TTY |
| `COLORTERM` | `truecolor`, `24bit` | Enable 24-bit color |
| `TERM` | `dumb`, etc. | Terminal type for capability lookup |
| `TERM_PROGRAM` | terminal name | Identifies the terminal emulator |
| `COLUMNS` | number | Override terminal width (respected by some tools) |
| `LINES` | number | Override terminal height |
| `FORCE_COLOR` | `0`-`3` | Node.js convention, support for compat |

## Testing

- All width calculations must have tests with CJK, emoji, combining marks, ZWJ sequences
- Color detection must have tests for every env var combination
- Piped output must be tested (spawn subprocess, capture output, verify no escapes)
- Resize handling must be tested
- Raw mode cleanup must be tested (including after panic)
- Use `TestBackend` pattern (from ratatui) for rendering tests without a real terminal

## Build & Run

```bash
cargo build
cargo test
cargo run --example hello
```

### Windows Cross-Compile (from WSL)

Target: `x86_64-pc-windows-gnu` (mingw). Windows test binaries go to `C:\Users\ericm\ori_console\`.

```bash
cargo build --target x86_64-pc-windows-gnu --example hello --release
cp target/x86_64-pc-windows-gnu/release/examples/hello.exe /mnt/c/Users/ericm/ori_console/
```

Launch from Windows: `C:\Users\ericm\ori_console\hello.exe`

## Code Style

- No `unwrap()` in library code — return `Result` or provide a default
- `#[must_use]` on all builder methods
- Prefer `impl Into<Color>` and `impl AsRef<str>` for ergonomic APIs
- Keep the public API surface small — expose primitives, not internals
- Document every public item

## Current State
See [current_state.md](current_state.md) for the current implementation status.
