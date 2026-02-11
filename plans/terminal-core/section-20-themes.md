---
section: "20"
title: Theme System
status: not-started
goal: Ship hundreds of built-in themes with light/dark auto-switching
sections:
  - id: "20.1"
    title: Theme Format & Loading
    status: not-started
  - id: "20.2"
    title: Built-in Theme Library
    status: not-started
  - id: "20.3"
    title: Light/Dark Auto-Switch
    status: not-started
  - id: "20.4"
    title: Completion Checklist
    status: not-started
---

# Section 20: Theme System

**Status:** Not Started
**Goal:** Ship 100+ built-in themes selectable by name, with automatic light/dark
mode switching based on system preference.

**Why this matters:** Ghostty ships 300+ themes. Having 7 hardcoded schemes makes
ori_term feel unfinished. Theme richness is a strong first impression signal —
users want to personalize their terminal immediately.

**Inspired by:**
- Ghostty: 300+ built-in themes, single-line config, light/dark auto-switch
- iTerm2: hundreds of importable color schemes
- base16: standardized 16-color scheme format used across editors/terminals
- Kitty: theme kitten with preview and selection

**Current state:** 7 hardcoded color schemes (Catppuccin Mocha/Latte, One Dark,
Solarized Dark/Light, Gruvbox Dark, Tokyo Night) in `palette.rs`.

---

## 20.1 Theme Format & Loading

Define a theme format and load themes at startup.

- [ ] Theme file format: TOML with 16 ANSI colors + semantic colors
  ```toml
  [colors]
  foreground = "#c0caf5"
  background = "#1a1b26"
  cursor = "#c0caf5"
  selection_fg = "#c0caf5"
  selection_bg = "#33467c"

  [colors.ansi]
  black = "#15161e"
  red = "#f7768e"
  # ... 14 more
  ```
- [ ] Load themes from:
  - [ ] Embedded in binary (include_bytes for core themes)
  - [ ] User theme directory (`%APPDATA%/ori_term/themes/` on Windows)
  - [ ] Config: `theme = "tokyo-night"` (by name)
  - [ ] Config: `theme = "/path/to/mytheme.toml"` (by path)
- [ ] Theme discovery: enumerate all `.toml` files in theme directories
- [ ] Theme hot-reload: detect changes to theme files via config monitor

---

## 20.2 Built-in Theme Library

Port popular color schemes as embedded themes.

- [ ] Port themes from iTerm2-Color-Schemes / base16 / Gogh repositories:
  - [ ] Catppuccin (Mocha, Latte, Frappe, Macchiato)
  - [ ] Tokyo Night, Tokyo Night Storm, Tokyo Night Light
  - [ ] One Dark, One Light
  - [ ] Solarized Dark, Solarized Light
  - [ ] Gruvbox Dark, Gruvbox Light, Gruvbox Material
  - [ ] Nord
  - [ ] Dracula
  - [ ] Rose Pine, Rose Pine Moon, Rose Pine Dawn
  - [ ] Everforest Dark, Everforest Light
  - [ ] Kanagawa
  - [ ] Ayu Dark, Ayu Light, Ayu Mirage
  - [ ] Material Dark, Material Light
  - [ ] Monokai Pro, Monokai Classic
  - [ ] Nightfox, Dawnfox, Carbonfox
  - [ ] Zenburn
  - [ ] Base16 set (50+ standardized themes)
- [ ] Script to convert iTerm2 .itermcolors XML to our TOML format
- [ ] Script to convert Ghostty theme format to our TOML format
- [ ] Total target: 100+ themes at launch

---

## 20.3 Light/Dark Auto-Switch

Automatically switch theme based on system appearance.

- [ ] Config: `theme = "dark:tokyo-night, light:tokyo-night-light"`
- [ ] Detect system dark/light mode:
  - [ ] Windows: read `AppsUseLightTheme` registry key
  - [ ] macOS: `NSAppearance` observation
  - [ ] Linux: `org.freedesktop.appearance.color-scheme` D-Bus
- [ ] On system theme change, swap palette and request redraw
- [ ] Settings dropdown: group themes by light/dark/universal
- [ ] Preview theme in dropdown before applying

---

## 20.4 Completion Checklist

- [ ] 100+ themes available by name in config
- [ ] Custom themes loadable from TOML files
- [ ] Light/dark auto-switching works on Windows
- [ ] Settings dropdown lists all available themes
- [ ] Theme hot-reload works (edit theme file, see change)
- [ ] User themes in theme directory discovered automatically

**Exit Criteria:** User can type `theme = "dracula"` in config and get the
Dracula color scheme. System dark/light mode change auto-switches themes.
