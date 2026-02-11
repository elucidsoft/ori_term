---
section: "17"
title: Shell Integration
status: not-started
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
    status: not-started
---

# Section 17: Shell Integration

**Status:** Not Started
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

**Current state:** ori_term already parses OSC 7 (CWD) and OSC 133 (prompt markers)
in the raw parser, but no shell scripts emit them. No bundled integration scripts.

---

## 17.1 Shell Script Injection

Bundle and auto-source shell integration scripts.

- [ ] Create integration scripts:
  - [ ] `shell-integration/bash.sh` — PROMPT_COMMAND hooks for OSC 133 + OSC 7
  - [ ] `shell-integration/zsh.sh` — precmd/preexec hooks
  - [ ] `shell-integration/fish.fish` — fish_prompt/fish_preexec functions
  - [ ] `shell-integration/pwsh.ps1` — PowerShell prompt function override
- [ ] OSC 133 sequences emitted by scripts:
  - [ ] `OSC 133;A ST` — prompt start
  - [ ] `OSC 133;B ST` — prompt end (command start)
  - [ ] `OSC 133;C ST` — command end (output start)
  - [ ] `OSC 133;D;exitcode ST` — output end with exit code
- [ ] OSC 7 CWD reporting: `OSC 7;file://hostname/path ST`
- [ ] Injection method:
  - [ ] Set `ORITERM_SHELL_INTEGRATION=1` env var
  - [ ] Set `ORITERM_RESOURCES_DIR` pointing to bundled scripts
  - [ ] For bash: prepend `source` to `BASH_ENV` or `ENV`
  - [ ] For zsh: use `zdotdir` technique or `.zshenv` sourcing
  - [ ] For fish: add to `fish_user_paths` or `conf.d`
  - [ ] For PowerShell: inject via `-Command` prefix
- [ ] Embed scripts in binary (include_bytes!) or ship alongside executable
- [ ] Config option: `shell_integration = true | false` (default: true)
- [ ] Scripts must be idempotent (safe to source twice)

---

## 17.2 Prompt Navigation

Jump between command prompts in scrollback.

- [ ] Track prompt positions:
  - [ ] When OSC 133;A received, record absolute row as prompt start
  - [ ] Store in `Vec<usize>` (prompt_rows) per tab, sorted
- [ ] Jump to prompt:
  - [ ] `Ctrl+Shift+Up` — jump to previous prompt
  - [ ] `Ctrl+Shift+Down` — jump to next prompt
  - [ ] Scroll viewport so the target prompt is near the top
  - [ ] If at bottom and pressing Down, scroll to live position
- [ ] Visual indicator: brief highlight flash on target prompt row
- [ ] Command output selection:
  - [ ] `Ctrl+Shift+Click` on a prompt — select the entire command output
    (from OSC 133;C to next OSC 133;A)

---

## 17.3 Smart Features

Features enabled by shell integration.

- [ ] Smart close confirmation:
  - [ ] If cursor is at a prompt (between OSC 133;A and 133;B), skip
    "are you sure?" confirmation when closing tab/pane
  - [ ] If a command is running (after 133;B, before 133;D), warn before close
- [ ] CWD inheritance:
  - [ ] New tab/split starts in the CWD of the focused pane (from OSC 7)
  - [ ] Pass CWD as `--working-directory` or `cd` command when spawning shell
- [ ] Cursor style at prompt:
  - [ ] When at prompt (133;A..133;B), switch cursor to bar/beam
  - [ ] When command running, revert to application-requested cursor
- [ ] Alt+click cursor positioning:
  - [ ] When at prompt, Alt+click moves cursor to clicked column
  - [ ] Emit appropriate arrow key sequences to move cursor
- [ ] Last command exit code:
  - [ ] Parse exit code from OSC 133;D
  - [ ] Optionally show in prompt or tab bar (colored dot: green/red)

---

## 17.4 SSH Integration

Maintain terminal features across SSH sessions.

- [ ] Detect SSH commands in PTY output or via shell preexec hook
- [ ] Auto-install terminfo on remote host:
  - [ ] `infocmp -x | ssh host tic -x -` pattern
  - [ ] Only when `shell-integration-features = ssh-terminfo` enabled
- [ ] Set `TERM=xterm-256color` fallback when remote lacks our terminfo
- [ ] Forward `COLORTERM=truecolor` in SSH environment
- [ ] Config option: `ssh_integration = true | false` (default: false, opt-in)

---

## 17.5 Completion Checklist

- [ ] Shell integration works for bash, zsh, fish, PowerShell
- [ ] OSC 133 prompt markers emitted correctly
- [ ] OSC 7 CWD reported after each command
- [ ] Prompt navigation (Ctrl+Shift+Up/Down) works in scrollback
- [ ] New tabs/splits inherit CWD from focused pane
- [ ] Smart close: no confirm at idle prompt, warn during running command
- [ ] Scripts are auto-injected and idempotent
- [ ] Integration can be disabled via config
- [ ] SSH terminfo auto-install works (opt-in)

**Exit Criteria:** Opening a new ori_term session with bash/zsh/fish/PowerShell
automatically enables prompt marking, CWD reporting, and prompt navigation
without user configuration.
