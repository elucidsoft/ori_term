# Terminal Core Plan Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Cell & Grid Model
**File:** `section-01-cell-grid.md` | **Status:** Complete

```
cell, grid, character, buffer, attribute, flag, bitflag
wide character, double-width, CJK, spacer, WIDE_CHAR
zero-width, combining mark, grapheme, CellExtra
foreground, background, fg, bg, bold, italic, underline
cell size, memory layout, packed struct
cursor, cursor position, saved cursor, DECSC, DECRC
```

---

### Section 02: VTE Escape Sequences
**File:** `section-02-vte-sequences.md` | **Status:** Complete

```
VTE, escape sequence, CSI, OSC, DCS, SGR
Select Graphic Rendition, color, attribute, style
scroll region, DECSTBM, scroll up, scroll down
alternate screen, DECSET, DECRST, mode
insert line, delete line, insert char, delete char
cursor save, cursor restore, cursor show, cursor hide
erase display, erase line, ED, EL
device attributes, DA, DSR, primary, secondary
window title, OSC 0, OSC 1, OSC 2
tab stop, set tab, clear tab
line feed, carriage return, newline mode
```

---

### Section 03: Scrollback Buffer
**File:** `section-03-scrollback.md` | **Status:** Complete (ring buffer optimization deferred)

```
scrollback, history, scroll, ring buffer, rotation
viewport, display offset, scroll position
storage, memory, capacity, max lines
scroll up, scroll down, page up, page down
alternate screen, no scrollback in alt screen
mouse wheel, scroll event, scroll bar
```

---

### Section 04: Resize Handling
**File:** `section-04-resize.md` | **Status:** Complete

```
resize, reflow, column change, line change
PTY resize, TIOCSWINSZ, ConPTY resize
SIGWINCH, window resize event
grid resize, grow, shrink, reflow text
wide character wrap, line wrap, WRAPLINE flag
```

---

### Section 05: Color System
**File:** `section-05-color.md` | **Status:** Complete

```
color, palette, ANSI, 256 color, truecolor, RGB
SGR, foreground, background, underline color
color cube, grayscale ramp, named colors
color scheme, theme, Catppuccin, Solarized
adaptive color, light, dark, background detection
NO_COLOR, CLICOLOR, COLORTERM, color profile
color downgrade, nearest color, convert
```

---

### Section 06: Font System
**File:** `section-06-font-system.md` | **Status:** Complete (06.9 Color Emoji deferred)

```
font, glyph, shaping, HarfBuzz, FreeType, fontdue
font fallback, font chain, font discovery
FontConfig, CoreText, DirectWrite, GDI
ligature, contextual alternates, OpenType feature
bold, italic, font variant, font style
metrics, ascent, descent, line height, cell size
emoji font, color font, COLR, CBDT
rasterization, bitmap, subpixel, antialiasing
```

---

### Section 07: GPU Rendering
**File:** `section-07-gpu-rendering.md` | **Status:** Complete (07.4 Damage Tracking deferred)

```
GPU, wgpu, Vulkan, Metal, DirectX, OpenGL, DX12
texture atlas, glyph atlas, atlas packing
batch rendering, draw call, instanced drawing
shader, vertex, fragment, pipeline, WGSL
frame buffer, swap chain, vsync, surface
instance buffer, instance writer, 80-byte stride
background pipeline, foreground pipeline, premultiplied alpha
transparency, opacity, tab_bar_opacity, blur, acrylic, vibrancy
DirectComposition, DxgiFromVisual, compositor, window-vibrancy
settings dropdown, theme selector, overlay
```

---

### Section 08: Unicode & Graphemes
**File:** `section-08-unicode-graphemes.md` | **Status:** Complete

```
unicode, grapheme cluster, grapheme break
emoji, ZWJ, zero-width joiner, variation selector
CJK, East Asian Width, fullwidth, halfwidth
combining mark, diacritical, accent, combining
unicode width, display width, wcwidth
UAX #11, UAX #29, Unicode 16
supplementary plane, surrogate pair
tab title truncation, byte length, width fix
render combining marks, overlay, fontdue
grapheme buffer, input buffering, flush
```

---

### Section 09: Selection & Clipboard
**File:** `section-09-selection-clipboard.md` | **Status:** Complete

```
selection, select, highlight, mouse select
copy, paste, clipboard, OSC 52
block selection, rectangular select
semantic selection, word select, double-click
triple-click, line select
wide character selection, grapheme selection
selection rotation, scroll during select
```

---

### Section 10: Keyboard Protocol
**File:** `section-10-keyboard.md` | **Status:** Complete (10.4 IME deferred)

```
keyboard, key event, key binding, shortcut
Kitty keyboard protocol, CSI u, progressive enhancement
bracketed paste, paste mode
modifier key, Ctrl, Alt, Shift, Super
function key, F1-F12, numpad
escape sequence, application mode, normal mode
IME, input method, compose
```

---

### Section 11: Terminal Modes & Advanced Features
**File:** `section-11-modes-features.md` | **Status:** Complete (11.5 Hyperlinks, 11.6 Images deferred)

```
terminal mode, DECSET, DECRST, private mode
mouse reporting, SGR mouse, motion, button
focus in, focus out, focus event
synchronized output, sync, DCS
bracketed paste, paste begin, paste end
cursor style, blinking, steady, bar, block, underline
origin mode, insert mode, replace mode
line wrap mode, auto wrap
hyperlink, OSC 8, URL
image, Kitty image protocol, sixel
```

---

### Section 12: Search
**File:** `section-12-search.md` | **Status:** Complete (wrapped line search deferred)

```
search, find, regex, pattern match
scrollback search, history search
search bar, overlay, Ctrl+Shift+F
highlight, match, next, previous
incremental search, case insensitive
match type, focused match, binary search
```

---

### Section 13: Configuration
**File:** `section-13-configuration.md` | **Status:** Complete (13.1-13.5 all complete)

```
config, configuration, settings, preferences
TOML, config file, hot reload, file watcher, notify
shell, default shell, PowerShell, bash, zsh, WSL
font size, font family, font config
color scheme, theme, custom colors
key binding, shortcut, remap
padding, margin, opacity, tab_bar_opacity, blur, transparency
acrylic, vibrancy, compositor, DirectComposition, DxgiFromVisual
config monitor, debounce, Ctrl+Shift+R, reload
```

---

### Section 14: Cross-Platform
**File:** `section-14-cross-platform.md` | **Status:** Not Started

```
cross-platform, Windows, Linux, macOS
ConPTY, PTY, openpty, forkpty
font path, system fonts, font directory
clipboard, Wayland, X11, Win32
GPU backend, Vulkan, Metal, DirectX, OpenGL
```

---

### Section 15: Performance
**File:** `section-15-performance.md` | **Status:** Not Started

```
performance, benchmark, profile, optimize
damage tracking, dirty region, minimal redraw
differential rendering, double buffer, diff
batch, batching, draw call reduction
memory, allocation, arena, pool
throughput, latency, frame rate, FPS
```

---

### Section 16: Split Panes
**File:** `section-16-split-panes.md` | **Status:** Not Started

```
split, pane, horizontal split, vertical split, divide
binary tree, layout tree, PaneNode, SplitDirection
split resize, drag divider, equalize, zoom pane
focus pane, navigate pane, Alt+Arrow, cycle pane
close pane, collapse split, nested split
```

---

### Section 17: Shell Integration
**File:** `section-17-shell-integration.md` | **Status:** Not Started

```
shell integration, prompt, OSC 133, semantic prompt
inject, auto-source, bash, zsh, fish, PowerShell
prompt navigation, jump to prompt, Ctrl+Shift+Up
CWD inheritance, working directory, OSC 7
smart close, idle prompt, running command
SSH integration, terminfo, ssh-terminfo
preexec, precmd, PROMPT_COMMAND, fish_prompt
```

---

### Section 18: Visual Polish
**File:** `section-18-visual-polish.md` | **Status:** Not Started

```
cursor blink, blinking, blink timer, DECSCUSR
hide cursor, hide mouse, typing, mouse move
minimum contrast, WCAG, readability, luminance
HiDPI, DPI, scale factor, display scaling, retina
smooth scroll, pixel scroll, animation, deceleration
background image, wallpaper, opacity, background
```

---

### Section 19: Font Ligatures
**File:** `section-19-font-ligatures.md` | **Status:** Not Started

```
ligature, font ligature, text shaping, HarfBuzz
rustybuzz, OpenType, calt, liga, dlig
Fira Code, JetBrains Mono, Cascadia Code
shaped glyph, glyph cluster, multi-cell glyph
programming font, arrow ligature, equals ligature
```

---

### Section 20: Theme System
**File:** `section-20-themes.md` | **Status:** Not Started

```
theme, color scheme, palette, built-in themes
Catppuccin, Dracula, Nord, Solarized, Gruvbox
Tokyo Night, One Dark, Rose Pine, Kanagawa
light theme, dark theme, auto-switch, system appearance
base16, iTerm2 colors, theme file, TOML theme
theme library, 100 themes, theme preview
```

---

### Section 21: Command Palette & Quick Terminal
**File:** `section-21-command-palette.md` | **Status:** Not Started

```
command palette, Ctrl+Shift+P, fuzzy search, action list
quick terminal, drop-down terminal, global hotkey, F12
Quake terminal, slide in, toggle terminal
desktop notification, OSC 9, OSC 777, toast
progress bar, OSC 9;4, ConEmu progress, taskbar progress
terminal inspector, debug, escape sequence log, dev tools
```

---

### Section 22: Extensibility & Advanced Features
**File:** `section-22-extensibility.md` | **Status:** Not Started

```
scripting, Lua, WASM, Rhai, plugin, extension
custom shader, WGSL, post-processing, CRT, bloom
smart paste, multi-line paste, paste warning, strip prompt
undo close tab, reopen tab, Ctrl+Shift+T
session, workspace, layout save, layout restore
```

---

## Quick Reference

| ID | Title | File | Tier | Status |
|----|-------|------|------|--------|
| 01 | Cell & Grid Model | `section-01-cell-grid.md` | 1 | **Complete** |
| 02 | VTE Escape Sequences | `section-02-vte-sequences.md` | 1 | **Complete** (OSC 52, OSC 7, OSC 133, XTVERSION, REP all done) |
| 03 | Scrollback Buffer | `section-03-scrollback.md` | 1 | **Complete** (functional; ring buffer optimization deferred to Sec 15) |
| 04 | Resize Handling | `section-04-resize.md` | 1 | **Complete** (resize + text reflow with wide char handling) |
| 05 | Color System | `section-05-color.md` | 1 | **Complete** (palette + render + 7 built-in color schemes + runtime switching) |
| 06 | Font System | `section-06-font-system.md` | 2 | **Complete** (06.9 Color Emoji deferred) |
| 07 | GPU Rendering | `section-07-gpu-rendering.md` | 2 | **Complete** (wgpu, glyph atlas, instanced rendering, full UI; 07.4 Damage Tracking deferred to Sec 15) |
| 08 | Unicode & Graphemes | `section-08-unicode-graphemes.md` | 2 | **Complete** (combining marks, ZWJ, variation selectors, selection) |
| 09 | Selection & Clipboard | `section-09-selection-clipboard.md` | 2 | **Complete** (mouse selection, copy/paste, Windows Terminal style) |
| 10 | Keyboard Protocol | `section-10-keyboard.md` | 2 | **Complete** (key_encoding.rs, Kitty protocol, legacy xterm, APP_KEYPAD; 10.4 IME deferred) |
| 11 | Terminal Modes & Features | `section-11-modes-features.md` | 3 | **Complete** (11.1–11.4 done; 11.5 Hyperlinks, 11.6 Images deferred) |
| 12 | Search | `section-12-search.md` | 3 | **Complete** (scrollback search with match highlighting; wrapped line search deferred) |
| 13 | Configuration | `section-13-configuration.md` | 3 | **Complete** (TOML config + load/save + all settings + hot reload + key bindings + opacity/blur) |
| 14 | Cross-Platform | `section-14-cross-platform.md` | 3 | Not Started |
| 15 | Performance | `section-15-performance.md` | 3 | Not Started |
| 16 | Split Panes | `section-16-split-panes.md` | 4 | Not Started |
| 17 | Shell Integration | `section-17-shell-integration.md` | 4 | Not Started |
| 18 | Visual Polish | `section-18-visual-polish.md` | 4 | Not Started |
| 19 | Font Ligatures | `section-19-font-ligatures.md` | 4 | Not Started |
| 20 | Theme System | `section-20-themes.md` | 4 | Not Started |
| 21 | Command Palette & Quick Terminal | `section-21-command-palette.md` | 4 | Not Started |
| 22 | Extensibility & Advanced Features | `section-22-extensibility.md` | 4 | Not Started |
