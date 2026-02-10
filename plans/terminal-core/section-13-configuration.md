---
section: "13"
title: Configuration
status: not-started
goal: User-configurable terminal settings with config file, hot reload, and sensible defaults
sections:
  - id: "13.1"
    title: Config File Format
    status: not-started
  - id: "13.2"
    title: Settings
    status: not-started
  - id: "13.3"
    title: Hot Reload
    status: not-started
  - id: "13.4"
    title: Key Bindings
    status: not-started
  - id: "13.5"
    title: Completion Checklist
    status: not-started
---

# Section 13: Configuration

**Status:** Not Started
**Goal:** Comprehensive configuration system with a config file, hot reload,
and all commonly-configured terminal settings.

**Inspired by:**
- Alacritty's TOML config with hot reload
- Ghostty's config with key-value pairs and repeatability
- WezTerm's Lua-based configuration

**Current state:** All settings hardcoded: font path, font size (16), colors
(Catppuccin Mocha), grid dimensions (computed from window), scrollback (10k).
No config file.

---

## 13.1 Config File Format

Define config file location and format.

- [ ] Config file location:
  - [ ] Windows: `%APPDATA%\ori_term\config.toml`
  - [ ] Linux: `~/.config/ori_term/config.toml`
  - [ ] macOS: `~/Library/Application Support/ori_term/config.toml`
  - [ ] Override with `ORI_TERM_CONFIG` env var
- [ ] Format: TOML (simple, widely understood, good Rust support)
- [ ] Add `toml` and `serde` dependencies
- [ ] Define `Config` struct with `#[derive(Deserialize)]`
- [ ] All fields optional with sensible defaults
- [ ] Error handling: invalid config shows error overlay, falls back to defaults
- [ ] Generate default config with comments: `ori_term --print-config`

**Ref:** Alacritty TOML config, Ghostty config format

---

## 13.2 Settings

Configurable settings.

- [ ] Font settings:
  - [ ] `font.family` — font family name (default: "Cascadia Mono")
  - [ ] `font.size` — font size in points (default: 16)
  - [ ] `font.bold_family` — override bold font
  - [ ] `font.italic_family` — override italic font
- [ ] Color settings:
  - [ ] `colors.scheme` — named color scheme (default: "catppuccin-mocha")
  - [ ] `colors.foreground` / `colors.background` — override default colors
  - [ ] `colors.cursor` — cursor color
  - [ ] `colors.selection` — selection highlight color
  - [ ] `colors.ansi` / `colors.bright` — override 16 ANSI colors
- [ ] Window settings:
  - [ ] `window.opacity` — background opacity (0.0-1.0)
  - [ ] `window.padding` — inner padding in pixels
  - [ ] `window.decorations` — "full" / "none" / "transparent"
  - [ ] `window.startup_size` — initial columns x rows
- [ ] Terminal settings:
  - [ ] `terminal.shell` — shell command (default: auto-detect)
  - [ ] `terminal.scrollback` — max scrollback lines (default: 10000)
  - [ ] `terminal.cursor_style` — "block" / "bar" / "underline"
  - [ ] `terminal.cursor_blink` — true/false
- [ ] Behavior settings:
  - [ ] `behavior.copy_on_select` — auto-copy on selection (default: false on Windows)
  - [ ] `behavior.confirm_close` — warn before closing with running processes
  - [ ] `behavior.bold_is_bright` — bold text uses bright colors (default: true)
  - [ ] `behavior.ambiguous_width` — 1 or 2 (default: 1)

**Ref:** Alacritty config options, Ghostty config reference

---

## 13.3 Hot Reload

Reload configuration without restarting.

- [ ] Watch config file for changes (using `notify` crate or polling)
- [ ] On change: re-parse config, apply delta
- [ ] Hot-reloadable settings (apply immediately):
  - [ ] Colors / color scheme
  - [ ] Font family and size
  - [ ] Window opacity
  - [ ] Padding
  - [ ] Key bindings
- [ ] Cold settings (require restart):
  - [ ] Shell command
  - [ ] Window decorations
- [ ] Show notification on reload: "Config reloaded" or "Config error: ..."
- [ ] Manual reload: Ctrl+Shift+R (configurable)

**Ref:** Alacritty live config reload, Ghostty config reload

---

## 13.4 Key Bindings

User-configurable keyboard shortcuts.

- [ ] Define default key bindings:
  - [ ] Ctrl+Shift+C — copy
  - [ ] Ctrl+Shift+V — paste
  - [ ] Ctrl+Shift+F — search
  - [ ] Ctrl+T — new tab
  - [ ] Ctrl+W — close tab
  - [ ] Ctrl+Tab / Ctrl+Shift+Tab — next/prev tab
  - [ ] Ctrl+= / Ctrl+- — font size adjust
  - [ ] Ctrl+Shift+R — reload config
- [ ] Config format:
  ```toml
  [[keybind]]
  key = "C"
  mods = "Ctrl|Shift"
  action = "Copy"
  ```
- [ ] Actions: enum of all bindable actions
- [ ] Allow unbinding defaults: `action = "None"`
- [ ] Allow binding to send custom escape sequences: `action = "SendText:\x1b[A"`
- [ ] Key binding resolution: user bindings override defaults

**Ref:** Alacritty key bindings, Ghostty keybind config

---

## 13.5 Completion Checklist

- [ ] Config file loads from platform-appropriate location
- [ ] All settings have sensible defaults (works with no config file)
- [ ] Font family/size configurable
- [ ] Color scheme configurable
- [ ] Window opacity configurable
- [ ] Scrollback size configurable
- [ ] Shell command configurable
- [ ] Key bindings configurable
- [ ] Hot reload works for colors, font, opacity
- [ ] Invalid config shows error, falls back to defaults
- [ ] `--print-config` generates default config

**Exit Criteria:** Users can customize the terminal via config file. Hot reload
works for visual settings. All commonly-needed settings are configurable.
