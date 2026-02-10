# Prior Art: Console Library & Terminal Emulator Analysis

Detailed findings from studying 26 reference repositories in
`/home/eric/projects/reference_repos/console_repos/`.

---

# Part 1: Rust Console Libraries

## Crossterm

**Repository:** `/home/eric/projects/reference_repos/console_repos/crossterm`

### Terminal Width Detection

Crossterm uses platform-specific system calls to detect terminal size, returning `(columns, rows)` as `(u16, u16)`.

**Unix** (`src/terminal/sys/unix.rs`):
- Primary: Opens `/dev/tty` and uses `ioctl(fd, TIOCGWINSZ, &mut winsize)` via libc
- Fallback (if `/dev/tty` unavailable): Uses `STDOUT_FILENO`
- Second fallback: Shells out to `tput cols` / `tput lines` via `std::process::Command`
- Also supports `rustix` as an alternative to libc (feature-gated)

```rust
// src/terminal/sys/unix.rs
pub(crate) fn window_size() -> io::Result<WindowSize> {
    let mut size = winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    let file = File::open("/dev/tty").map(|file| FileDesc::new(file.into_raw_fd(), true));
    let fd = if let Ok(file) = &file { file.raw_fd() } else { STDOUT_FILENO };
    if wrap_with_result(unsafe { ioctl(fd, TIOCGWINSZ.into(), &mut size) }).is_ok() {
        return Ok(size.into());
    }
    Err(std::io::Error::last_os_error().into())
}

pub(crate) fn size() -> io::Result<(u16, u16)> {
    if let Ok(window_size) = window_size() {
        return Ok((window_size.columns, window_size.rows));
    }
    tput_size().ok_or_else(|| std::io::Error::last_os_error().into())
}
```

**Windows** (`src/terminal/sys/windows.rs`):
- Uses `ScreenBuffer::current()?.info()?.terminal_size()` via WinAPI

**Extended size**: `window_size()` also returns pixel dimensions (`ws_xpixel`, `ws_ypixel`).

### Color Support Detection

Two-layer approach:

**NO_COLOR support** (`src/style/types/colored.rs`):
```rust
pub fn ansi_color_disabled() -> bool {
    !std::env::var("NO_COLOR").unwrap_or("".to_string()).is_empty()
}
```
Result is memoized via `AtomicBool` + `parking_lot::Once` for thread-safe one-time initialization.

**Color depth detection** (`src/style.rs`):
```rust
pub fn available_color_count() -> u16 {
    #[cfg(windows)]
    { if crate::ansi_support::supports_ansi() { return u16::MAX; } }
    const DEFAULT: u16 = 8;
    env::var("COLORTERM")
        .or_else(|_| env::var("TERM"))
        .map_or(DEFAULT, |x| match x {
            _ if x.contains("24bit") || x.contains("truecolor") => u16::MAX,
            _ if x.contains("256") => 256,
            _ => DEFAULT,
        })
}
```

**Windows ANSI support** (`src/ansi_support.rs`):
```rust
pub fn supports_ansi() -> bool {
    INITIALIZER.call_once(|| {
        let supported = enable_vt_processing().is_ok()
            || std::env::var("TERM").map_or(false, |term| term != "dumb");
        SUPPORTS_ANSI_ESCAPE_CODES.store(supported, Ordering::SeqCst);
    });
    SUPPORTS_ANSI_ESCAPE_CODES.load(Ordering::SeqCst)
}
```

**`force_color_output()`** allows applications to override `NO_COLOR`.

### Unicode/Wide Characters

Crossterm itself does not handle Unicode width. It delegates this to consumers (ratatui, console crate). It outputs raw characters and escape sequences; the terminal emulator handles rendering widths.

### Terminal Resize Handling

**Unix** (`src/event/source/unix/mio.rs`):
- Registers for `SIGWINCH` signal via `signal-hook-mio`
- Uses mio event loop with dedicated `SIGNAL_TOKEN`
- On signal receipt, queries current size and emits `Event::Resize(cols, rows)`

```rust
let mut signals = Signals::new([signal_hook::consts::SIGWINCH])?;
registry.register(&mut signals, SIGNAL_TOKEN, Interest::READABLE)?;
// In event loop:
if self.signals.pending().next() == Some(signal_hook::consts::SIGWINCH) {
    let new_size = crate::terminal::size()?;
    return Ok(Some(InternalEvent::Event(Event::Resize(new_size.0, new_size.1))));
}
```

Also supports a pipe-based signal handler (`src/event/source/unix/tty.rs`).

### Key Architectural Patterns

- **Command pattern**: All terminal operations implement a `Command` trait with `write_ansi()` and optional `execute_winapi()`
- **Memoized detection**: `AtomicBool` + `Once` for ANSI support and NO_COLOR checks
- **Dual backend**: libc vs. rustix feature-gated implementations for Unix
- **Raw mode state tracking**: `parking_lot::Mutex<Option<Termios>>` stores pre-raw-mode terminal state

---

## Console (console-rs/console)

**Repository:** `/home/eric/projects/reference_repos/console_repos/console`

### Terminal Width Detection

**Unix** (`src/unix_term.rs`):
```rust
pub(crate) const DEFAULT_WIDTH: u16 = 80;

pub(crate) fn terminal_size(out: &Term) -> Option<(u16, u16)> {
    if !is_a_terminal(out) { return None; }
    let winsize = unsafe {
        let mut winsize: libc::winsize = mem::zeroed();
        libc::ioctl(out.as_raw_fd(), libc::TIOCGWINSZ.into(), &mut winsize);
        winsize
    };
    if winsize.ws_row > 0 && winsize.ws_col > 0 {
        Some((winsize.ws_row, winsize.ws_col))
    } else {
        None
    }
}
```

Returns `(rows, cols)` -- note the order difference from crossterm. Default width is 80. Validates both dimensions are > 0.

### Color Support Detection

The most comprehensive color detection of all the Rust libraries. Follows the [CLI Colors standard](https://bixense.com/clicolors/).

**Unix** (`src/unix_term.rs`):
```rust
pub(crate) fn is_a_color_terminal(out: &Term) -> bool {
    if !is_a_terminal(out) { return false; }
    if env::var("NO_COLOR").is_ok() { return false; }
    match env::var("TERM") {
        Ok(term) => term != "dumb",
        Err(_) => false,
    }
}

pub(crate) fn is_a_true_color_terminal(out: &Term) -> bool {
    if !is_a_color_terminal(out) { return false; }
    env::var("COLORTERM").is_ok_and(|term| term == "truecolor" || term == "24bit")
}
```

**Windows** (`src/windows_term/mod.rs`):
- Checks `NO_COLOR` first
- Checks MSYS TTY via `msys_tty_on()` -- if so, checks `TERM` and `COLORTERM`
- For native Windows console: tries to enable `ENABLE_VIRTUAL_TERMINAL_PROCESSING`

**Style layer** (`src/utils.rs`):
```rust
fn default_colors_enabled(out: &Term) -> bool {
    (out.features().colors_supported()
        && &env::var("CLICOLOR").unwrap_or_else(|_| "1".into()) != "0")
        || &env::var("CLICOLOR_FORCE").unwrap_or_else(|_| "0".into()) != "0"
}
```

**Separate stdout/stderr tracking** with lazy statics:
```rust
static STDOUT_COLORS: Lazy<AtomicBool> = ...;
static STDERR_COLORS: Lazy<AtomicBool> = ...;
```

Detection priority:
1. `isatty()` -- must be a terminal
2. `NO_COLOR` -- if set, disable colors
3. `TERM` -- if `"dumb"`, disable colors
4. `CLICOLOR` -- if `"0"`, disable colors (default `"1"`)
5. `CLICOLOR_FORCE` -- if not `"0"`, force colors on
6. `COLORTERM` -- `"truecolor"` or `"24bit"` for true color

### Unicode/Wide Characters

`src/utils.rs`:

```rust
pub fn measure_text_width(s: &str) -> usize {
    AnsiCodeIterator::new(s)
        .filter_map(|(s, is_ansi)| match is_ansi {
            false => Some(str_width(s)),
            true => None,
        })
        .sum()
}

fn str_width(s: &str) -> usize {
    #[cfg(feature = "unicode-width")]
    { use unicode_width::UnicodeWidthStr; s.width() }
    #[cfg(not(feature = "unicode-width"))]
    { s.chars().count() }
}
```

Also provides `truncate_str()` and `pad_str()` that correctly handle ANSI escape codes and Unicode widths, including CJK double-width characters.

### Key Architectural Patterns

- **`Term` struct** wraps `TermTarget` (Stdout/Stderr/ReadWritePair) with optional buffering
- **`TermFeatures`** provides capability queries
- **`TermFamily`** enum: `File`, `UnixTerm`, `WindowsConsole`, `Dummy`
- **Separate stdout/stderr color state** with `AtomicBool` + `Lazy`
- **`Emoji` struct** with automatic fallback based on `wants_emoji()`

---

## unicode-width

**Repository:** `/home/eric/projects/reference_repos/console_repos/unicode-width`

The foundational crate used by most Rust console libraries. Implements Unicode Standard Annex #11 (East Asian Width).

**Core API** (`src/lib.rs`):
```rust
pub trait UnicodeWidthChar: private::Sealed {
    fn width(self) -> Option<usize>;
    fn width_cjk(self) -> Option<usize>;
}

pub trait UnicodeWidthStr: private::Sealed {
    fn width(&self) -> usize;
    fn width_cjk(&self) -> usize;
}
```

**Width rules** (in order of precedence):
1. **String-level ligatures** (width differs from sum of character widths):
   - `"\r\n"` has width 1
   - Emoji ZWJ sequences: width 2
   - Emoji modifier sequences: width 2
   - Script-specific ligatures
2. **Character-level widths**:
   - `Fullwidth` / `Wide` (East Asian): width 2
   - `Ambiguous`: width 1 in non-CJK, width 2 in CJK context
   - `Default_Ignorable_Code_Point`, `Grapheme_Extend`: width 0
   - All others: width 1

**Key characteristics**:
- `#![no_std]` compatible
- `#![forbid(unsafe_code)]`
- Pure static lookup tables, no runtime allocation

---

## Ratatui

**Repository:** `/home/eric/projects/reference_repos/console_repos/ratatui`

### Terminal Width Detection

Delegates size queries to its backend abstraction.

**Backend trait** (`ratatui-core/src/backend.rs`):
```rust
pub trait Backend {
    type Error: core::error::Error;
    fn size(&self) -> Result<Size, Self::Error>;
    fn window_size(&mut self) -> Result<WindowSize, Self::Error>;
    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where I: Iterator<Item = (u16, u16, &'a Cell)>;
    fn flush(&mut self) -> Result<(), Self::Error>;
}
```

### Terminal Resize Handling

**Three viewport modes** (`ratatui-core/src/terminal/resize.rs`):

```rust
pub fn autoresize(&mut self) -> Result<(), B::Error> {
    if matches!(self.viewport, Viewport::Fullscreen | Viewport::Inline(_)) {
        let area = self.size()?.into();
        if area != self.last_known_area {
            self.resize(area)?;
        }
    }
    Ok(())
}
```

- **Fullscreen**: Auto-resizes on every `draw()` call
- **Inline**: Maintains cursor offset relative to viewport, recomputes origin on resize
- **Fixed**: Never auto-resized; manual `resize()` only

**Frame stability** (`ratatui-core/src/terminal/frame.rs`):
```rust
impl Frame<'_> {
    /// This is guaranteed not to change during rendering
    pub const fn area(&self) -> Rect { self.viewport_area }
}
```

### Key Architectural Patterns

- **Backend abstraction**: Swappable crossterm/termion/termwiz
- **Dual-buffer differential rendering**: Compares current buffer to previous, only redraws changed cells
- **Constraint-based layout**: `Length`, `Percentage`, `Min`, `Max`, `Ratio`, `Fill`
- **`u16` throughout**: All dimensions use `u16` with saturating arithmetic
- **`TestBackend`**: In-memory backend for testing without a real terminal

---

## Indicatif

**Repository:** `/home/eric/projects/reference_repos/console_repos/indicatif`

### Terminal Width Detection

Delegates to the `console` crate via the `TermLike` trait.

```rust
pub trait TermLike: Debug + Send + Sync {
    fn width(&self) -> u16;
    fn height(&self) -> u16 { 20 }
    // ...
}
```

### Key Architectural Patterns

- **Rate-limited drawing**: Default 20 Hz refresh rate
- **`TermLike` trait**: Abstracts terminal operations
- **`ProgressBar` is `Arc`-wrapped**: Cloning shares state for multi-threaded progress
- **Multi-progress**: `MultiProgress` manages multiple bars with coordinated rendering
- **Hidden when not a TTY**: Automatically degrades

---

## Dialoguer

**Repository:** `/home/eric/projects/reference_repos/console_repos/dialoguer`

### Terminal Resize Handling

The `Paging` struct polls `term.size()` on each `update()` call:

```rust
pub fn update(&mut self, cursor_pos: usize) -> Result {
    let new_term_size = self.term.size();
    if self.current_term_size != new_term_size {
        self.current_term_size = new_term_size;
        self.capacity = self.max_capacity.unwrap_or(usize::MAX)
            .clamp(3, self.current_term_size.0 as usize) - 2;
        self.term.clear_last_lines(self.capacity)?;
    }
}
```

### Key Architectural Patterns

- **Full delegation to `console` crate**
- **Default to stderr**: All prompts default to `Term::stderr()`
- **Paging system**: Auto-activates when items exceed terminal height
- **Theme system**: Pluggable `DialoguerTheme` trait

---

## Inquire

**Repository:** `/home/eric/projects/reference_repos/console_repos/inquire`

### Terminal Width Detection

```rust
pub struct TerminalSize { width: u16, height: u16 }

impl TerminalSize {
    pub fn new(width: u16, height: u16) -> Option<Self> {
        if width == 0 || height == 0 { None }
        else { Some(Self { width, height }) }
    }
}

impl Default for TerminalSize {
    fn default() -> Self { Self { width: 80, height: 24 } }
}
```

Validates non-zero dimensions at construction time.

### Key Architectural Patterns

- **Multi-backend abstraction**: Feature flags choose crossterm, termion, or console
- **Compile-time backend selection** with clear priority order
- **Separation of InputReader and Terminal** traits

---

## Rustyline

**Repository:** `/home/eric/projects/reference_repos/console_repos/rustyline`

### Color Support Detection

```rust
pub fn color_mode(&self) -> ColorMode {
    if self.color_mode == ColorMode::Enabled
        && std::env::var_os("NO_COLOR").is_some_and(|os| !os.is_empty())
    {
        return ColorMode::Disabled;
    }
    self.color_mode
}
```

Three modes: `Enabled` (default, respects `NO_COLOR`), `Forced` (always on), `Disabled` (always off).

### Unicode/Wide Characters

The most sophisticated Unicode width handling of all the Rust libraries, using a **`GraphemeClusterMode`** system (`src/layout.rs`):

```rust
pub enum GraphemeClusterMode {
    Unicode,  // Full grapheme cluster support (Apple Terminal, iTerm, WezTerm)
    WcWidth,  // Traditional wcwidth behavior (character-by-character)
    NoZwj,    // Skip zero-width joiners (kitty)
}
```

**Per-terminal detection**:
```rust
pub fn from_env() -> Self {
    match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("Apple_Terminal") => GraphemeClusterMode::Unicode,
        Ok("iTerm.app") => GraphemeClusterMode::Unicode,
        Ok("WezTerm") => GraphemeClusterMode::Unicode,
        Err(std::env::VarError::NotPresent) => match std::env::var("TERM").as_deref() {
            Ok("xterm-kitty") => GraphemeClusterMode::NoZwj,
            _ => GraphemeClusterMode::WcWidth,
        },
        _ => GraphemeClusterMode::WcWidth,
    }
}
```

**Three width calculation strategies**:
```rust
fn uwidth(s: &str) -> Unit { s.width() }           // Whole-string width (handles ligatures)
fn wcwidth(s: &str) -> Unit { /* char by char */ }  // Character-by-character
fn no_zwj(s: &str) -> Unit { /* split on ZWJ */ }   // Split on ZWJ, measure segments
```

**ANSI escape sequence filtering**: State machine tracks whether inside an escape sequence and returns width 0 for all escape components.

---

# Part 2: Non-Rust Console Libraries

## Bubbletea (Go)

**Repository:** `/home/eric/projects/reference_repos/console_repos/bubbletea`

### Architecture: The Elm Architecture (TEA)

```go
// From: bubbletea/tea.go
type Model interface {
    Init() Cmd
    Update(Msg) (Model, Cmd)
    View() string
}
```

- **Model**: Entire application state
- **Update**: Pure function: `Msg -> (Model, Cmd)`
- **View**: Pure function: `Model -> String`
- **Cmd**: Async IO operation (`func() Msg`)

### Terminal Width/Resize

```go
// bubbletea/tty.go
w, h, err := term.GetSize(p.ttyOutput.Fd())

// bubbletea/screen.go
type WindowSizeMsg struct { Width int; Height int }

// bubbletea/signals_unix.go
signal.Notify(sig, syscall.SIGWINCH)
```

### Key Design Decisions

- **Framerate-based renderer**: Batches updates within a frame interval
- **Panic recovery**: Terminal state restored even on panics
- **Terminal state restoration**: `restoreTerminalState()` restores cursor, mouse tracking, alt screen, TTY state
- **Cancelable readers**: Uses `cancelreader` for clean goroutine shutdown

---

## Lipgloss (Go)

**Repository:** `/home/eric/projects/reference_repos/console_repos/lipgloss`

### Architecture: Declarative CSS-like Styling

```go
var style = lipgloss.NewStyle().
    Bold(true).
    Foreground(lipgloss.Color("#FAFAFA")).
    Width(22)
```

### Color Detection

Delegates to termenv via `Renderer` struct with `sync.Once` caching:

```go
type Renderer struct {
    output            *termenv.Output
    colorProfile      termenv.Profile
    hasDarkBackground bool
    getColorProfile      sync.Once
    mtx sync.RWMutex
}
```

### Adaptive Colors

```go
type AdaptiveColor struct { Light string; Dark string }

func (ac AdaptiveColor) color(r *Renderer) termenv.Color {
    if r.HasDarkBackground() { return r.ColorProfile().Color(ac.Dark) }
    return r.ColorProfile().Color(ac.Light)
}
```

### Width Handling

Uses `github.com/clipperhouse/displaywidth` for accurate CJK/emoji width measurement.

---

## Termenv (Go)

**Repository:** `/home/eric/projects/reference_repos/console_repos/termenv`

### The Canonical Color Detection Implementation

```go
// termenv/termenv.go
func (o *Output) EnvNoColor() bool {
    return o.environ.Getenv("NO_COLOR") != "" ||
        (o.environ.Getenv("CLICOLOR") == "0" && !o.cliColorForced())
}

func (o *Output) EnvColorProfile() Profile {
    if o.EnvNoColor() { return Ascii }
    p := o.ColorProfile()
    if o.cliColorForced() && p == Ascii { return ANSI }
    return p
}
```

### Color Profile Detection (termenv_unix.go)

```go
func (o *Output) ColorProfile() Profile {
    if !o.isTTY() { return Ascii }
    // COLORTERM checks
    // TERM checks against known terminals:
    //   alacritty, contour, rio, wezterm, xterm-ghostty, xterm-kitty -> TrueColor
    //   linux, xterm -> ANSI
    // 256color substring -> ANSI256
}
```

### Automatic Color Downgrading (profile.go)

```go
func (p Profile) Convert(c Color) Color {
    if p == Ascii { return NoColor{} }
    switch v := c.(type) {
    case RGBColor:
        if p != TrueColor {
            ac := hexToANSI256Color(h)
            if p == ANSI { return ansi256ToANSIColor(ac) }
            return ac
        }
    }
}
```

### TTY Detection

```go
func (o *Output) isTTY() bool {
    if o.assumeTTY || o.unsafe { return true }
    if len(o.environ.Getenv("CI")) > 0 { return false }  // CI is not a TTY
    if f, ok := o.Writer().(*os.File); ok {
        return isatty.IsTerminal(f.Fd())
    }
    return false
}
```

---

## Rich (Python)

**Repository:** `/home/eric/projects/reference_repos/console_repos/rich`

### Architecture: Renderable/Segment Pipeline

Objects implement `__rich_console__` to yield `Segment` tuples of `(text, style)`.

### Color Detection

```python
# rich/console.py
self.no_color = no_color if no_color is not None else self._environ.get("NO_COLOR", "") != ""

def _detect_color_system(self):
    if self.is_jupyter: return ColorSystem.TRUECOLOR
    if not self.is_terminal or self.is_dumb_terminal: return None
    color_term = self._environ.get("COLORTERM", "").strip().lower()
    if color_term in ("truecolor", "24bit"): return ColorSystem.TRUECOLOR
    term = self._environ.get("TERM", "").strip().lower()
    _term_name, _hyphen, colors = term.rpartition("-")
    return _TERM_COLORS.get(colors, ColorSystem.STANDARD)
```

### Terminal Width

```python
@property
def size(self) -> ConsoleDimensions:
    if self._width is not None: return ConsoleDimensions(self._width, ...)
    if self.is_dumb_terminal: return ConsoleDimensions(80, 25)
    # Try os.get_terminal_size()
    # Then check COLUMNS/LINES env vars
    # Fallback: 80x25
```

### Piped Output

```python
@property
def is_terminal(self) -> bool:
    if self._force_terminal is not None: return self._force_terminal
    # FORCE_COLOR support
    force_color = environ.get("FORCE_COLOR")
    if force_color is not None: return force_color != ""
    isatty = getattr(self.file, "isatty", None)
    return False if isatty is None else isatty()

@property
def is_dumb_terminal(self) -> bool:
    _term = self._environ.get("TERM", "")
    return self.is_terminal and _term.lower() in ("dumb", "unknown")
```

---

## Textual (Python)

**Repository:** `/home/eric/projects/reference_repos/console_repos/textual`

### Architecture: CSS-Based Widget/DOM System

```python
class ClockApp(App):
    CSS = """
    Screen { align: center middle; }
    Digits { width: auto; }
    """
    def compose(self) -> ComposeResult:
        yield Digits("")
```

- **CSS for terminals**: Full CSS-like language for layout, theming, responsive design
- **Built-in testing**: `textual.testing` for headless widget testing
- **Web deployment**: Apps can run both in terminal and browser

---

## Ink (JavaScript/React)

**Repository:** `/home/eric/projects/reference_repos/console_repos/ink`

### Architecture: React Component Model with Yoga Flexbox Layout

```jsx
const Counter = () => {
    const [counter, setCounter] = useState(0);
    return <Text color="green">{counter} tests passed</Text>;
};
render(<Counter />);
```

### Key Design Decisions

- **Framerate limiting**: `maxFps: 30`
- **Incremental rendering**: Optional mode that only updates changed lines
- **Console patching**: `patchConsole: true` prevents `console.log` from interfering
- **Screen reader support**: `isScreenReaderEnabled` option

---

## Chalk (JavaScript)

**Repository:** `/home/eric/projects/reference_repos/console_repos/chalk`

### The Most Comprehensive Color Detection

Priority order in `source/vendor/supports-color/index.js`:

1. CLI flags (`--no-color`, `--color=false`, `--color=16m`)
2. `FORCE_COLOR` env var (maps to level 0-3)
3. TTY check (not a TTY and no force -> 0)
4. `TERM === 'dumb'` -> min (0 unless forced)
5. Windows version detection (build 10586 = 256, build 14931 = TrueColor)
6. CI environments:
   - GitHub Actions, Gitea, CircleCI -> level 3 (TrueColor)
   - Travis, AppVeyor, GitLab, Buildkite, Drone -> level 1
7. `COLORTERM === 'truecolor'` -> 3
8. Known `TERM` values: `xterm-kitty`, `xterm-ghostty`, `wezterm` -> 3
9. `TERM_PROGRAM`: iTerm.app v3+ -> 3, iTerm.app -> 2, Apple_Terminal -> 2
10. `TERM` regex: `/-256(color)?$/i` -> 2
11. `TERM` regex: `/^screen|^xterm|^vt100|^rxvt|color|ansi|cygwin|linux/i` -> 1
12. `COLORTERM` present -> 1

### Per-Stream Detection

```javascript
const supportsColor = {
    stdout: createSupportsColor({isTTY: tty.isatty(1)}),
    stderr: createSupportsColor({isTTY: tty.isatty(2)}),
};
```

---

## python-prompt-toolkit

**Repository:** `/home/eric/projects/reference_repos/console_repos/python-prompt-toolkit`

### Color Depth

```python
class ColorDepth(str, Enum):
    DEPTH_1_BIT = "DEPTH_1_BIT"    # Monochrome
    DEPTH_4_BIT = "DEPTH_4_BIT"    # ANSI 16 colors
    DEPTH_8_BIT = "DEPTH_8_BIT"    # 256 colors (default)
    DEPTH_24_BIT = "DEPTH_24_BIT"  # True color

    @classmethod
    def from_env(cls):
        if os.environ.get("NO_COLOR"): return cls.DEPTH_1_BIT
        if os.environ.get("PROMPT_TOOLKIT_COLOR_DEPTH") in all_values:
            return cls(os.environ["PROMPT_TOOLKIT_COLOR_DEPTH"])
        return None
```

### Color Conversion

- `_16ColorCache`: Maps RGB to closest 16 ANSI colors with exclusion support
- `_256ColorCache`: Maps RGB to closest 256-color palette, ignoring the 16 base ANSI colors to avoid color-scheme dependency

---

## fzf (Go)

**Repository:** `/home/eric/projects/reference_repos/console_repos/fzf`

### Architecture: Event-Driven Pipeline

```go
// fzf/src/core.go
/*
Reader   -> EvtReadFin
Reader   -> EvtReadNew        -> Matcher  (restart)
Terminal -> EvtSearchNew:bool -> Matcher  (restart)
Matcher  -> EvtSearchProgress -> Terminal (update info)
Matcher  -> EvtSearchFin      -> Terminal (update list)
*/
```

### NO_COLOR

```go
if os.Getenv("NO_COLOR") != "" {
    theme = tui.NoColorTheme
}
```

### ANSI State Tracking

Maintains per-line ANSI state across multi-line inputs for correct color preservation when filtering/sorting text with embedded colors.

---

## lazygit (Go)

**Repository:** `/home/eric/projects/reference_repos/console_repos/lazygit`

### Color Detection (via vendored libs)

**tcell:**
```go
if os.Getenv("NO_COLOR") != "" { /* disable */ }
```

**xo/terminfo:**
```go
func ColorLevelFromEnv() (ColorLevel, error) {
    colorTerm, termProg, forceColor := os.Getenv("COLORTERM"),
        os.Getenv("TERM_PROGRAM"), os.Getenv("FORCE_COLOR")
    // truecolor/24bit -> Millions
    // Apple_Terminal -> Hundreds
    // iTerm.app -> version-dependent
    // Fallback to terminfo MaxColors
}
```

---

# Part 3: Terminal Emulators

## Ghostty

**Repository:** `/home/eric/projects/reference_repos/console_repos/ghostty`

### Environment Variables Set

| Variable | Value | Notes |
|----------|-------|-------|
| `TERM` | `xterm-ghostty` or `xterm-256color` | Fallback if bundled terminfo not found |
| `COLORTERM` | `truecolor` | Always |
| `TERMINFO` | `<resources_dir>/../terminfo` | Bundled terminfo database |
| `TERM_PROGRAM` | `ghostty` | Used by neovim etc. for detection |
| `TERM_PROGRAM_VERSION` | build version | |
| `VTE_VERSION` | *removed* | Explicitly unset to prevent false detection |

### Color Support

256-color palette constructed in `src/terminal/color.zig`:
- 16 named ANSI colors (0-15)
- 216 color cube (16-231): `r*40+55, g*40+55, b*40+55`
- 24 grayscale ramp (232-255): `value = (i - 232) * 10 + 8`

Truecolor via `Tc` terminfo capability and `COLORTERM=truecolor`.

### Unicode/Emoji

`src/unicode/props.zig`:
```zig
pub const Properties = packed struct {
    width: u2 = 0,                    // Clamped to [0, 2]
    grapheme_break: GraphemeBreakNoControl = .other,
    emoji_vs_base: bool = false,
};
```

- Width clamped to 0-2
- Full grapheme break property support
- Emoji variation selector base detection
- HarfBuzz + CoreText/FreeType for font shaping

### Terminfo Capabilities

From `src/terminfo/ghostty.zig`:
- **Synchronized output**: `Sync` capability
- **Bracketed paste**: `BD`/`BE`, `PS`/`PE`
- **Focus reporting**: `fe`/`fd`
- **Mouse**: SGR mouse protocol via `XM`/`xm`
- **Colored underlines**: `Su`, `Setulc`, `Smulx` (curly/dashed/dotted)
- **Kitty keyboard protocol**: `fullkbd`
- **OSC 52 clipboard**: `Ms`

### Known Compatibility Problems

- tcell-based apps require primary terminfo name to equal `$TERM`
- vim breaks if terminfo name doesn't start with `xterm-`
- VTE_VERSION from other terminals causes false VTE detection

---

## Alacritty

**Repository:** `/home/eric/projects/reference_repos/console_repos/alacritty`

### Environment Variables

```rust
// alacritty_terminal/src/tty/mod.rs
pub fn setup_env() {
    let terminfo = if terminfo_exists("alacritty") { "alacritty" } else { "xterm-256color" };
    env::set_var("TERM", terminfo);
    env::set_var("COLORTERM", "truecolor");
}
```

### Terminfo Search Order

1. `$TERMINFO/<first_char>/alacritty`
2. `~/.terminfo/...`
3. Each directory in `$TERMINFO_DIRS`
4. `$PREFIX/etc/terminfo`, `$PREFIX/lib/terminfo`, `$PREFIX/share/terminfo`
5. `/etc/terminfo`, `/lib/terminfo`, `/usr/share/terminfo`
6. `/boot/system/data/terminfo` (Haiku OS)

### Color Representation

Extended 269-entry array in `alacritty_terminal/src/term/color.rs`:
- 0-15: Named ANSI
- 16-231: Color cube
- 232-255: Grayscale
- 256-268: Semantic colors (fg, bg, cursor, dim variants)

### Resize Handling

```rust
// alacritty/src/event.rs (line 1954)
if size.width == 0 || size.height == 0 { return; }
```

Zero-dimension resize explicitly guarded. Grid reflow on column change in `grid/resize.rs`.

### PTY Resize

```rust
impl OnResize for Pty {
    fn on_resize(&mut self, window_size: WindowSize) {
        let win = window_size.to_winsize();
        unsafe { libc::ioctl(self.file.as_raw_fd(), libc::TIOCSWINSZ, &win) };
    }
}
```

---

## WezTerm

**Repository:** `/home/eric/projects/reference_repos/console_repos/wezterm`

### Color Level Detection (`termwiz/src/caps/mod.rs`)

```rust
pub enum ColorLevel {
    MonoChrome,     // NO_COLOR
    Sixteen,        // Basic ANSI
    TwoFiftySix,    // 256 color
    TrueColor,      // 24-bit RGB
}
```

Detection priority:
1. `COLORTERM=truecolor` or `24bit` -> TrueColor
2. `COLORTERM` set -> TwoFiftySix
3. Terminfo `TrueColor` cap -> TrueColor
4. Terminfo `MaxColors >= 16777216` -> TrueColor
5. Terminfo `MaxColors >= 256` -> TwoFiftySix
6. `TERM` contains `256color` -> TwoFiftySix
7. Default -> Sixteen

### PTY Size

```rust
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self { PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 } }
}
```

### Environment Variables Read

`TERM`, `COLORTERM`, `COLORTERM_BCE`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`

### Windows Environment Handling

Case-insensitive env var keys on Windows via `EnvEntry` struct.

---

## xterm.js

**Repository:** `/home/eric/projects/reference_repos/console_repos/xterm.js`

### Unicode Width Implementation

`src/common/input/UnicodeV6.ts` uses a flat 64KB `Uint8Array` lookup table for BMP:

```typescript
table = new Uint8Array(65536);
table.fill(1);              // default: width 1
table[0] = 0;
table.fill(0, 1, 32);      // control chars: width 0
table.fill(0, 0x7f, 0xa0);
table.fill(2, 0x1100, 0x1160);   // Hangul Jamo: width 2
table.fill(2, 0x2e80, 0xa4d0);   // CJK unified: width 2
table.fill(2, 0xac00, 0xd7a4);   // Hangul syllables: width 2
// ... more wide ranges ...
// Then overlay combining marks as width 0
```

Supplementary plane: binary search for combining marks, range checks for CJK Extension B+.

### Addon Architecture

- `addon-unicode-graphemes`: Experimental enhanced grapheme clustering
- `addon-webgl`: GPU-accelerated rendering
- `addon-search`: Search functionality

Capabilities added via addons rather than built into core.

---

# Cross-Cutting Patterns

## Universal Color Detection Priority

1. `NO_COLOR` (any non-empty value) -> highest priority, disable color
2. `CLICOLOR_FORCE` / `FORCE_COLOR` -> force color even without TTY
3. `CLICOLOR=0` -> disable color
4. `isatty()` check
5. `TERM != "dumb"`
6. `COLORTERM` for truecolor
7. `TERM` for 256-color
8. Windows: `ENABLE_VIRTUAL_TERMINAL_PROCESSING`

## Universal Color Levels

| Level | termenv | Rich | Chalk | prompt-toolkit | WezTerm |
|-------|---------|------|-------|----------------|---------|
| None | `Ascii` | `None` | `0` | `DEPTH_1_BIT` | `MonoChrome` |
| 16 | `ANSI` | `STANDARD` | `1` | `DEPTH_4_BIT` | `Sixteen` |
| 256 | `ANSI256` | `EIGHT_BIT` | `2` | `DEPTH_8_BIT` | `TwoFiftySix` |
| 16M | `TrueColor` | `TRUECOLOR` | `3` | `DEPTH_24_BIT` | `TrueColor` |

## Terminal Size Fallback Chain

1. `ioctl(TIOCGWINSZ)` on Unix / WinAPI on Windows
2. `COLUMNS` / `LINES` env vars
3. `tput cols` / `tput lines` (crossterm only)
4. Default: 80x24

## Width Rules (Consistent Across All)

- Width 0: Control chars, combining marks, zero-width characters
- Width 1: ASCII printable, most alphabetic scripts
- Width 2: CJK ideographs, Hangul syllables, fullwidth forms, most emoji
- Supplementary plane CJK (U+20000-U+3FFFD): width 2

## Resize Safety Rules

1. Never send 0x0 resize (crashes ConPTY on Windows)
2. Always include both character and pixel dimensions in `winsize`
3. Default size: 80x24
4. Reflow text on column change (Alacritty supports this)

## Architecture Patterns

| Pattern | Libraries | Best For |
|---------|-----------|----------|
| Elm Architecture | Bubbletea, Bubbles | Stateful TUI apps |
| Segment Pipeline | Rich | Output formatting |
| CSS/DOM Widget Tree | Textual | Complex layouts |
| React Components | Ink | React developers |
| Event Pipeline | fzf | High-throughput data UIs |
| Chainable Builder | Chalk, Lipgloss | Style construction |

## Output Abstraction Pattern

Every mature library abstracts the output target:

```go
output := termenv.NewOutput(w, opts...)     // Go
```
```python
console = Console(file=my_file)             // Python
```
```typescript
render(<App />, { stdout: customStream })   // Node.js
```

Enables: testing without a real terminal, multiple simultaneous outputs, per-stream color profiles.

## Key Dependencies (Rust Ecosystem)

| Crate | Used By |
|-------|---------|
| `unicode-width` | console, rustyline, indicatif (indirect), ratatui |
| `unicode-segmentation` | rustyline |
| `signal-hook` / `signal-hook-mio` | crossterm |
| `mio` | crossterm |
| `libc` / `rustix` | crossterm, console |
| `once_cell` | console |
| `parking_lot` | crossterm |
