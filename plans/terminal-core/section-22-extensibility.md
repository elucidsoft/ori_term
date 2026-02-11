---
section: "22"
title: Extensibility & Advanced Features
status: not-started
goal: Scripting layer, custom shaders, smart paste, and undo close tab
sections:
  - id: "22.1"
    title: Scripting Layer
    status: not-started
  - id: "22.2"
    title: Custom Shaders
    status: not-started
  - id: "22.3"
    title: Smart Paste
    status: not-started
  - id: "22.4"
    title: Undo Close Tab
    status: not-started
  - id: "22.5"
    title: Completion Checklist
    status: not-started
---

# Section 22: Extensibility & Advanced Features

**Status:** Not Started
**Goal:** Long-term differentiation features — scripting, shaders, smart paste,
and quality-of-life additions that make ori_term uniquely powerful.

**Why this matters:** Ghostty has no scripting. WezTerm has Lua but poor
performance. Kitty has kittens but they're Python. A well-designed scripting
layer is the long-term moat — the feature that creates an ecosystem around
ori_term that competitors can't easily replicate.

---

## 22.1 Scripting Layer

Programmable terminal via embedded scripting engine.

- [ ] Evaluate embedding options:
  - [ ] **Lua** (mlua crate) — proven by WezTerm, large ecosystem, fast
  - [ ] **WASM** (wasmtime) — sandboxed, any language, modern
  - [ ] **Rhai** — Rust-native scripting, easy to embed, no FFI
- [ ] Script access to:
  - [ ] Terminal state (current tab, pane, cursor position, CWD)
  - [ ] Actions (new tab, split, close, focus, resize)
  - [ ] Events (on_tab_created, on_output, on_key, on_resize, on_focus)
  - [ ] Configuration (dynamic config override from script)
  - [ ] Appearance (tab title formatting, status bar content)
- [ ] Script loading:
  - [ ] Config: `script = "/path/to/init.lua"`
  - [ ] Auto-load from `~/.config/ori_term/scripts/`
  - [ ] Hot-reload on file change
- [ ] Use cases:
  - [ ] Custom status bar with git branch, time, hostname
  - [ ] Auto-rename tabs based on running command
  - [ ] Custom key bindings with complex logic
  - [ ] Auto-split layouts on startup
  - [ ] Session save/restore (named workspaces)

---

## 22.2 Custom Shaders

Post-processing fragment shaders for visual effects.

- [ ] Config: `custom_shader = "/path/to/shader.wgsl"`
- [ ] Shader receives:
  - [ ] Terminal framebuffer as texture
  - [ ] Cursor position (cell and pixel)
  - [ ] Time (for animations)
  - [ ] Resolution
- [ ] Render pipeline: terminal -> framebuffer texture -> custom shader -> screen
- [ ] Use cases:
  - [ ] CRT effect (scanlines, curvature, bloom)
  - [ ] Colorblindness correction filters
  - [ ] Vignette, film grain
  - [ ] Cursor glow/trail effects
- [ ] Shadertoy compatibility: support Shadertoy uniform names as aliases
- [ ] Hot-reload: detect shader file changes, recompile
- [ ] Error handling: show shader compilation errors, fall back to no-shader

---

## 22.3 Smart Paste

Intelligent paste behavior for safety and convenience.

- [ ] Multi-line paste warning:
  - [ ] If paste contains newlines, show confirmation dialog
  - [ ] "You are about to paste N lines. Continue?"
  - [ ] Option to paste as single line (replace newlines with spaces)
  - [ ] Config: `warn_multiline_paste = true | false` (default: true)
- [ ] Strip leading `$` / `#` / `>` from pasted commands:
  - [ ] Detect when paste starts with common prompt characters
  - [ ] Strip them (user likely copied from a tutorial/README)
  - [ ] Config: `strip_paste_prompt = true | false` (default: false)
- [ ] Bracketed paste safety:
  - [ ] Already supported — ensure all pastes use bracketed paste when mode is on
  - [ ] Sanitize pasted text: strip ESC characters to prevent escape injection
- [ ] Large paste warning:
  - [ ] If paste > 1MB, warn user
  - [ ] Config: `large_paste_threshold = 1048576` (bytes)

---

## 22.4 Undo Close Tab

Restore accidentally closed tabs.

- [ ] Maintain a closed-tab stack (last N closed tabs):
  - [ ] Store: scrollback content, CWD, tab title, color scheme
  - [ ] Do NOT store running process (can't resurrect a PTY)
- [ ] `Ctrl+Shift+T` — reopen last closed tab
  - [ ] Create new tab with stored CWD
  - [ ] Optionally prepopulate scrollback with saved content
- [ ] Stack limit: 10 most recently closed tabs
- [ ] Visual: brief "Tab closed — Ctrl+Shift+T to undo" toast notification

---

## 22.5 Completion Checklist

- [ ] Scripting engine embedded and loading user scripts
- [ ] Scripts can react to events and invoke actions
- [ ] Custom shaders render as post-processing pass
- [ ] Shader hot-reload works
- [ ] Multi-line paste shows confirmation
- [ ] Paste stripping works for prompt characters
- [ ] Undo close tab restores CWD and title
- [ ] All features documented in config reference

**Exit Criteria:** ori_term has a scripting layer that enables an ecosystem of
user-created extensions, custom visual effects via shaders, and quality-of-life
paste safety features.
