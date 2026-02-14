---
section: 20
title: Shell Integration
status: not-started
tier: 4
goal: Shell detection, injection, OSC 7/133 handling, two-parser strategy, prompt state machine
sections:
  - id: "20.1"
    title: Shell Detection
    status: not-started
  - id: "20.2"
    title: Shell Injection Mechanisms
    status: not-started
  - id: "20.3"
    title: Integration Scripts
    status: not-started
  - id: "20.4"
    title: Version Stamping
    status: not-started
  - id: "20.5"
    title: Raw Interceptor
    status: not-started
  - id: "20.6"
    title: CWD Tracking
    status: not-started
  - id: "20.7"
    title: Tab Title Resolution
    status: not-started
  - id: "20.8"
    title: Prompt State Machine
    status: not-started
  - id: "20.9"
    title: Keyboard Mode Stack Swap
    status: not-started
  - id: "20.10"
    title: XTVERSION Response
    status: not-started
  - id: "20.11"
    title: Notification Handling
    status: not-started
  - id: "20.12"
    title: Section Completion
    status: not-started
---

# Section 20: Shell Integration

**Status:** 📋 Planned
**Goal:** Detect the user's shell and inject integration scripts that enable CWD tracking, prompt markers, and notifications. Five shell injection mechanisms, each with different approaches. WSL is a special case (launcher, not shell).

**Crate:** `oriterm` (binary only — no core changes)

**Reference:** `_old/src/shell_integration.rs`, `_old/shell-integration/`, `_old/src/tab/interceptor.rs`

---

## 20.1 Shell Detection

Detect the user's shell from the program path so the correct injection mechanism can be selected.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] `Shell` enum: `Bash`, `Zsh`, `Fish`, `PowerShell`, `Wsl`
- [ ] `detect_shell(program: &str) -> Option<Shell>` — match basename (ignoring `.exe`), handle full paths

---

## 20.2 Shell Injection Mechanisms

Each shell requires a different injection strategy. WSL is a special case — it's a launcher, not a shell.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] Injection mechanisms (per shell):
  | Shell | Method | How |
  |-------|--------|-----|
  | Bash | `--posix` + `ENV` var | Set `ENV=path/to/oriterm.bash`, shell sources it on startup |
  | Zsh | `ZDOTDIR` redirect | Set `ZDOTDIR` to our dir with `.zshenv` that sources integration then restores original `ZDOTDIR` |
  | Fish | `XDG_DATA_DIRS` prepend | Prepend our dir so Fish finds `vendor_conf.d/oriterm-shell-integration.fish` |
  | PowerShell | `ORITERM_PS_PROFILE` env var | User's `$PROFILE` can check and source integration script |
  | WSL | `WSLENV` propagation | Simple env vars only (no path injection across WSL boundary). Users manually source scripts from their `.bashrc`/`.zshrc` |

---

## 20.3 Integration Scripts

The shell integration scripts emit OSC sequences that the terminal intercepts for CWD tracking, prompt marking, and notifications.

**File:** `shell-integration/` directory

**Reference:** `_old/shell-integration/`

- [ ] Integration scripts emit:
  - [ ] `OSC 7 ; file://hostname/path ST` — current working directory
  - [ ] `OSC 133 ; A ST` — prompt start
  - [ ] `OSC 133 ; B ST` — command start (user typing)
  - [ ] `OSC 133 ; C ST` — output start (command executing)
  - [ ] `OSC 133 ; D ST` — command complete
  - [ ] `OSC 9` / `OSC 99` / `OSC 777` — notifications (iTerm2 / Kitty / rxvt-unicode)

---

## 20.4 Version Stamping

Prevent stale scripts from persisting after app updates by stamping a version file alongside the integration scripts.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] `shell-integration/.version` file contains app version string
- [ ] On launch: if `.version` matches `env!("CARGO_PKG_VERSION")`, skip writing scripts
- [ ] Otherwise: overwrite all shell integration scripts and update `.version`
- [ ] Prevents stale scripts from persisting after app updates

---

## 20.5 Raw Interceptor

The high-level VTE processor drops sequences it doesn't recognize. A two-parser strategy catches OSC 7, OSC 133, and other custom sequences before they are lost.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/tab/interceptor.rs`

- [ ] The high-level VTE processor (`vte::ansi::Processor`) drops sequences it doesn't recognize (OSC 7, OSC 133, etc.)
- [ ] Solution: a raw `vte::Parser` with custom `Perform` trait impl runs on the **same bytes** before the high-level processor
- [ ] Raw interceptor catches: OSC 7 (CWD), OSC 133 (prompts), OSC 9/99/777 (notifications), CSI >q (XTVERSION response)
- [ ] Both parsers run within the same terminal lock
- [ ] Interceptor writes to mutable refs on TerminalState fields (no separate struct needed in rebuild — `Term<T>` handles these directly in the VTE handler)

---

## 20.6 CWD Tracking

Track the current working directory via OSC 7 so the tab bar can display it and new tabs can inherit it.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] When OSC 7 received: parse `file://hostname/path`, strip prefix, store in `Term.cwd`
- [ ] Mark `title_dirty = true` (CWD change may affect tab bar title)
- [ ] If no explicit title (OSC 0/2) was set: tab bar shows short path from CWD

---

## 20.7 Tab Title Resolution

Three sources for tab titles, with strict priority ordering.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] Tab title resolution — three sources with priority:
  1. [ ] Explicit title from OSC 0/2: `has_explicit_title = true`, show `title` field
  2. [ ] CWD short path: if `cwd.is_some()` and `!has_explicit_title`, show last component(s) of path
  3. [ ] Fallback: static title (e.g., "Tab N")
- [ ] `effective_title() -> &str` implements this priority
- [ ] When OSC 7 updates CWD: clears `has_explicit_title` so CWD-based title takes over

---

## 20.8 Prompt State Machine

Track prompt lifecycle via OSC 133 sub-parameters. Prompt marking is deferred because the cursor position is updated by the high-level processor, not the raw interceptor.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] `PromptState` enum: `None`, `PromptStart`, `CommandStart`, `OutputStart`
- [ ] Transitions on OSC 133 sub-params (A → B → C → D → None)
- [ ] `prompt_mark_pending: bool` — when OSC 133;A arrives, set pending. Actual grid row marking happens **after both parsers finish** (deferred), because the cursor position is updated by the high-level processor, not the raw interceptor
- [ ] Prompt lines can be used for: smart selection (select full command), scroll-to-prompt navigation

---

## 20.9 Keyboard Mode Stack Swap

When switching between primary and alt screen, the keyboard mode stack must be swapped so alt-screen apps can use different key encodings without affecting the primary shell.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] `keyboard_mode_stack: Vec<KeyboardModes>` — active screen's stack
- [ ] `inactive_keyboard_mode_stack: Vec<KeyboardModes>` — stashed stack
- [ ] When switching primary ↔ alt screen (`swap_alt()`): swap the two stacks
- [ ] Allows alt-screen apps (vim, less) to use different key encodings without affecting the primary shell

---

## 20.10 XTVERSION Response

Respond to XTVERSION queries so that shell integration scripts and applications can detect the terminal emulator.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] On CSI >q: generate `DCS > | oriterm(version build N) ST`
- [ ] Append to VTE response buffer for reader thread to flush outside the terminal lock

---

## 20.11 Notification Handling

Collect notifications from OSC 9/99/777 sequences and forward them to the OS notification system.

**File:** `oriterm/src/shell_integration.rs`

**Reference:** `_old/src/shell_integration.rs`

- [ ] `pending_notifications: Vec<Notification>` — drained by main thread on each Wakeup
- [ ] `Notification { title: String, body: String }`
- [ ] OS notification dispatch (platform-specific, stretch goal)

---

## 20.12 Section Completion

- [ ] All 20.1–20.11 items complete
- [ ] Shell detection identifies all five shell types correctly
- [ ] Injection mechanisms set correct environment variables per shell
- [ ] Integration scripts emit proper OSC 7, OSC 133, and notification sequences
- [ ] Version stamping prevents stale scripts
- [ ] Two-parser strategy catches all custom sequences without dropping standard VTE output
- [ ] CWD tracking updates tab bar title correctly
- [ ] Tab title resolution follows 3-source priority (explicit → CWD → fallback)
- [ ] Prompt state machine transitions correctly through all OSC 133 sub-params with deferred marking
- [ ] Keyboard mode stack swaps correctly on primary ↔ alt screen transitions
- [ ] XTVERSION response is correct and flushed outside terminal lock
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — clean build
- [ ] `cargo clippy -p oriterm -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test -p oriterm_core` — all tests pass

**Exit Criteria:** Shell integration works for all five shell types. CWD tracking, prompt marking, and notifications function correctly. The two-parser strategy catches all custom OSC sequences. Title resolution follows the correct priority chain. Keyboard mode stacks swap cleanly on alt screen transitions.
