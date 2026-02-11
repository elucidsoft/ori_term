---
section: "13"
title: Configuration
status: complete
goal: User-configurable terminal settings with config file, hot reload, and sensible defaults
sections:
  - id: "13.1"
    title: Config File Format
    status: complete
  - id: "13.2"
    title: Settings
    status: complete
  - id: "13.3"
    title: Hot Reload
    status: complete
  - id: "13.4"
    title: Key Bindings
    status: complete
  - id: "13.5"
    title: Completion Checklist
    status: complete
---

# Section 13: Configuration

**Status:** Complete (13.1-13.4 all complete)
**Goal:** Comprehensive configuration system with a config file, hot reload,
and all commonly-configured terminal settings.

**Inspired by:**
- Alacritty's TOML config with hot reload
- Ghostty's config with key-value pairs and repeatability
- WezTerm's Lua-based configuration

**Current state:** TOML config file loads from platform-specific path. Font size,
shell command, scrollback size, color scheme, initial window dimensions, opacity
(grid + tab bar independently), blur, and key bindings are all configurable.
Settings window provides runtime theme switching with persistence. Hot reload
watches config file via `notify` crate and applies changes live. Key bindings
configurable via `[[keybind]]` TOML sections with hot reload. Per-color overrides
(foreground, background, cursor, selection, ANSI 0-15) apply on top of any scheme.

**Implementation:** `src/config.rs` (`Config` struct with serde Serialize/Deserialize),
`src/config_monitor.rs` (`ConfigMonitor` file watcher with debounce).
`app.rs` loads config at startup, passes settings to tab spawning and rendering,
and handles `TermEvent::ConfigReload` for live updates.

---

## 13.1 Config File Format

Define config file location and format.

- [x] Config file location:
  - [x] Windows: `%APPDATA%\ori_term\config.toml`
  - [x] Linux: `$XDG_CONFIG_HOME/ori_term/config.toml` (fallback `~/.config/ori_term/`)
  - [ ] macOS: `~/Library/Application Support/ori_term/config.toml` — untested
  - [ ] Override with `ORI_TERM_CONFIG` env var — not implemented
- [x] Format: TOML via `toml` crate
- [x] Add `toml` and `serde` (with derive) dependencies
- [x] Define `Config` struct with `#[derive(Deserialize, Serialize)]`
- [x] All fields optional with sensible defaults (via `#[serde(default)]`)
- [x] Error handling: invalid config logs error and falls back to defaults
- [x] `Config::load()` reads from default path, returns defaults on missing/invalid
- [x] `Config::save()` writes TOML to default path, creates directory if needed
- [x] Generate default config: `oriterm --print-config`

**Tests:**
- [x] Default config roundtrip (serialize → deserialize)
- [x] Partial TOML uses defaults for missing fields
- [x] Empty TOML gives full defaults
- [x] Config dir is not empty
- [x] Config path ends with `.toml`

**Files:** `src/config.rs` (`Config`, `config_dir()`, `config_path()`)

**Ref:** Alacritty TOML config, Ghostty config format

---

## 13.2 Settings

Configurable settings.

- [x] Font settings:
  - [x] `font.size` — font size in pixels (default: 16.0)
  - [x] `font.family` — optional font family name override
  - [ ] `font.bold_family` — override bold font — not implemented
  - [ ] `font.italic_family` — override italic font — not implemented
- [x] Color settings:
  - [x] `colors.scheme` — named color scheme (default: "Catppuccin Mocha")
  - [x] Runtime theme switching via settings dropdown (persists to config)
  - [x] `colors.foreground` / `colors.background` — override default fg/bg colors
  - [x] `colors.cursor` — cursor color override
  - [x] `colors.selection_foreground` / `colors.selection_background` — selection highlight colors (default: fg/bg swap)
  - [x] `[colors.ansi]` / `[colors.bright]` — sparse override of 16 ANSI colors by index
- [x] Window settings:
  - [x] `window.columns` — initial columns (default: 120)
  - [x] `window.rows` — initial rows (default: 30)
  - [x] `window.opacity` — background/grid opacity (0.0-1.0, default 1.0)
  - [x] `window.tab_bar_opacity` — tab bar opacity (Option<f32>, falls back to opacity)
  - [x] `window.blur` — compositor blur behind transparent areas (default: true)
  - [ ] `window.padding` — inner padding in pixels — not implemented
  - [ ] `window.decorations` — "full" / "none" / "transparent" — not implemented
- [x] Terminal settings:
  - [x] `terminal.shell` — shell command (default: auto-detect cmd.exe on Windows)
  - [x] `terminal.scrollback` — max scrollback lines (default: 10000)
  - [x] `terminal.cursor_style` — "block" / "bar" / "underline" (default: "block")
  - [ ] `terminal.cursor_blink` — true/false — not implemented (cursor blinking not yet supported)
- [x] Behavior settings:
  - [x] `behavior.copy_on_select` — auto-copy to clipboard on mouse release (default: true)
  - [ ] `behavior.confirm_close` — not implemented
  - [x] `behavior.bold_is_bright` — promote ANSI 0-7 to 8-15 when bold (default: true)
  - [ ] `behavior.ambiguous_width` — not implemented

**Files:** `src/config.rs` (`FontConfig`, `TerminalConfig`, `ColorConfig`, `WindowConfig`, `BehaviorConfig`)

**Ref:** Alacritty config options, Ghostty config reference

---

## 13.3 Hot Reload

Reload configuration without restarting. **Complete.**

- [x] Watch config file for changes (using `notify` crate — `ReadDirectoryChangesW` on Windows, `inotify` on Linux)
- [x] On change: re-parse config, apply delta (200ms debounce)
- [x] `Config::try_load()` returns `Result` so reload preserves previous config on errors
- [x] Hot-reloadable settings (apply immediately):
  - [x] Colors / color scheme / color overrides — scheme via `apply_scheme_to_all_tabs()`, overrides via `apply_overrides()`
  - [x] Font family and size — full `FontSet::load()` + atlas rebuild + tab resize
  - [x] Cursor style — updates `tab.cursor_shape` on all tabs
  - [x] `behavior.bold_is_bright` — updates all tab palettes
  - [x] `behavior.copy_on_select` — read from `self.config` at use site, auto-updates
  - [x] Window opacity — reads `effective_opacity()` each frame, auto-applies on reload
  - [x] Tab bar opacity — reads `effective_tab_bar_opacity()` each frame
  - [x] Key bindings — re-merged on config reload
  - [ ] Padding — not implemented (padding setting itself not implemented)
- [x] Cold settings (only affect new tabs/windows):
  - [x] Shell command
  - [x] Scrollback size
  - [x] Window columns/rows
- [x] Error handling: parse errors logged, previous config kept
- [x] Manual reload: Ctrl+Shift+R
- [x] `ConfigMonitor` shuts down cleanly on app exit
- [ ] Show notification on reload: "Config reloaded" or "Config error: ..." — logged to debug log, no in-app notification yet

**Edge cases handled:**
- Rapid saves collapsed by 200ms debounce
- Parse errors: logged, previous config preserved
- File deleted: `try_load` returns Err, previous config kept
- Atomic saves (vim rename): parent dir watch catches rename events
- Settings dropdown theme change: triggers watcher but `apply_config_reload` is idempotent

**Files:** `src/config_monitor.rs` (`ConfigMonitor`), `src/config.rs` (`Config::try_load()`),
`src/tab.rs` (`TermEvent::ConfigReload`), `src/app.rs` (event handler, `apply_config_reload()`, Ctrl+Shift+R)

**Ref:** Alacritty live config reload, Ghostty config reload

---

## 13.4 Key Bindings

User-configurable keyboard shortcuts. **Complete.**

- [x] Define default key bindings (20 bindings):
  - [x] Ctrl+Shift+C — copy, Ctrl+Shift+V — paste
  - [x] Ctrl+Insert — copy, Shift+Insert — paste
  - [x] Ctrl+Shift+F — search, Ctrl+Shift+R — reload config
  - [x] Ctrl+T — new tab, Ctrl+W — close tab
  - [x] Ctrl+Tab / Ctrl+Shift+Tab — next/prev tab
  - [x] Ctrl+= / Ctrl+- / Ctrl+0 — zoom in/out/reset
  - [x] Shift+PageUp/PageDown/Home/End — scrollback navigation
  - [x] Ctrl+C — smart copy (selection or ^C), Ctrl+V — smart paste
- [x] Config format:
  ```toml
  [[keybind]]
  key = "c"
  mods = "Ctrl|Shift"
  action = "Copy"
  ```
- [x] Actions: enum of all bindable actions (`Action` in `keybindings.rs`)
- [x] Allow unbinding defaults: `action = "None"`
- [x] Allow binding to send custom escape sequences: `action = "SendText:\x1b[A"`
- [x] Key binding resolution: user bindings override defaults by (key+mods)
- [x] Hot reload: Ctrl+Shift+R and config file watcher re-merge bindings
- [x] 12 unit tests covering defaults, merge, parse, normalization

**Ref:** Alacritty key bindings, Ghostty keybind config

---

## 13.5 Completion Checklist

- [x] Config file loads from platform-appropriate location
- [x] All settings have sensible defaults (works with no config file)
- [x] Font size configurable
- [x] Font family configurable (loaded at startup via `FontSet::load`)
- [x] Color scheme configurable (config + runtime dropdown)
- [x] Window opacity configurable (`window.opacity`, premultiplied alpha blending)
- [x] Tab bar opacity configurable (`window.tab_bar_opacity`, independent from grid)
- [x] Blur configurable (`window.blur`, compositor blur behind transparent areas)
- [x] Scrollback size configurable
- [x] Shell command configurable
- [x] Cursor style configurable ("block" / "bar" / "underline")
- [x] Copy-on-select configurable (default: true)
- [x] Bold-is-bright configurable (default: true)
- [x] Key bindings configurable (keybindings.rs, TOML `[[keybind]]`, hot reload)
- [x] Color overrides (fg/bg/cursor/selection/ANSI) apply on top of scheme
- [x] Hot reload works for colors, color overrides, font, cursor style, bold_is_bright
- [x] Invalid config shows error (logs), falls back to defaults
- [x] `--print-config` generates default config (also `--help`, `--version`)

**Exit Criteria:** Users can customize the terminal via config file. Hot reload
works for visual settings. All commonly-needed settings are configurable.
