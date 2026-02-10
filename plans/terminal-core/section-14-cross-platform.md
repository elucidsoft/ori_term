---
section: "14"
title: Cross-Platform
status: not-started
goal: Run on Windows, Linux, and macOS with platform-native PTY, fonts, clipboard, and GPU backends
sections:
  - id: "14.1"
    title: PTY Abstraction
    status: not-started
  - id: "14.2"
    title: Platform Fonts
    status: not-started
  - id: "14.3"
    title: Platform Clipboard
    status: not-started
  - id: "14.4"
    title: Platform Integration
    status: not-started
  - id: "14.5"
    title: Completion Checklist
    status: not-started
---

# Section 14: Cross-Platform

**Status:** Not Started
**Goal:** ori_term runs natively on Windows, Linux, and macOS with each platform
using its native PTY, font discovery, clipboard, and GPU backend.

**Inspired by:**
- Ghostty's platform abstraction with separate macOS/Linux/Windows implementations
- Alacritty's cross-platform support via crossfont and winit
- WezTerm's extensive cross-platform support including Wayland

**Current state:** Windows-only. ConPTY for PTY, winit for windowing, softbuffer
for rendering. Font paths hardcoded to Windows locations. No Linux/macOS support.

---

## 14.1 PTY Abstraction

Abstract PTY creation across platforms.

- [ ] Define `Pty` trait:
  ```rust
  trait Pty {
      fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
      fn write(&mut self, data: &[u8]) -> io::Result<usize>;
      fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()>;
  }
  ```
- [ ] Windows: ConPTY (current implementation in `tab.rs`)
  - [ ] Already working via `windows` crate
  - [ ] `CreatePseudoConsole`, `ResizePseudoConsole`
- [ ] Linux: `openpty` / `forkpty`
  - [ ] Use `libc` crate for `openpty()` and `forkpty()`
  - [ ] Resize via `TIOCSWINSZ` ioctl
  - [ ] Handle `SIGCHLD` for child process exit
- [ ] macOS: same as Linux (POSIX PTY)
  - [ ] Uses same `openpty` / `forkpty` API
  - [ ] May need `posix_openpt` on some versions
- [ ] Shell detection:
  - [ ] Windows: `cmd.exe` or `powershell.exe` or `pwsh.exe`
  - [ ] Linux/macOS: read `$SHELL` env var, default to `/bin/sh`
  - [ ] WSL detection: if running in WSL, use `bash` or configured shell

**Ref:** Alacritty `tty/` module (windows.rs, unix.rs), Ghostty PTY abstraction

---

## 14.2 Platform Fonts

Platform-specific font discovery and loading.

- [ ] Windows font discovery:
  - [ ] Scan `C:\Windows\Fonts\` (current approach)
  - [ ] Or use DirectWrite `IDWriteFontCollection` for proper enumeration
  - [ ] Default: Cascadia Mono, Consolas
- [ ] Linux font discovery:
  - [ ] Use fontconfig (`fc-match`, `fc-list`)
  - [ ] Parse fontconfig output or use `fontconfig` crate
  - [ ] Default: system monospace, DejaVu Sans Mono, Liberation Mono
- [ ] macOS font discovery:
  - [ ] Use CoreText `CTFontCreateWithName`
  - [ ] Default: SF Mono, Menlo, Monaco
- [ ] Embedded fallback font:
  - [ ] Bundle a basic monospace font (e.g., JetBrains Mono) as last resort
  - [ ] Include at compile time via `include_bytes!`
  - [ ] Prevents panic if no system fonts found

**Ref:** Alacritty `crossfont` crate, Ghostty `font/discovery.zig`

---

## 14.3 Platform Clipboard

Platform-native clipboard access.

- [ ] Windows: Win32 clipboard API
  - [ ] `OpenClipboard`, `GetClipboardData(CF_UNICODETEXT)`, `SetClipboardData`
  - [ ] Or use `clipboard-win` crate
- [ ] Linux: X11 / Wayland clipboard
  - [ ] X11: `XSetSelectionOwner` / `XGetSelectionOwner` (PRIMARY + CLIPBOARD)
  - [ ] Wayland: `wl_data_device` protocol
  - [ ] Or use `arboard` crate for cross-platform abstraction
- [ ] macOS: NSPasteboard
  - [ ] `pbcopy` / `pbpaste` for simple approach
  - [ ] Or use `arboard` crate
- [ ] Consider using `arboard` crate for all platforms (simpler, well-maintained)

**Ref:** Alacritty clipboard handling, `arboard` crate

---

## 14.4 Platform Integration

Platform-specific window and system integration.

- [ ] Window management:
  - [ ] Windows: frameless window with custom title bar (current implementation)
  - [ ] Linux: respect window manager decorations, or offer frameless option
  - [ ] macOS: native title bar with tab bar integration, or frameless
- [ ] Notifications:
  - [ ] OSC 9 (iTerm2) / OSC 777 (urxvt) notification sequences
  - [ ] Windows: `ToastNotification` or system tray balloon
  - [ ] Linux: `notify-send` / D-Bus notifications
  - [ ] macOS: `NSUserNotification` / `UNUserNotificationCenter`
- [ ] GPU backend selection:
  - [ ] Windows: DirectX 12 primary, Vulkan fallback
  - [ ] Linux: Vulkan primary, OpenGL fallback
  - [ ] macOS: Metal
  - [ ] wgpu handles this automatically, but allow user override
- [ ] System theme detection:
  - [ ] Detect dark/light mode preference
  - [ ] Adapt default colors or offer "auto" color scheme

**Ref:** Ghostty platform layers, WezTerm cross-platform support

---

## 14.5 Completion Checklist

- [ ] Terminal runs on Windows with ConPTY
- [ ] Terminal runs on Linux with openpty
- [ ] Terminal runs on macOS with openpty
- [ ] Font discovery works on all platforms
- [ ] Clipboard copy/paste works on all platforms
- [ ] GPU rendering works on all platforms (via wgpu)
- [ ] Default shell detected correctly per platform
- [ ] Window decorations appropriate per platform
- [ ] No platform-specific panics or crashes
- [ ] CI builds for all three platforms

**Exit Criteria:** ori_term builds and runs on Windows, Linux, and macOS with
native PTY, font, and clipboard support on each platform.
