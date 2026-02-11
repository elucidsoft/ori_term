---
section: "16"
title: Split Panes
status: not-started
goal: Support horizontal and vertical terminal splits within a single window
sections:
  - id: "16.1"
    title: Split Data Model
    status: not-started
  - id: "16.2"
    title: Split Creation & Navigation
    status: not-started
  - id: "16.3"
    title: Split Rendering
    status: not-started
  - id: "16.4"
    title: Split Resize
    status: not-started
  - id: "16.5"
    title: Completion Checklist
    status: not-started
---

# Section 16: Split Panes

**Status:** Not Started
**Goal:** Allow users to split the terminal window into multiple panes, each running
its own shell/tab, with keyboard and mouse navigation between them.

**Why this matters:** Split panes are the #1 most-requested missing feature in
terminal comparisons. Ghostty, Kitty, and WezTerm all have them. Many users
cite splits as the reason they can't leave tmux. Built-in splits with proper
GPU rendering and native keybindings are faster and more polished than tmux.

**Inspired by:**
- Ghostty: native AppKit/GTK splits, platform-specific UI
- WezTerm: Lua-configurable split layouts, zoom/unzoom
- Kitty: flexible window layouts (tall, fat, grid, splits, stack)
- tmux: the baseline expectation for split behavior

**Current state:** Each window has a flat `Vec<TabId>` of tabs. No concept of
spatial layout within a tab. The `TermWindow` struct needs a layout tree.

---

## 16.1 Split Data Model

Replace flat tab list with a binary tree layout.

- [ ] Define `SplitTree` enum:
  ```
  enum PaneNode {
      Leaf(TabId),
      Split { direction: SplitDirection, ratio: f32, first: Box<PaneNode>, second: Box<PaneNode> },
  }
  enum SplitDirection { Horizontal, Vertical }
  ```
- [ ] Each tab in the window participates in the layout tree
- [ ] `active_pane: TabId` tracks which pane has focus
- [ ] Ratio is 0.0–1.0 (default 0.5 for even split)
- [ ] Tree can be arbitrarily nested (split a split)
- [ ] Each leaf calculates its pixel rect from parent constraints

---

## 16.2 Split Creation & Navigation

Keybindings and focus management.

- [ ] Create splits:
  - [ ] `Ctrl+Shift+D` — split horizontal (new pane below)
  - [ ] `Ctrl+Shift+E` — split vertical (new pane right)
  - [ ] New pane spawns a new tab with a new PTY
  - [ ] New pane inherits CWD from focused pane (requires OSC 7 / shell integration)
- [ ] Navigate between panes:
  - [ ] `Alt+Arrow` — move focus to pane in direction
  - [ ] `Alt+Tab` — cycle focus between panes
  - [ ] Click on a pane to focus it
- [ ] Close pane:
  - [ ] `Ctrl+W` closes the focused pane (not the whole tab)
  - [ ] When a split has one child removed, collapse to the remaining child
  - [ ] When last pane closes, close the tab
- [ ] Zoom/unzoom:
  - [ ] `Ctrl+Shift+Z` — toggle zoom on focused pane (fills entire tab area)
  - [ ] Zoomed pane shows indicator in tab bar or border

---

## 16.3 Split Rendering

Draw split borders and render each pane independently.

- [ ] Calculate pixel rects for each leaf node in the split tree
- [ ] Each pane gets its own grid rendering area (offset + size)
- [ ] PTY resize: each pane gets its own cols/rows based on pixel rect
- [ ] Split divider:
  - [ ] 1-2px line between panes (configurable color)
  - [ ] Active pane has highlighted border or accent color
  - [ ] Inactive panes optionally dimmed
- [ ] Render order: backgrounds, then split dividers, then foreground glyphs per pane
- [ ] Each pane maintains its own scroll position, selection, cursor

---

## 16.4 Split Resize

Drag to resize splits.

- [ ] Mouse drag on split divider resizes the split ratio
  - [ ] Cursor changes to resize icon when hovering divider (3-5px hit zone)
  - [ ] Minimum pane size: ~4 columns / 2 rows
- [ ] Keyboard resize:
  - [ ] `Alt+Shift+Arrow` — resize focused pane in direction
  - [ ] Step size: ~5% of parent dimension
- [ ] Equalize: `Ctrl+Shift+=` — reset all split ratios to equal
- [ ] Window resize reflows all panes proportionally

---

## 16.5 Completion Checklist

- [ ] Horizontal and vertical splits work
- [ ] Nested splits (split a split) work
- [ ] Keyboard navigation between panes
- [ ] Mouse click to focus pane
- [ ] Drag to resize split divider
- [ ] Close pane collapses the split tree correctly
- [ ] Each pane has independent scroll, selection, cursor
- [ ] PTY resize sent to each pane independently
- [ ] Zoom/unzoom a single pane
- [ ] No rendering artifacts at split boundaries

**Exit Criteria:** User can create, navigate, resize, and close split panes
with no tmux needed for basic multi-pane workflows.
