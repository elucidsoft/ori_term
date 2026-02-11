---
section: "21"
title: Command Palette & Quick Terminal
status: not-started
goal: Add a searchable command palette and global hotkey drop-down terminal
sections:
  - id: "21.1"
    title: Command Palette
    status: not-started
  - id: "21.2"
    title: Quick Terminal (Drop-Down)
    status: not-started
  - id: "21.3"
    title: Desktop Notifications
    status: not-started
  - id: "21.4"
    title: Progress Indicators
    status: not-started
  - id: "21.5"
    title: Terminal Inspector
    status: not-started
  - id: "21.6"
    title: Completion Checklist
    status: not-started
---

# Section 21: Command Palette & Quick Terminal

**Status:** Not Started
**Goal:** Power-user features: searchable command palette, global-hotkey drop-down
terminal, desktop notifications, progress indicators, and a terminal inspector
for debugging.

**Why this matters:** The command palette (Ctrl+Shift+P) is becoming standard
thanks to VS Code training. Quick terminal is a killer feature on Windows where
no good drop-down terminal exists. The terminal inspector is a unique differentiator
that no other terminal except Ghostty offers.

---

## 21.1 Command Palette

Searchable overlay for all terminal actions.

- [ ] `Ctrl+Shift+P` opens a centered overlay with text input
- [ ] Fuzzy-match filter against all available actions:
  - [ ] New Tab, Close Tab, Next Tab, Previous Tab
  - [ ] Split Horizontal, Split Vertical, Close Pane
  - [ ] Copy, Paste, Select All
  - [ ] Zoom In, Zoom Out, Reset Zoom
  - [ ] Toggle Fullscreen, Toggle Maximize
  - [ ] Open Settings, Reload Config
  - [ ] Change Theme (sub-list of themes)
  - [ ] Toggle Search
  - [ ] All custom keybindings (show the binding next to the action)
- [ ] Rendering:
  - [ ] Semi-transparent overlay background
  - [ ] Input field at top
  - [ ] Scrollable list of matching actions
  - [ ] Highlight matched characters in results
  - [ ] Show keybinding shortcut on right side
- [ ] Enter executes selected action, Escape closes
- [ ] Arrow keys or Ctrl+N/P to navigate results

---

## 21.2 Quick Terminal (Drop-Down)

Global hotkey summons a terminal window.

- [ ] Config: `quick_terminal_hotkey = "F12"` (or any key combo)
- [ ] Register global hotkey (system-wide, works when ori_term is not focused):
  - [ ] Windows: `RegisterHotKey` Win32 API
  - [ ] Linux: X11 `XGrabKey` / Wayland protocol
  - [ ] macOS: `CGEvent` tap or `NSEvent.addGlobalMonitorForEvents`
- [ ] Behavior:
  - [ ] First press: create or show quick terminal window
  - [ ] Second press (while visible): hide quick terminal
  - [ ] Quick terminal is a special window (no tab bar, slides in from top)
- [ ] Config: `quick_terminal_position = top | bottom | left | right | center`
- [ ] Config: `quick_terminal_size = 50%` (percentage of screen)
- [ ] Animation: slide in/out (200ms ease-out)
- [ ] Quick terminal persists across hide/show (shell stays alive)
- [ ] Auto-hide when focus is lost (configurable)

---

## 21.3 Desktop Notifications

Surface terminal notifications to the OS.

- [ ] Parse notification sequences:
  - [ ] OSC 9 (iTerm2): `OSC 9 ; message ST`
  - [ ] OSC 777 (urxvt): `OSC 777 ; notify ; title ; body ST`
  - [ ] OSC 99 (kitty): `OSC 99 ; ... ST`
- [ ] Platform notification dispatch:
  - [ ] Windows: `ToastNotification` via `windows` crate
  - [ ] Linux: `notify-send` subprocess or D-Bus `org.freedesktop.Notifications`
  - [ ] macOS: `NSUserNotificationCenter` or `UNUserNotificationCenter`
- [ ] Click notification to focus the originating terminal tab/window
- [ ] Config: `notifications = true | false` (default: true)
- [ ] Rate limit: max 5 notifications per second per tab

---

## 21.4 Progress Indicators

Show task progress in tab bar or window title.

- [ ] Parse ConEmu OSC 9;4 progress sequences:
  - [ ] `OSC 9;4;1;N ST` — set progress to N% (0-100)
  - [ ] `OSC 9;4;2 ST` — error state (red)
  - [ ] `OSC 9;4;3 ST` — indeterminate
  - [ ] `OSC 9;4;0 ST` — clear progress
- [ ] Display progress:
  - [ ] Thin progress bar at bottom of tab label
  - [ ] Or colored overlay on tab (green fill from left)
  - [ ] Windows taskbar progress via `ITaskbarList3::SetProgressValue`
- [ ] Config: `show_progress = true | false` (default: true)

---

## 21.5 Terminal Inspector

Real-time debugging overlay for terminal developers.

- [ ] Toggle with `Ctrl+Shift+I` (like browser dev tools)
- [ ] Side panel or overlay showing:
  - [ ] **Input tab:** last N keystrokes with their encoded sequences
  - [ ] **Output tab:** raw escape sequences received from PTY (scrollable log)
  - [ ] **State tab:** current terminal modes, cursor position, grid size,
    scrollback size, active charset, cursor style
  - [ ] **Timing tab:** frame render time, PTY read throughput, FPS
- [ ] Pause button: freeze output log for inspection
- [ ] Copy button: copy selected sequences to clipboard
- [ ] Filterable: show only CSI, only OSC, only SGR, etc.

**Why unique:** Only Ghostty has this. It's invaluable for TUI developers
debugging rendering issues. Would make ori_term the go-to for terminal app development.

---

## 21.6 Completion Checklist

- [ ] Command palette opens with Ctrl+Shift+P
- [ ] Fuzzy search filters actions correctly
- [ ] Quick terminal toggles with global hotkey
- [ ] Quick terminal slides in/out from configured position
- [ ] Desktop notifications display for OSC 9/777
- [ ] Progress bars show in tab labels for OSC 9;4
- [ ] Terminal inspector shows input/output/state/timing
- [ ] All features can be disabled via config

**Exit Criteria:** Power users can discover actions via palette, summon a terminal
instantly with a hotkey, and debug terminal sequences with the inspector.
