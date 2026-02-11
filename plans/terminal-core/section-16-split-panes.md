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

**Current state:** Each window has a flat `Vec<TabId>` of tabs in
`TermWindow.tabs` (`src/window.rs`). Tabs live in `App.tabs: HashMap<TabId, Tab>`
separate from windows — this separation already enables tab tear-off. No concept
of spatial layout within a tab. The renderer (`src/gpu/renderer.rs`) draws one
grid per window, with grid origin and dimensions calculated from window size.

**Architecture impact:** This is the largest architectural change remaining. It
requires either (a) replacing the flat tab list with a tree layout, or (b)
making each "tab" contain a tree of panes. Option (b) is cleaner: a tab becomes
a layout container, each leaf in the tree is a pane (shell + grid). This matches
Ghostty and WezTerm's approach.

---

## 16.1 Split Data Model

Replace flat tab list with a binary tree layout per tab.

**Design:** Each tab entry becomes a `PaneTree` — a binary tree where leaves are
terminal panes and internal nodes are splits.

```rust
/// A single terminal pane: owns a Grid, PTY, and VTE parser.
struct Pane {
    id: PaneId,
    grid_primary: Grid,
    grid_alt: Grid,
    active_is_alt: bool,
    pty_writer: Option<Box<dyn Write + Send>>,
    pty_master: Box<dyn MasterPty>,
    _child: Box<dyn Child>,
    processor: vte::ansi::Processor,
    raw_parser: vte::Parser,
    // ... (all fields currently in Tab)
}

/// Layout tree for a single tab.
enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,       // 0.0–1.0, default 0.5
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

enum SplitDirection { Horizontal, Vertical }

/// A PaneId is globally unique, like TabId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PaneId(u64);
```

**Migration path from current Tab struct:**
1. Extract all terminal-session fields from `Tab` into a new `Pane` struct
2. `Tab` becomes: `{ title, pane_tree: PaneNode, active_pane: PaneId, ... }`
3. `App.panes: HashMap<PaneId, Pane>` replaces the terminal fields in `App.tabs`
4. `App.tabs` keeps metadata (title, color scheme, selection of "tab" in tab bar)
5. When a tab has a single pane (no splits), `PaneNode::Leaf(pane_id)` — functionally identical to today

- [ ] Define `PaneId`, `Pane`, `PaneNode`, `SplitDirection` types
- [ ] Refactor `Tab` to separate session state (`Pane`) from tab metadata
- [ ] Add `PaneNode` to `Tab` (defaults to `Leaf` wrapping one pane)
- [ ] `active_pane: PaneId` on `Tab` tracks which pane has focus
- [ ] `App.panes: HashMap<PaneId, Pane>` global pane registry
- [ ] Ratio is 0.0–1.0 (default 0.5 for even split)
- [ ] Tree can be arbitrarily nested (split a split)
- [ ] `PaneNode::compute_rects(total_rect) -> Vec<(PaneId, Rect)>`:
  recursively subdivide the available pixel area

**Key design decisions:**
- Panes are **not** tabs in the tab bar — the tab bar still shows tabs,
  each tab may contain 1+ panes
- Tab tear-off still works: tearing off moves the whole tab (with all its panes)
- PTY events use `PaneId` instead of `TabId` for routing `PtyOutput`

---

## 16.2 Split Creation & Navigation

Keybindings and focus management.

- [ ] Create splits:
  - [ ] `Ctrl+Shift+D` — split horizontal (new pane below)
  - [ ] `Ctrl+Shift+E` — split vertical (new pane right)
  - [ ] New pane spawns a new `Pane` with a new PTY
  - [ ] New pane inherits CWD from focused pane (if OSC 7 reported a CWD)
  - [ ] Insert split: replace `Leaf(active)` with `Split { first: Leaf(active), second: Leaf(new) }`
- [ ] Navigate between panes:
  - [ ] `Alt+Arrow` — move focus to pane in direction
    - [ ] Find pane whose rect center is closest in the given direction
    - [ ] Only consider panes in the same tab
  - [ ] `Alt+[` / `Alt+]` — cycle focus between panes (in tree order)
  - [ ] Click on a pane to focus it (grid area hit-test per pane rect)
  - [ ] Visual indicator: focused pane has a colored border or accent on its edge
- [ ] Close pane:
  - [ ] `Ctrl+W` closes the focused pane (not the whole tab)
  - [ ] When a split has one child removed, collapse: replace `Split { first, _ }` with `first`
  - [ ] When last pane closes, close the tab
  - [ ] Closing a pane kills its PTY and removes it from `App.panes`
- [ ] Zoom/unzoom:
  - [ ] `Ctrl+Shift+Z` — toggle zoom on focused pane (fills entire tab area)
  - [ ] Store `zoomed_pane: Option<PaneId>` on Tab
  - [ ] When zoomed, render only that pane at full tab dimensions
  - [ ] Tab bar shows "[Z]" or zoom icon when a pane is zoomed
  - [ ] Any split/navigate action unzooms first

---

## 16.3 Split Rendering

Draw split borders and render each pane independently.

- [ ] Layout computation:
  - [ ] `PaneNode::compute_rects(available: Rect) -> Vec<(PaneId, Rect)>`
  - [ ] Subtract divider width (2px) when splitting
  - [ ] Each pane's pixel rect → cols/rows for grid and PTY resize
- [ ] Render each pane:
  - [ ] `build_grid_instances()` already takes grid dimensions and offset
  - [ ] Call it once per pane, with each pane's offset and size
  - [ ] Each pane has its own: cursor, scroll position, selection
- [ ] Split divider rendering:
  - [ ] 2px line between panes (palette surface color)
  - [ ] Active pane border: highlight the focused pane's edge with accent color
  - [ ] Inactive panes optionally dimmed (lower opacity — multiply fg alpha by 0.7)
- [ ] Render order:
  1. All pane backgrounds (one pass)
  2. Split dividers
  3. All pane foreground glyphs (one pass)
  4. Cursor for active pane
  5. Selection highlights per pane
- [ ] PTY resize: when layout changes, resize each pane's PTY independently
  - [ ] `pane.pty_master.resize(pane_cols, pane_rows)`

---

## 16.4 Split Resize

Drag to resize splits.

- [ ] Mouse drag on split divider:
  - [ ] Detect hover on divider: 5px hit zone centered on the 2px divider
  - [ ] Change cursor to resize icon (`CursorIcon::ColResize` / `RowResize`)
  - [ ] On drag: update `ratio` in the `Split` node
  - [ ] Clamp ratio so minimum pane size is 4 columns / 2 rows
  - [ ] Resize all affected pane PTYs after ratio change
- [ ] Keyboard resize:
  - [ ] `Alt+Shift+Arrow` — resize focused pane in direction
  - [ ] Find the nearest split ancestor in the direction
  - [ ] Adjust ratio by ±5% of parent dimension
- [ ] Equalize: `Ctrl+Shift+=` — reset all split ratios to 0.5 (recursive)
- [ ] Window resize: pane pixel rects recalculated proportionally
  - [ ] Each pane gets `resize()` called with new cols/rows
  - [ ] Text reflow applies independently per pane

---

## 16.5 Completion Checklist

- [ ] Horizontal and vertical splits work
- [ ] Nested splits (split a split) work
- [ ] Keyboard navigation between panes (Alt+Arrow, Alt+[/])
- [ ] Mouse click to focus pane
- [ ] Drag to resize split divider
- [ ] Close pane collapses the split tree correctly
- [ ] Each pane has independent scroll, selection, cursor
- [ ] PTY resize sent to each pane independently
- [ ] Zoom/unzoom a single pane
- [ ] No rendering artifacts at split boundaries
- [ ] Tab tear-off works with multi-pane tabs
- [ ] Performance: multiple panes don't cause frame drops

**Exit Criteria:** User can create, navigate, resize, and close split panes
with no tmux needed for basic multi-pane workflows.
