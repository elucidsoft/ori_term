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
**File:** `section-02-vte-sequences.md` | **Status:** In Progress

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
**File:** `section-03-scrollback.md` | **Status:** In Progress

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
**File:** `section-04-resize.md` | **Status:** In Progress

```
resize, reflow, column change, line change
PTY resize, TIOCSWINSZ, ConPTY resize
SIGWINCH, window resize event
grid resize, grow, shrink, reflow text
wide character wrap, line wrap, WRAPLINE flag
```

---

### Section 05: Color System
**File:** `section-05-color.md` | **Status:** In Progress

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
**File:** `section-06-font-system.md` | **Status:** Not Started

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
**File:** `section-07-gpu-rendering.md` | **Status:** Not Started

```
GPU, wgpu, Vulkan, Metal, DirectX, OpenGL
texture atlas, glyph atlas, atlas packing
batch rendering, draw call, instanced drawing
shader, vertex, fragment, pipeline
frame buffer, swap chain, vsync
softbuffer, CPU rendering, migration
```

---

### Section 08: Unicode & Graphemes
**File:** `section-08-unicode-graphemes.md` | **Status:** Not Started

```
unicode, grapheme cluster, grapheme break
emoji, ZWJ, zero-width joiner, variation selector
CJK, East Asian Width, fullwidth, halfwidth
combining mark, diacritical, accent
unicode width, display width, wcwidth
UAX #11, UAX #29, Unicode 16
supplementary plane, surrogate pair
```

---

### Section 09: Selection & Clipboard
**File:** `section-09-selection-clipboard.md` | **Status:** Not Started

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
**File:** `section-10-keyboard.md` | **Status:** Not Started

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
**File:** `section-11-modes-features.md` | **Status:** Not Started

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
**File:** `section-12-search.md` | **Status:** Not Started

```
search, find, regex, pattern match
scrollback search, history search
DFA, regex automata, lazy evaluation
highlight, match, next, previous
incremental search, case insensitive
```

---

### Section 13: Configuration
**File:** `section-13-configuration.md` | **Status:** Not Started

```
config, configuration, settings, preferences
TOML, config file, hot reload
shell, default shell, PowerShell, bash, zsh, WSL
font size, font family, font config
color scheme, theme, custom colors
key binding, shortcut, remap
padding, margin, opacity, blur
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

## Quick Reference

| ID | Title | File | Tier | Status |
|----|-------|------|------|--------|
| 01 | Cell & Grid Model | `section-01-cell-grid.md` | 1 | **Complete** |
| 02 | VTE Escape Sequences | `section-02-vte-sequences.md` | 1 | **In Progress** (02.1-02.5 done, OSC/DA partial) |
| 03 | Scrollback Buffer | `section-03-scrollback.md` | 1 | **In Progress** (functional, ring buffer deferred) |
| 04 | Resize Handling | `section-04-resize.md` | 1 | **In Progress** (resize works, text reflow not started) |
| 05 | Color System | `section-05-color.md` | 1 | **In Progress** (palette + render done, schemes not started) |
| 06 | Font System | `section-06-font-system.md` | 2 | Not Started |
| 07 | GPU Rendering | `section-07-gpu-rendering.md` | 2 | Not Started |
| 08 | Unicode & Graphemes | `section-08-unicode-graphemes.md` | 2 | Not Started |
| 09 | Selection & Clipboard | `section-09-selection-clipboard.md` | 2 | Not Started |
| 10 | Keyboard Protocol | `section-10-keyboard.md` | 2 | Not Started |
| 11 | Terminal Modes & Features | `section-11-modes-features.md` | 3 | Not Started |
| 12 | Search | `section-12-search.md` | 3 | Not Started |
| 13 | Configuration | `section-13-configuration.md` | 3 | Not Started |
| 14 | Cross-Platform | `section-14-cross-platform.md` | 3 | Not Started |
| 15 | Performance | `section-15-performance.md` | 3 | Not Started |
