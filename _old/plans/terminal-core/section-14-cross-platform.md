---
section: "14"
title: Cross-Platform
status: in-progress
goal: Run on Windows, Linux, and macOS with platform-native PTY, fonts, clipboard, and GPU backends
sections:
  - id: "14.1"
    title: PTY Abstraction
    status: complete
  - id: "14.2"
    title: Platform Fonts
    status: in-progress
  - id: "14.3"
    title: Platform Clipboard
    status: in-progress
  - id: "14.4"
    title: Platform Integration
    status: in-progress
  - id: "14.5"
    title: Completion Checklist
    status: in-progress
---

# Section 14: Cross-Platform

**Status:** In Progress (14.1 complete, 14.2–14.4 partially done)
**Goal:** ori_term runs natively on Windows, Linux, and macOS with each platform
using its native PTY, font discovery, clipboard, and GPU backend.

**Inspired by:**
- Ghostty's platform abstraction with separate macOS/Linux/Windows implementations
- Alacritty's cross-platform support via crossfont and winit
- WezTerm's extensive cross-platform support including Wayland

**Current state:** Windows is the primary platform with full functionality.
`portable-pty` already provides cross-platform PTY (ConPTY on Windows,
`openpty`/`forkpty` on Unix). Font paths are defined for both Windows and Linux
(`src/render.rs`). Clipboard works on Windows via `clipboard-win` but is
stubbed out on other platforms. GPU rendering uses wgpu which auto-selects
DX12/Vulkan/Metal. Windowing is via winit which is cross-platform. The main
gaps are: clipboard on non-Windows, untested macOS/Linux builds, and some
Windows-specific code paths (DirectComposition transparency, Win32 API calls).

---

## 14.1 PTY Abstraction

**Status: Complete** — `portable-pty` handles this cross-platform.

- [x] Cross-platform PTY via `portable-pty` crate (`src/tab.rs:181`)
  - [x] Windows: ConPTY (`portable_pty::native_pty_system()`)
  - [x] Linux: `openpty` / `forkpty` (same crate, automatic)
  - [x] macOS: POSIX PTY (same crate, automatic)
- [x] PTY resize via `pty_master.resize()` (works on all platforms)
- [x] Background reader thread per tab (sends `TermEvent::PtyOutput`)
- [x] Shell detection (`src/tab.rs:258-267`):
  - [x] Windows: `cmd.exe` default (configurable via `terminal.shell`)
  - [x] Linux/macOS: reads `$SHELL` env var, defaults to `/bin/sh`
- [ ] Handle `SIGCHLD` on Unix for child process exit notification
  - [ ] Currently the PTY reader thread detects EOF when child exits
  - [ ] No explicit signal handling — may want to add for robustness

**Ref:** portable-pty crate handles the platform abstraction layer

---

## 14.2 Platform Fonts

**Status: In Progress** — paths defined for Windows and Linux, macOS untested.

- [x] Windows font discovery (`src/render.rs:42-80`):
  - [x] Scans `C:\Windows\Fonts\` for font families in priority order
  - [x] Priority: JetBrainsMono > JetBrainsMonoNerdFont > CascadiaMonoNF >
    CascadiaMono > Consolas > Courier
  - [x] Fallback fonts: Segoe UI Symbol, MS Gothic (CJK), Segoe UI
- [x] Linux font discovery (`src/render.rs:82-100+`):
  - [x] Searches `~/.local/share/fonts`, `/usr/share/fonts`, `/usr/local/share/fonts`
  - [x] Priority: JetBrainsMono > UbuntuMono > DejaVuSansMono > LiberationMono
  - [x] Fallback fonts: NotoSansMono, NotoSansSymbols2, NotoSansCJK, DejaVuSans
- [ ] macOS font discovery:
  - [ ] Currently uses Linux paths (will fail on macOS)
  - [ ] Need CoreText `CTFontCreateWithName` or scan `/Library/Fonts/`,
    `/System/Library/Fonts/`
  - [ ] Default: SF Mono, Menlo, Monaco
- [ ] Proper font discovery via platform APIs:
  - [ ] Windows: DirectWrite `IDWriteFontCollection` (more robust than path scan)
  - [ ] Linux: fontconfig (`fc-match`) or `fontconfig` crate
  - [ ] macOS: CoreText
  - [ ] Current approach works but is fragile (exact filenames required)
- [ ] Embedded fallback font:
  - [ ] Bundle a basic monospace font (e.g., JetBrains Mono) via `include_bytes!`
  - [ ] Prevents panic if no system fonts found
  - [ ] Low priority — current path-based approach works for common setups
- [ ] Config `font.family` override:
  - [x] Field exists in config (`src/config.rs:27`)
  - [ ] Need to map family name to file path on each platform

**Ref:** Alacritty `crossfont` crate, Ghostty `font/discovery.zig`

---

## 14.3 Platform Clipboard

**Status: In Progress** — Windows working, other platforms stubbed.

- [x] Windows: `clipboard-win` crate (`src/clipboard.rs:8-11, 20-23`)
  - [x] `get_text()` via `clipboard_win::get_clipboard_string()`
  - [x] `set_text()` via `clipboard_win::set_clipboard_string()`
- [ ] Linux / macOS: currently no-op stubs (`src/clipboard.rs:14-17, 26-30`)
  - [ ] `get_text()` returns `None`
  - [ ] `set_text()` returns `false`
- [ ] Replace stubs with `arboard` crate:
  - [ ] `arboard` provides cross-platform clipboard (X11, Wayland, macOS, Windows)
  - [ ] Single dependency replaces both `clipboard-win` and the stubs
  - [ ] API: `Clipboard::new()?.get_text()`, `Clipboard::new()?.set_text()`
  - [ ] Handles X11 PRIMARY vs CLIPBOARD selections
  - [ ] Handles Wayland clipboard protocol
  - [ ] Handles macOS NSPasteboard
- [ ] Alternative: keep `clipboard-win` for Windows (lighter), `arboard` for others
- [ ] OSC 52 clipboard already works on all platforms (base64 encode/decode is pure Rust)

**Ref:** `arboard` crate (well-maintained, used by Alacritty)

---

## 14.4 Platform Integration

**Status: In Progress** — wgpu and winit handle most of it automatically.

### Window Management

- [x] Frameless window with custom title bar (Windows — working)
- [x] winit cross-platform window creation
- [ ] Linux window manager integration:
  - [ ] Test with X11 and Wayland compositors
  - [ ] Some WMs may not support `drag_window()` / `drag_resize_window()`
  - [ ] May need `_NET_WM_MOVERESIZE` for X11 drag
  - [ ] Decide: frameless by default or respect WM decorations?
- [ ] macOS integration:
  - [ ] Native title bar with tab bar integration, or frameless
  - [ ] Handle `NSWindow` full screen properly
  - [ ] Menu bar integration

### GPU Backend Selection

- [x] wgpu auto-selects backend: DX12 on Windows, Vulkan on Linux, Metal on macOS
- [x] DirectComposition (DxgiFromVisual) for Windows transparency
- [ ] Linux transparency:
  - [ ] X11 composited transparency (ARGB visual)
  - [ ] Wayland compositor transparency
  - [ ] Test with Picom, KWin, Mutter
- [ ] macOS transparency:
  - [ ] NSVisualEffectView for vibrancy
  - [ ] `window-vibrancy` crate has macOS support (untested)

### Platform-Specific Code Paths

Current platform-specific code (`#[cfg(target_os = "windows")]`):
- `src/app.rs`: Window transparency setup (DComp, blur), `ShellExecuteW` for URL open
- `src/render.rs`: Font family definitions and fallback font paths
- `src/clipboard.rs`: Clipboard access
- `src/config.rs`: Config directory paths (`%APPDATA%` vs `$XDG_CONFIG_HOME`)

Each needs a working non-Windows implementation:
- [ ] URL opening: `xdg-open` on Linux, `open` on macOS (partially done in `app.rs`)
- [ ] Config paths: Linux path works, macOS needs `~/Library/Application Support/`
- [ ] Transparency: need to test/fix Linux and macOS compositor paths

### System Theme Detection

- [ ] Detect dark/light mode preference:
  - [ ] Windows: `AppsUseLightTheme` registry key
  - [ ] macOS: `NSAppearance` observation
  - [ ] Linux: `org.freedesktop.appearance.color-scheme` D-Bus
- [ ] Adapt default color scheme based on system theme

**Ref:** Ghostty platform layers, WezTerm cross-platform support

---

## 14.5 Completion Checklist

- [x] Terminal runs on Windows with ConPTY
- [ ] Terminal runs on Linux with openpty — builds but untested
- [ ] Terminal runs on macOS with openpty — builds but untested
- [x] Font discovery works on Windows
- [ ] Font discovery works on Linux — paths defined, untested
- [ ] Font discovery works on macOS — not yet defined
- [x] Clipboard copy/paste works on Windows
- [ ] Clipboard copy/paste works on Linux (arboard)
- [ ] Clipboard copy/paste works on macOS (arboard)
- [x] GPU rendering works on Windows (DX12 via wgpu)
- [ ] GPU rendering works on Linux (Vulkan via wgpu) — should work, untested
- [ ] GPU rendering works on macOS (Metal via wgpu) — should work, untested
- [x] Default shell detected correctly per platform
- [ ] Window decorations appropriate per platform
- [ ] No platform-specific panics or crashes
- [ ] CI builds for all three platforms

**Exit Criteria:** ori_term builds and runs on Windows, Linux, and macOS with
native PTY, font, and clipboard support on each platform.
