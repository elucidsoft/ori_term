---
section: "20"
title: Theme System
status: in-progress
goal: Ship hundreds of built-in themes with light/dark auto-switching
sections:
  - id: "20.1"
    title: Theme Format & Loading
    status: in-progress
  - id: "20.2"
    title: Built-in Theme Library
    status: in-progress
  - id: "20.3"
    title: Light/Dark Auto-Switch
    status: not-started
  - id: "20.4"
    title: Completion Checklist
    status: in-progress
---

# Section 20: Theme System

**Status:** In Progress (8 hardcoded themes + runtime dropdown selector working)
**Goal:** Ship 100+ built-in themes selectable by name, with automatic light/dark
mode switching based on system preference.

**Why this matters:** Ghostty ships 300+ themes. Having 8 hardcoded schemes makes
ori_term feel unfinished. Theme richness is a strong first impression signal —
users want to personalize their terminal immediately.

**Inspired by:**
- Ghostty: 300+ built-in themes, single-line config, light/dark auto-switch
- iTerm2: hundreds of importable color schemes
- base16: standardized 16-color scheme format used across editors/terminals
- Kitty: theme kitten with preview and selection

**Current state:** 8 hardcoded color schemes in `src/palette.rs`:
1. WezTerm Default
2. Catppuccin Mocha (default)
3. Catppuccin Latte
4. One Dark
5. Solarized Dark
6. Solarized Light
7. Dracula
8. Tokyo Night

`BUILTIN_SCHEMES: &[&ColorScheme]` array (`palette.rs:218`). `find_scheme(name)`
looks up by name. `Palette::set_scheme()` applies a scheme's 16 ANSI colors +
semantic colors. Runtime switching via settings dropdown in `app.rs` — click a
theme name to apply it. Selection persisted to `colors.scheme` in config file.
Tab bar colors derived dynamically from palette background.

---

## 20.1 Theme Format & Loading

Define a theme format and support loading from files.

**Current `ColorScheme` struct** (`src/palette.rs:9`):
```rust
pub struct ColorScheme {
    pub name: &'static str,
    pub ansi: [[u8; 3]; 16],    // 16 ANSI colors as RGB
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
}
```

- [x] Hardcoded `ColorScheme` constants (8 schemes)
- [x] `BUILTIN_SCHEMES` array with all schemes
- [x] `find_scheme(name)` lookup by name
- [x] Config: `colors.scheme = "Catppuccin Mocha"` (by name)
- [ ] TOML theme file format:
  ```toml
  name = "Nord"

  [colors]
  foreground = "#D8DEE9"
  background = "#2E3440"
  cursor = "#D8DEE9"

  [colors.ansi]
  black = "#3B4252"
  red = "#BF616A"
  green = "#A3BE8C"
  yellow = "#EBCB8B"
  blue = "#81A1C1"
  magenta = "#B48EAD"
  cyan = "#88C0D0"
  white = "#E5E9F0"
  bright_black = "#4C566A"
  bright_red = "#BF616A"
  bright_green = "#A3BE8C"
  bright_yellow = "#EBCB8B"
  bright_blue = "#81A1C1"
  bright_magenta = "#B48EAD"
  bright_cyan = "#8FBCBB"
  bright_white = "#ECEFF4"

  # Optional extended colors
  [colors.extended]
  selection_fg = "#D8DEE9"
  selection_bg = "#434C5E"
  ```
- [ ] `ThemeFile` struct with `Deserialize`:
  ```rust
  #[derive(Deserialize)]
  struct ThemeFile {
      name: String,
      colors: ThemeColors,
  }
  #[derive(Deserialize)]
  struct ThemeColors {
      foreground: String,  // "#RRGGBB"
      background: String,
      cursor: String,
      ansi: ThemeAnsi,
      extended: Option<ThemeExtended>,
  }
  ```
- [ ] Parse hex color strings (`#RRGGBB`) to `[u8; 3]`
- [ ] Load themes from:
  - [ ] Embedded in binary (current hardcoded schemes)
  - [ ] User theme directory: `config_dir/themes/*.toml`
  - [ ] Config: `colors.scheme = "nord"` (by name, case-insensitive)
  - [ ] Config: `colors.scheme = "/path/to/mytheme.toml"` (by absolute path)
- [ ] Theme discovery at startup:
  - [ ] Scan `config_dir/themes/` for `*.toml` files
  - [ ] Parse each, build `Vec<ColorScheme>` of user themes
  - [ ] Merge with built-in schemes (user themes can override built-in names)
- [ ] Theme hot-reload:
  - [ ] ConfigMonitor already watches config dir
  - [ ] Extend to watch `themes/` subdirectory
  - [ ] On theme file change: re-parse and apply if it's the active theme

---

## 20.2 Built-in Theme Library

Port popular color schemes as embedded themes.

**Current built-in schemes (8):**
- [x] Catppuccin Mocha, Catppuccin Latte
- [x] One Dark
- [x] Solarized Dark, Solarized Light
- [x] Dracula
- [x] Tokyo Night
- [x] WezTerm Default

**Additional schemes to add (target: 50+ built-in):**
- [ ] Catppuccin Frappe, Catppuccin Macchiato (complete the Catppuccin family)
- [ ] Tokyo Night Storm, Tokyo Night Light
- [ ] One Light
- [ ] Gruvbox Dark, Gruvbox Light, Gruvbox Material Dark
- [ ] Nord
- [ ] Rose Pine, Rose Pine Moon, Rose Pine Dawn
- [ ] Everforest Dark, Everforest Light
- [ ] Kanagawa, Kanagawa Wave, Kanagawa Dragon
- [ ] Ayu Dark, Ayu Light, Ayu Mirage
- [ ] Material Dark, Material Lighter, Material Palenight
- [ ] Monokai Pro, Monokai Classic
- [ ] Nightfox, Dawnfox, Carbonfox, Nordfox
- [ ] Zenburn
- [ ] GitHub Dark, GitHub Light
- [ ] Horizon Dark
- [ ] Poimandres
- [ ] Andromeda
- [ ] Moonlight II
- [ ] Synthwave '84
- [ ] Base16 Default Dark/Light

**Conversion tools:**
- [ ] Script to convert iTerm2 `.itermcolors` XML to our TOML format
- [ ] Script to convert Ghostty theme format (key=value) to our TOML format
- [ ] Script to convert base16 YAML to our TOML format
- [ ] Source: https://github.com/mbadolato/iTerm2-Color-Schemes (200+ schemes)

**Implementation:** Add each scheme as a `const ColorScheme` in `palette.rs`
and include in `BUILTIN_SCHEMES`. Binary size impact: ~64 bytes per scheme
(16 * 3 + 3 * 3 + name), so 100 schemes = ~6.4 KB — negligible.

---

## 20.3 Light/Dark Auto-Switch

Automatically switch theme based on system appearance.

- [ ] Config syntax:
  ```toml
  [colors]
  scheme = "dark:Tokyo Night, light:Tokyo Night Light"
  # Or just: scheme = "Tokyo Night" (always that theme)
  ```
- [ ] Parse `scheme` value:
  - [ ] If contains `dark:` / `light:` prefixes: conditional theme
  - [ ] Otherwise: static theme (current behavior)
- [ ] Detect system dark/light mode:
  - [ ] Windows: read `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`
    - [ ] 0 = dark mode, 1 = light mode
    - [ ] Use `windows-sys` `RegGetValueW` or `winreg` crate
  - [ ] macOS: `NSAppearance` observation via `objc2` or check `defaults read -g AppleInterfaceStyle`
  - [ ] Linux: `org.freedesktop.appearance.color-scheme` D-Bus property
    - [ ] 1 = prefer dark, 2 = prefer light
    - [ ] Use `zbus` crate or `dconf read`
- [ ] On system theme change:
  - [ ] Swap palette to the appropriate theme
  - [ ] Apply to all tabs via `set_scheme()`
  - [ ] Request redraw for all windows
- [ ] Settings dropdown improvements:
  - [ ] Group themes by light/dark/universal
  - [ ] Show "(dark)" / "(light)" label next to theme names
  - [ ] Optionally: preview theme on hover before click-to-apply

---

## 20.4 Completion Checklist

- [x] 8 built-in themes available by name in config
- [x] Runtime theme switching via settings dropdown
- [x] Theme selection persisted to config file
- [ ] 50+ themes available by name in config
- [ ] Custom themes loadable from TOML files in theme directory
- [ ] Light/dark auto-switching works on Windows
- [ ] Settings dropdown lists all available themes (built-in + user)
- [ ] Theme hot-reload works (edit theme file, see change)
- [ ] User themes in theme directory discovered automatically
- [ ] Theme conversion scripts for iTerm2/Ghostty/base16 formats

**Exit Criteria:** User can type `colors.scheme = "nord"` in config and get the
Nord color scheme. System dark/light mode change auto-switches themes.
