---
section: 27
title: Command Palette & Quick Terminal
status: not-started
tier: 7
goal: Searchable command palette, global hotkey drop-down terminal, notifications, progress, inspector
sections:
  - id: "27.1"
    title: Command Palette
    status: not-started
  - id: "27.2"
    title: Quick Terminal (Drop-Down)
    status: not-started
  - id: "27.3"
    title: Desktop Notifications
    status: not-started
  - id: "27.4"
    title: Progress Indicators
    status: not-started
  - id: "27.5"
    title: Terminal Inspector
    status: not-started
  - id: "27.6"
    title: Section Completion
    status: not-started
---

# Section 27: Command Palette & Quick Terminal

**Status:** Not Started
**Goal:** Power-user features: searchable command palette, global-hotkey drop-down terminal, desktop notifications, progress indicators, and a terminal inspector for debugging.

**Crate:** `oriterm` (app + rendering layer)
**Dependencies:** `fuzzy-matcher` (optional, for palette search), platform crates for global hotkey and notifications

**Why this matters:** The command palette (Ctrl+Shift+P) is becoming standard thanks to VS Code training. Quick terminal is a killer feature on Windows where no good drop-down terminal exists. The terminal inspector is a unique differentiator that no other terminal except Ghostty offers.

**Current state:** A settings dropdown overlay exists that renders a theme selector list. This overlay infrastructure (semi-transparent background, text rendering, click hit-testing) can be reused for the command palette.

---

## 27.1 Command Palette

Searchable overlay for all terminal actions.

**File:** `oriterm/src/app.rs` (overlay state, input), `oriterm/src/gpu/render_overlay.rs` (rendering), `oriterm/src/keybindings.rs` (action registry)

**Reference:** `_old/src/app/mod.rs` (dropdown overlay), `_old/src/gpu/render_overlay.rs` (overlay rendering), `_old/src/keybindings/defaults.rs` (action enum)

- [ ] Trigger: `Ctrl+Shift+P` opens a centered overlay
- [ ] Action registry:
  - [ ] Enumerate all bindable actions from `keybindings::Action` enum
  - [ ] Each action has: display name, description, keybinding (if any)
  - [ ] Example: `("New Tab", "Open a new terminal tab", "Ctrl+T")`
  - [ ] Include dynamically generated actions (theme list, etc.)
- [ ] Fuzzy matching:
  - [ ] Simple substring or subsequence match on action name
  - [ ] Score by: exact prefix > word prefix > subsequence
  - [ ] Could use `fuzzy-matcher` crate or simple custom implementation
  - [ ] Update results on every keystroke (incremental)
- [ ] Rendering (reuse dropdown overlay infrastructure):
  - [ ] Semi-transparent dark overlay covering the terminal grid
  - [ ] Input field at top (rendered text with cursor)
  - [ ] Scrollable list of matching actions below
  - [ ] Highlight matched characters in results (bold or accent color)
  - [ ] Show keybinding shortcut right-aligned on each row
  - [ ] Selected item highlighted with accent background
- [ ] Input handling:
  - [ ] Text input: append to query, re-filter
  - [ ] Backspace: remove last char from query
  - [ ] Arrow Up/Down or Ctrl+N/P: navigate results
  - [ ] Enter: execute selected action, close palette
  - [ ] Escape: close palette without executing
  - [ ] Tab: autocomplete to selected action name (optional)
- [ ] Actions available in palette:
  - [ ] New Tab, Close Tab, Next Tab, Previous Tab
  - [ ] Split Horizontal, Split Vertical, Close Pane (Section 25)
  - [ ] Copy, Paste, Select All
  - [ ] Zoom In, Zoom Out, Reset Zoom
  - [ ] Toggle Fullscreen, Toggle Maximize
  - [ ] Open Settings File, Reload Config
  - [ ] Change Theme -- sub-list of all available themes
  - [ ] Toggle Search Bar
  - [ ] Previous Prompt, Next Prompt (shell integration)
  - [ ] All user-configured keybindings

**Tests:**
- [ ] Ctrl+Shift+P opens palette overlay
- [ ] Typing filters action list by fuzzy match
- [ ] Exact prefix match scores higher than subsequence
- [ ] Arrow keys navigate result list
- [ ] Enter executes selected action
- [ ] Escape closes palette without side effects
- [ ] All registered actions appear in unfiltered list

---

## 27.2 Quick Terminal (Drop-Down)

Global hotkey summons a terminal window from any application.

**File:** `oriterm/src/app.rs` (window management), `oriterm/src/platform_windows.rs` (global hotkey)

**Reference:** `_old/src/app/window_management.rs` (window creation), `_old/src/platform_windows.rs` (Win32 integration)

- [ ] Config:
  ```toml
  [behavior]
  quick_terminal_hotkey = "F12"
  quick_terminal_position = "top"      # top | bottom | center
  quick_terminal_size = 50             # percentage of screen height
  quick_terminal_animation = true
  ```
- [ ] Register global hotkey (system-wide, works when ori_term is not focused):
  - [ ] Windows: `RegisterHotKey` Win32 API via `windows-sys`
    - [ ] Runs even when app is minimized/unfocused
    - [ ] Message loop receives `WM_HOTKEY`
    - [ ] Map from winit event loop: use a background thread monitoring `GetMessage` for `WM_HOTKEY`, send `TermEvent::QuickTerminalToggle`
  - [ ] Linux: X11 `XGrabKey` (requires display connection)
    - [ ] Wayland: no standard global hotkey protocol (may need D-Bus or compositor-specific)
  - [ ] macOS: `CGEvent` tap or `NSEvent.addGlobalMonitorForEvents`
- [ ] Behavior:
  - [ ] First press: create or show quick terminal window
  - [ ] Second press (while visible): hide quick terminal
  - [ ] Quick terminal is a special window:
    - [ ] No tab bar (single full-height terminal pane)
    - [ ] Frameless, positioned at screen edge
    - [ ] `always_on_top` flag set
  - [ ] Quick terminal persists across hide/show (shell stays alive)
  - [ ] Auto-hide when focus is lost (configurable): `quick_terminal_autohide = true | false`
- [ ] Animation:
  - [ ] Slide in from configured edge (200ms ease-out)
  - [ ] Slide out on hide (150ms ease-in)
  - [ ] Use `ControlFlow::WaitUntil` for animation frames
  - [ ] Or: just show/hide instantly (animation is polish, not MVP)

**Tests:**
- [ ] Global hotkey registration succeeds on target platform
- [ ] First press creates quick terminal window
- [ ] Second press hides the window (shell stays alive)
- [ ] Third press shows the window again
- [ ] Quick terminal has no tab bar
- [ ] Auto-hide triggers when window loses focus
- [ ] Config position modes place window at correct screen edge

---

## 27.3 Desktop Notifications

Surface terminal notifications to the OS.

**File:** `oriterm/src/term_handler.rs` (OSC parsing), `oriterm/src/notifications.rs` (platform dispatch)

**Reference:** `_old/src/term_handler/mod.rs` (OSC handling), `_old/src/tab/interceptor.rs` (raw interceptor)

- [ ] Parse notification sequences:
  - [ ] OSC 9 (iTerm2): `OSC 9 ; message ST`
  - [ ] OSC 777 (urxvt): `OSC 777 ; notify ; title ; body ST`
  - [ ] OSC 99 (kitty): `OSC 99 ; ... ST`
  - [ ] Add handlers in `term_handler.rs` or `RawInterceptor`
- [ ] Platform notification dispatch:
  - [ ] Windows: `ToastNotification` via `windows` crate
    - [ ] Or simpler: system tray balloon via `Shell_NotifyIconW`
  - [ ] Linux: `notify-send` subprocess or D-Bus `org.freedesktop.Notifications`
  - [ ] macOS: `NSUserNotificationCenter` or `osascript -e 'display notification'`
  - [ ] Consider `notify-rust` crate for cross-platform abstraction
- [ ] Click notification to focus the originating terminal tab/window
- [ ] Config: `behavior.notifications = true | false` (default: true)
- [ ] Rate limit: max 5 notifications per second per tab (prevent spam)
- [ ] Bell notification: when BEL (0x07) received, optionally trigger OS notification
  - [ ] Config: `behavior.bell = "none" | "visual" | "notification"` (default: "visual")

**Tests:**
- [ ] OSC 9 message parsed correctly
- [ ] OSC 777 title and body parsed correctly
- [ ] Rate limiter blocks 6th notification within 1 second
- [ ] Config false disables all notifications
- [ ] BEL triggers notification when configured

---

## 27.4 Progress Indicators

Show task progress in tab bar or window title.

**File:** `oriterm/src/term_handler.rs` (OSC parsing), `oriterm/src/tab_bar.rs` (progress rendering), `oriterm/src/platform_windows.rs` (taskbar)

**Reference:** `_old/src/term_handler/mod.rs` (OSC handling), `_old/src/gpu/render_tab_bar.rs` (tab bar rendering)

- [ ] Parse ConEmu-style progress sequences:
  - [ ] `OSC 9;4;1;N ST` -- set progress to N% (0-100)
  - [ ] `OSC 9;4;2 ST` -- error state (red)
  - [ ] `OSC 9;4;3 ST` -- indeterminate (pulsing)
  - [ ] `OSC 9;4;0 ST` -- clear progress
  - [ ] Add to `RawInterceptor` or `term_handler.rs`
- [ ] Store progress state per tab: `progress: Option<Progress>`
  ```rust
  enum Progress {
      Percent(u8),
      Error,
      Indeterminate,
  }
  ```
- [ ] Display in tab bar:
  - [ ] Thin progress bar (2px) at bottom of tab label
  - [ ] Green for normal, red for error, pulsing for indeterminate
  - [ ] Or: colored fill overlay on tab background
- [ ] Windows taskbar progress:
  - [ ] `ITaskbarList3::SetProgressValue` via COM
  - [ ] Shows progress on the taskbar icon itself
  - [ ] Map `Progress::Percent` to taskbar value
  - [ ] Map `Progress::Error` to `TBPF_ERROR` state
- [ ] Config: `behavior.show_progress = true | false` (default: true)

**Tests:**
- [ ] OSC 9;4;1;50 sets progress to 50%
- [ ] OSC 9;4;2 sets error state
- [ ] OSC 9;4;0 clears progress
- [ ] Progress bar renders at correct width in tab label
- [ ] Taskbar progress updates on Windows

---

## 27.5 Terminal Inspector

Real-time debugging overlay for terminal developers. Only Ghostty has this -- it makes ori_term the go-to for terminal app development and escape sequence debugging.

**File:** `oriterm/src/inspector.rs` (state + logic), `oriterm/src/gpu/render_overlay.rs` (rendering)

**Reference:** `_old/src/gpu/render_overlay.rs` (overlay rendering), `_old/src/gpu/renderer.rs` (frame metrics)

- [ ] Toggle with `Ctrl+Shift+I` (like browser dev tools)
- [ ] Implementation: side panel (right side, configurable width ~40 columns) or bottom panel that shares the window with the terminal grid
- [ ] Inspector tabs/sections:
  - [ ] **Input:** Last N keystrokes with their encoded escape sequences
    - [ ] Log key name, modifiers, and raw bytes sent to PTY
    - [ ] Example: `Ctrl+C -> \x03`, `Up -> \x1b[A`
  - [ ] **Output:** Raw escape sequences received from PTY
    - [ ] Scrollable log with color-coded sequence types
    - [ ] CSI in blue, OSC in green, SGR in yellow, text in white
    - [ ] Show both raw bytes and parsed description
  - [ ] **State:** Current terminal state snapshot
    - [ ] Cursor position (row, col), cursor shape, cursor visible
    - [ ] Grid dimensions (cols x lines), scrollback size
    - [ ] Active terminal modes (as TermMode bitflags, human-readable)
    - [ ] Active charset, origin mode, insert mode
    - [ ] Mouse reporting mode, bracketed paste mode
  - [ ] **Timing:** Performance metrics
    - [ ] Frame render time (ms), current FPS
    - [ ] PTY read throughput (bytes/sec)
    - [ ] Instance count (bg + fg quads)
    - [ ] Atlas utilization (% of 1024x1024 used)
- [ ] Controls:
  - [ ] Pause button: freeze output log for inspection
  - [ ] Clear button: clear log
  - [ ] Copy button: copy visible log to clipboard
  - [ ] Filter: show only CSI, only OSC, only SGR, only text
- [ ] Rendering: use the same GPU text rendering as the terminal grid
  - [ ] Smaller font size (UI_FONT_SCALE)
  - [ ] Semi-transparent background panel

**Tests:**
- [ ] Ctrl+Shift+I toggles inspector panel
- [ ] Input tab logs keystrokes with correct escape encoding
- [ ] Output tab color-codes CSI, OSC, SGR sequences
- [ ] State tab reflects current cursor position and terminal modes
- [ ] Timing tab shows non-zero FPS and render time
- [ ] Pause freezes output log while terminal continues

---

## 27.6 Section Completion

- [ ] All 27.1-27.5 items complete
- [ ] Command palette opens with Ctrl+Shift+P
- [ ] Fuzzy search filters actions correctly
- [ ] Quick terminal toggles with global hotkey
- [ ] Quick terminal slides in/out from configured position
- [ ] Desktop notifications display for OSC 9/777
- [ ] Progress bars show in tab labels for OSC 9;4
- [ ] Terminal inspector shows input/output/state/timing
- [ ] All features can be disabled via config

**Exit Criteria:** Power users can discover actions via palette, summon a terminal instantly with a hotkey, and debug terminal sequences with the inspector.
