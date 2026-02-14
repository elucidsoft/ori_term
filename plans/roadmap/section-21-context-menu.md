---
section: 21
title: Context Menu & Window Controls
status: not-started
tier: 4
goal: GPU-rendered context menus, config reload broadcasting, settings UI, window controls
sections:
  - id: "21.1"
    title: Context Menu
    status: not-started
  - id: "21.2"
    title: Config Reload Broadcasting
    status: not-started
  - id: "21.3"
    title: Settings UI
    status: not-started
  - id: "21.4"
    title: Window Controls
    status: not-started
  - id: "21.5"
    title: Section Completion
    status: not-started
---

# Section 21: Context Menu & Window Controls

**Status:** 📋 Planned
**Goal:** GPU-rendered context menus, config reload broadcasting, settings UI, window controls. This is the final feature parity section — completing it means the rebuild matches the old prototype's full capability.

**Crate:** `oriterm` (binary only — no core changes)

**Reference:** `_old/src/context_menu.rs`, `_old/src/gpu/render_overlay.rs`, `_old/src/app/config_reload.rs`, `_old/src/app/settings_ui.rs`, `_old/src/gpu/render_settings.rs`, `_old/src/gpu/render_tab_bar.rs`, `_old/src/tab_bar.rs`

---

## 21.1 Context Menu

GPU-rendered context menus (not OS native) for consistent cross-platform styling. Three distinct menu types depending on what was right-clicked.

**File:** `oriterm/src/chrome/context_menu.rs`

**Reference:** `_old/src/context_menu.rs`, `_old/src/gpu/render_overlay.rs`

- [ ] `MenuOverlay` struct:
  - [ ] `entries: Vec<MenuEntry>` — menu items
  - [ ] `position: (f32, f32)` — absolute pixel position (top-left corner)
  - [ ] `hovered: Option<usize>` — currently hovered entry index (None if not hovering any item)
  - [ ] `width: f32` — computed menu width
  - [ ] `height: f32` — computed menu height
  - [ ] `scale: f32` — DPI scale factor
- [ ] `MenuEntry` enum:
  - [ ] `Item { label: String, action: ContextAction }` — clickable item
  - [ ] `Check { label: String, checked: bool, action: ContextAction }` — item with checkmark indicator
  - [ ] `Separator` — horizontal line divider
- [ ] Three menu contexts:
  1. [ ] **Tab context menu** (right-click on a tab):
     - [ ] Close Tab
     - [ ] Duplicate Tab
     - [ ] Move to New Window
  2. [ ] **Grid context menu** (right-click in terminal area):
     - [ ] Copy (enabled only if selection exists)
     - [ ] Paste
     - [ ] Select All
     - [ ] Separator
     - [ ] New Tab
     - [ ] Close Tab
     - [ ] Separator
     - [ ] Settings
  3. [ ] **Dropdown menu** (click dropdown button in tab bar):
     - [ ] Settings (opens settings window)
     - [ ] Separator
     - [ ] Color scheme selector: list all built-in schemes with `Check` entries (active scheme has checkmark)
- [ ] Layout calculation:
  - [ ] Measure max label width using UI font collection
  - [ ] If any `Check` entry exists: add checkmark icon width + gap
  - [ ] `width = max_label_width + 2 * ITEM_PADDING_X + MENU_EXTRA_WIDTH`, clamped to `MENU_MIN_WIDTH`
  - [ ] `height = 2 * MENU_PADDING_Y + sum(entry_height for each entry)`
  - [ ] Entry heights: `ITEM_HEIGHT` for Item/Check, `SEPARATOR_HEIGHT` for Separator
- [ ] Hit testing:
  - [ ] `hit_test(x: f32, y: f32) -> Option<usize>`
  - [ ] Check if point is within menu rect
  - [ ] Iterate entries, accumulate Y offset
  - [ ] Return entry index if clickable (skip separators)
  - [ ] Return None if outside or on separator
- [ ] Dismiss conditions:
  - [ ] Click outside menu rect
  - [ ] Escape key
  - [ ] Any action selected and executed
- [ ] GPU rendering (overlay pass, topmost):
  - [ ] Shadow rectangle (2px offset down-right, rounded corners, semi-transparent)
  - [ ] Menu background rectangle (rounded corners)
  - [ ] Per-entry:
    - [ ] **Item**: text label at `ITEM_PADDING_X` from left
    - [ ] **Check**: checkmark icon (if checked) + label indented past icon
    - [ ] **Separator**: horizontal line with left/right margins
    - [ ] Hover highlight: rounded rectangle with inset, lighter background
- [ ] Menu constants:
  - [ ] `MENU_PADDING_Y: f32` — vertical padding inside menu
  - [ ] `ITEM_PADDING_X: f32` — horizontal padding for labels
  - [ ] `SEPARATOR_MARGIN_X: f32` — left/right margin for separator lines
  - [ ] `SEPARATOR_THICKNESS: f32` — separator line height
  - [ ] `ITEM_HEIGHT: f32` — height per clickable item
  - [ ] `MENU_MIN_WIDTH: f32` — minimum menu width
  - [ ] `MENU_EXTRA_WIDTH: f32` — extra padding for checkmark column
  - [ ] `MENU_RADIUS: f32` — corner radius for menu shape
  - [ ] `ITEM_HOVER_INSET: f32` — inset of hover highlight from menu edges
  - [ ] `ITEM_HOVER_RADIUS: f32` — corner radius for hover highlight

---

## 21.2 Config Reload Broadcasting

When the config file changes (detected by file watcher), changes must be applied to ALL tabs and ALL windows consistently. Some changes (font) require expensive atlas rebuilds and grid reflow.

**File:** `oriterm/src/app/config_reload.rs`

**Reference:** `_old/src/app/config_reload.rs`

- [ ] `apply_config_reload(&mut self)`:
  - [ ] Load new config from disk via `Config::try_load()` — if parse fails, log error and return (keep current config)
  - [ ] **Color scheme changes**: if `new.colors.scheme != old.colors.scheme`:
    - [ ] Resolve scheme from built-in list: `palette::find_scheme(&name)`
    - [ ] Apply to ALL tabs: `tab.apply_color_config(scheme, &colors, bold_is_bright)`
    - [ ] Mark all grids dirty
  - [ ] **Font changes**: if any of `size`, `family`, `features`, `fallback`, `weight`, `tab_bar_font_weight`, `tab_bar_font_family` changed:
    - [ ] Rebuild main font collection at `new_size * scale_factor`
    - [ ] Rebuild UI font collection at `new_size * scale_factor * UI_FONT_SCALE`
    - [ ] Rebuild glyph atlas (expensive — clears all cached glyphs)
    - [ ] **Resize ALL tabs in ALL windows** — cell dimensions changed, grids need reflow:
      - [ ] For each window (skipping settings window):
        - [ ] For each tab: `tab.clear_selection()`, `tab.resize(new_cols, new_rows, ...)`
    - [ ] Log: `"config reload: font size={}, cell={}x{}, tab_bar_weight={}"`
  - [ ] **Cursor style changes**: if `new.terminal.cursor_style != old.terminal.cursor_style`:
    - [ ] Parse new cursor shape
    - [ ] Apply to ALL tabs: `tab.set_cursor_shape(new_cursor)`
  - [ ] **Keybinding changes**:
    - [ ] Rebuild binding table: `self.bindings = keybindings::merge_bindings(&new.keybind)`
  - [ ] **Opacity changes**: mark all windows for redraw (compositor effect may need update)
  - [ ] Store new config: `self.config = new_config`
  - [ ] Mark `tab_bar_dirty = true`, all grids dirty
  - [ ] Request redraw on all windows

---

## 21.3 Settings UI

Separate frameless settings window (not an overlay). Displays color scheme selector with live preview. GPU-rendered for consistent styling.

**File:** `oriterm/src/app/settings_ui.rs`, `oriterm/src/gpu/render_settings.rs`

**Reference:** `_old/src/app/settings_ui.rs`, `_old/src/gpu/render_settings.rs`

- [ ] `settings_window: Option<WindowId>` on App — None if settings not open
- [ ] Settings window lifecycle:
  - [ ] `open_settings_window(event_loop)` — create separate small window (~300×350px), init GPU surface
  - [ ] `close_settings_window()` — remove from windows map, set `settings_window = None`
  - [ ] Only Escape key works in settings window (all other input consumed)
- [ ] Settings window content:
  - [ ] Title bar: "Theme" label + close button (top-right corner, 30×30px)
  - [ ] Color scheme list: rows of ~40px height each:
    - [ ] Color swatch: 16×16px square showing scheme's background color
    - [ ] Scheme name: text label 40px from left
    - [ ] Active indicator: checkmark icon if this is the current scheme
    - [ ] Hover highlight: rounded rect across full row width
- [ ] Mouse handling:
  - [ ] Top-right 30×30px: close button
  - [ ] Top 50px: title area (no interaction)
  - [ ] Below: scheme rows. `row_idx = (y - 50) / 40`
  - [ ] Click on row: `apply_scheme_to_all_tabs(scheme)`
- [ ] Scheme application:
  - [ ] Update `self.active_scheme`
  - [ ] Apply to ALL tabs: `tab.apply_color_config(scheme, &config.colors, bold_is_bright)`
  - [ ] Persist to config file: `self.config.colors.scheme = scheme.name; self.config.save()`
  - [ ] Request redraw on all windows
- [ ] GPU rendering:
  - [ ] Full-window background (dark, derived from palette)
  - [ ] 1px border on all edges
  - [ ] Per-row rendering with color derivation from palette
  - [ ] This is a stretch goal — can be deferred past initial feature parity

---

## 21.4 Window Controls

Custom window controls for the frameless window, integrated into the tab bar. Platform-specific rendering (rectangular on Windows, circular on macOS/Linux).

**File:** `oriterm/src/chrome/tab_bar.rs` (integrated with tab bar rendering)

**Reference:** `_old/src/gpu/render_tab_bar.rs`, `_old/src/tab_bar.rs`

- [ ] Three buttons in top-right corner of tab bar:
  - [ ] Minimize (─): `window.set_minimized(true)`
  - [ ] Maximize (□ / ⧉): toggle `window.set_maximized()` — icon changes based on `is_maximized`
  - [ ] Close (×): close window
- [ ] Platform-specific rendering:
  - [ ] **Windows**: Three rectangular buttons, each `CONTROL_BUTTON_WIDTH` (58px) wide:
    - [ ] Minimize: horizontal line icon
    - [ ] Maximize: single square icon (when not maximized) or two overlapping squares with erase-out (when maximized/restored)
    - [ ] Close: X icon (two diagonal small rectangles)
    - [ ] Close button hover: red background (`CONTROL_CLOSE_HOVER_BG`) with white icon (`CONTROL_CLOSE_HOVER_FG`)
    - [ ] Other buttons hover: subtle background change (`control_hover_bg`)
  - [ ] **Linux/macOS**: Three circular buttons:
    - [ ] Diameter: 24px
    - [ ] Spacing: 8px between buttons
    - [ ] Margins: 12px from edges
    - [ ] Colored circles with icons on hover
- [ ] Window dragging:
  - [ ] Double-click on `DragArea` (empty tab bar space): toggle maximize
  - [ ] Click + drag on `DragArea`: `window.drag_window()` — OS handles movement
  - [ ] Aero Snap on Windows: handled by OS via `drag_window()` when custom WndProc subclass is installed
- [ ] Aero Snap subclass (Windows-specific):
  - [ ] Custom `WndProc` that handles `WM_NCHITTEST` — returns `HTCAPTION` for drag areas, `HTCLIENT` for interactive areas
  - [ ] Also handles `WM_DPICHANGED` — stores new DPI for `handle_resize()` to read
  - [ ] Required because frameless windows don't get Snap behavior by default

---

## 21.5 Section Completion

This is the **final feature parity checkpoint**. Completing this section means the rebuild matches the old prototype's full capability.

- [ ] All 21.1–21.4 items complete
- [ ] Context menu: 3 menu types, GPU-rendered, checkmark entries, shadow rendering
- [ ] Config reload: broadcast to all tabs/windows, font atlas rebuild, grid reflow
- [ ] Settings UI: separate window, color scheme selector, live preview, persist to config
- [ ] Window controls: platform-specific rendering, Aero Snap, frameless drag
- [ ] Tab struct: clean ownership, lock-free mode cache, background thread cleanup
- [ ] Tab management: create, close, duplicate, cycle, reorder, CWD inheritance
- [ ] Tab bar layout: DPI-aware, width lock, platform-specific control zone
- [ ] Tab bar rendering: separators with suppression, bell pulse, dragged tab overlay, animation offsets
- [ ] Hit testing: correct priority order, close button inset, platform-specific controls
- [ ] Drag: 10px threshold, center-based insertion, tear-off with directional thresholds, mouse offset preservation
- [ ] OS drag + merge: WM_MOVING detection, seamless drag continuation, synthesized mouse-down, stale button-up suppression
- [ ] Multi-window: shared GPU, flat tab storage, cross-window tab movement
- [ ] Window lifecycle: no-flash startup, DPI-aware resize, ConPTY-safe cleanup, exit-before-drop
- [ ] Coordinate systems: pixel → cell, tab bar layout, grid padding, side detection
- [ ] Event routing: 7-layer keyboard dispatch, 7-layer mouse dispatch, search/menu interception
- [ ] Render scheduling: about_to_wait coalescing, 8ms frame budget, cursor blink scheduling
- [ ] Shell integration: 5 shell injection mechanisms, two-parser strategy, CWD tracking, prompt state machine, title priority
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu --release` — clean build
- [ ] `cargo clippy -p oriterm -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test -p oriterm_core` — all tests pass
- [ ] Deploy to Windows: full feature parity with old prototype
- [ ] **Stress test**: multiple tabs, htop in one, vim in another, heavy output in third — no lockup, no starvation
- [ ] **Drag stress test**: rapid drag reorder across multiple windows, tear-off and merge in quick succession — no crash, no orphaned tabs
- [ ] **Close stress test**: rapidly close many tabs while hovering tab bar — close buttons don't shift unexpectedly (tab width lock works)

**Exit Criteria:** Feature parity with the old prototype. Clean architecture, clean threading, no god objects, no contention issues. Every intricacy from the old prototype is preserved: ConPTY deadlock avoidance, seamless drag-merge, mode cache, CWD inheritance, tab width lock, bell pulse animation, prompt state deferred marking, keyboard mode stack swap on alt screen. The terminal emulator is ready for daily use.
