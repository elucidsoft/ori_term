---
section: "17"
title: Shell Integration
status: in-progress
goal: Auto-inject shell scripts that enable prompt navigation, smart close, CWD inheritance, and command-aware features
sections:
  - id: "17.1"
    title: Shell Script Injection
    status: not-started
  - id: "17.2"
    title: Prompt Navigation
    status: not-started
  - id: "17.3"
    title: Smart Features
    status: not-started
  - id: "17.4"
    title: SSH Integration
    status: not-started
  - id: "17.5"
    title: Completion Checklist
    status: in-progress
---

# Section 17: Shell Integration

**Status:** In Progress (OSC 133 + OSC 7 parsing complete, no UI or scripts yet)
**Goal:** Provide shell integration scripts for bash, zsh, fish, and PowerShell
that mark prompt boundaries (OSC 133), report CWD (OSC 7), and enable
terminal-aware features like prompt navigation and smart close.

**Why this matters:** Shell integration is what makes Ghostty feel intelligent.
It enables prompt jumping, "don't confirm close when at idle prompt," CWD
inheritance for new tabs/splits, and command output selection. Without it,
these features are impossible.

**Inspired by:**
- Ghostty: auto-inject for bash/zsh/fish/elvish via `GHOSTTY_RESOURCES_DIR`
- Kitty: shell integration with `KITTY_SHELL_INTEGRATION` env var
- WezTerm: OSC 133 prompt marking, OSC 7 CWD reporting
- VS Code terminal: shell integration for prompt detection

**Current state:** ori_term already handles the terminal side:
- **OSC 133** (prompt markers): `RawInterceptor` in `src/tab.rs:73-115` parses
  A/B/C/D markers and updates `Tab.prompt_state: PromptState` enum
  (`PromptStart`, `CommandStart`, `OutputStart`, `None`)
- **OSC 7** (CWD): `RawInterceptor` parses `file://hostname/path` URIs and
  stores the path in `Tab.cwd: Option<String>` (`src/tab.rs:79-98`)
- **XTVERSION**: Responds with build number (`src/tab.rs:124-134`)
- **No shell scripts** emit these sequences — integration only works with shells
  that already emit them (e.g., fish, some zsh configs)
- **No UI** exposes the parsed state (no prompt navigation, no CWD in tab title)

---

## 17.1 Shell Script Injection

Bundle and auto-source shell integration scripts.

**Design:** Ship integration scripts alongside the binary. On startup, set
environment variables that tell the script where to find itself. The scripts
hook into shell prompt/preexec mechanisms to emit OSC 7 and OSC 133.

- [ ] Create integration scripts:
  - [ ] `shell-integration/bash.sh`:
    ```bash
    # Wraps PROMPT_COMMAND to emit OSC 133 markers
    __oriterm_precmd() {
        local exit_code=$?
        printf '\e]133;D;%d\a' "$exit_code"  # output end
        printf '\e]7;file://%s%s\a' "$HOSTNAME" "$PWD"  # CWD
        printf '\e]133;A\a'  # prompt start
    }
    __oriterm_preexec() {
        printf '\e]133;C\a'  # command output start
    }
    # Hook into PROMPT_COMMAND and DEBUG trap
    ```
  - [ ] `shell-integration/zsh.sh`:
    - [ ] Use `precmd` and `preexec` hooks via `add-zsh-hook`
    - [ ] Same OSC 133 A/B/C/D + OSC 7 pattern
  - [ ] `shell-integration/fish.fish`:
    - [ ] Override `fish_prompt` wrapper and `fish_preexec`
    - [ ] Fish natively supports some OSC sequences
  - [ ] `shell-integration/pwsh.ps1`:
    - [ ] Override `prompt` function
    - [ ] Use `$PSStyle.OutputRendering` and write escape sequences
- [ ] OSC 133 sequences emitted by scripts:
  - [x] `OSC 133;A ST` — prompt start (already parsed in `tab.rs`)
  - [x] `OSC 133;B ST` — prompt end / command start (already parsed)
  - [x] `OSC 133;C ST` — command output start (already parsed)
  - [x] `OSC 133;D;exitcode ST` — output end with exit code (already parsed)
- [ ] OSC 7 CWD reporting: `OSC 7;file://hostname/path ST`
  - [x] Already parsed and stored in `Tab.cwd` (`src/tab.rs:79-98`)
- [ ] Injection method:
  - [ ] Set `ORITERM=1` env var (general terminal detection)
  - [ ] Set `ORITERM_RESOURCES_DIR` pointing to script directory
  - [ ] Set `ORITERM_SHELL_INTEGRATION=1` to signal scripts are available
  - [ ] For bash: set `ENV` or `BASH_ENV` to source the script
  - [ ] For zsh: use `ZDOTDIR` wrapper technique (like Ghostty):
    create a temp `.zshenv` that sources our script then sources user's
  - [ ] For fish: add to `$__fish_config_dir/conf.d/` or use `XDG_DATA_DIRS`
  - [ ] For PowerShell: inject via `-Command` prefix in shell spawn
- [ ] Embed scripts in binary:
  - [ ] `include_str!("../shell-integration/bash.sh")` etc.
  - [ ] Write to temp directory on startup, set env vars to point there
  - [ ] Or: write to `config_dir/shell-integration/` on first run
- [ ] Config option: `behavior.shell_integration = true | false` (default: true)
- [ ] Scripts must be idempotent (safe to source twice)
- [ ] Scripts must not break if `ORITERM_SHELL_INTEGRATION` is not set

---

## 17.2 Prompt Navigation

Jump between command prompts in scrollback.

**Prerequisite:** OSC 133 markers must be emitted by shell (17.1) and parsed
(already done in `tab.rs`).

- [ ] Track prompt positions:
  - [ ] When OSC 133;A received, record absolute row as prompt start
  - [ ] Store in `Vec<usize>` (`prompt_rows`) per pane/tab, sorted ascending
  - [ ] Invalidate/adjust on: resize with reflow, scrollback truncation
  - [ ] On reflow: prompt rows need to be remapped to new absolute positions
- [ ] Jump to prompt:
  - [ ] `Ctrl+Shift+Up` — jump to previous prompt:
    - [ ] Binary search `prompt_rows` for largest row < current viewport top
    - [ ] Set `display_offset` so target prompt is near top of viewport
  - [ ] `Ctrl+Shift+Down` — jump to next prompt:
    - [ ] Find smallest row > current viewport top
    - [ ] If at bottom: scroll to live position (display_offset = 0)
  - [ ] Add keybinding actions: `Action::PreviousPrompt`, `Action::NextPrompt`
  - [ ] Add default keybindings to `keybindings.rs`
- [ ] Visual feedback:
  - [ ] Brief highlight flash on target prompt row (yellow tint, 200ms fade)
  - [ ] Or: render a left-margin marker (like VS Code's gutter dots)
- [ ] Command output selection:
  - [ ] `Ctrl+Shift+Click` on a command — select the entire command output
    (from OSC 133;C row to next OSC 133;A row)
  - [ ] Requires mapping click position to prompt region boundaries

---

## 17.3 Smart Features

Features enabled by shell integration.

- [ ] CWD in tab title:
  - [ ] When `Tab.cwd` is set (from OSC 7), show directory name in tab title
  - [ ] Format: `~/projects/ori_term` (truncate home to `~`)
  - [ ] If shell sets OSC 0/1/2 title, prefer that; use CWD as fallback
- [ ] CWD inheritance:
  - [ ] New tab/split starts in the CWD of the focused pane (from `Tab.cwd`)
  - [ ] Pass CWD as working directory in `CommandBuilder::cwd()` when spawning PTY
  - [ ] Fallback: if no CWD known, use home directory
- [ ] Smart close confirmation:
  - [ ] If cursor is at a prompt (`PromptState::PromptStart`), skip "are you sure?"
  - [ ] If a command is running (`PromptState::OutputStart`), warn before close
  - [ ] Requires Section 22.3 (close confirmation dialog) or similar UI
- [ ] Last command exit code:
  - [ ] Parse exit code from OSC 133;D (already available in raw parser)
  - [ ] Store `last_exit_code: Option<i32>` per pane
  - [ ] Optionally show in tab bar: small colored dot (green = 0, red = nonzero)
- [ ] Alt+click cursor positioning at prompt:
  - [ ] When at prompt (`PromptState::PromptStart`), Alt+click moves cursor
  - [ ] Calculate column delta between current cursor and click position
  - [ ] Emit appropriate arrow key sequences to move the readline cursor

---

## 17.4 SSH Integration

Maintain terminal features across SSH sessions.

- [ ] Detect SSH sessions:
  - [ ] Check if `SSH_CONNECTION` or `SSH_TTY` env vars are set in child
  - [ ] Or: detect `ssh` command in shell preexec hook
- [ ] Auto-install terminfo on remote host:
  - [ ] `infocmp -x oriterm | ssh host tic -x -` pattern
  - [ ] Only when opt-in: `behavior.ssh_terminfo = true` (default: false)
- [ ] Set `TERM=xterm-256color` fallback when remote lacks our terminfo
- [ ] Forward `COLORTERM=truecolor` and `ORITERM=1` in SSH environment
- [ ] Config option: `behavior.ssh_integration = true | false` (default: false, opt-in)

---

## 17.5 Completion Checklist

- [x] OSC 133 prompt markers parsed and stored (PromptState enum)
- [x] OSC 7 CWD parsed and stored (Tab.cwd)
- [ ] Shell integration scripts created for bash, zsh, fish, PowerShell
- [ ] Scripts auto-injected via environment variables
- [ ] Prompt navigation (Ctrl+Shift+Up/Down) works in scrollback
- [ ] New tabs/splits inherit CWD from focused pane
- [ ] CWD shown in tab title (when available)
- [ ] Smart close: skip confirm at idle prompt, warn during running command
- [ ] Integration can be disabled via config
- [ ] SSH terminfo auto-install works (opt-in)

**Exit Criteria:** Opening a new ori_term session with bash/zsh/fish/PowerShell
automatically enables prompt marking, CWD reporting, and prompt navigation
without user configuration.
