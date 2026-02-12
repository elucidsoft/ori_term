---
name: design-pattern-review
description: Compare ori_term's design against prior art from established terminal emulators and propose best-of-breed designs
allowed-tools: Read, Grep, Glob, Task, Bash, Write
---

# Design Pattern Review

Compare ori_term's current implementation in a specific domain against established terminal emulators (Alacritty, WezTerm, Ghostty, Crossterm, Ratatui), then propose a best-of-breed design combining the strongest patterns.

**One domain per invocation.** This keeps each run focused and within context limits.

## Invocation

`/design-pattern-review <domain>`

### Domains

| Domain | ori_term Files | Best Reference Repos | Prior Art Section |
|--------|---------------|---------------------|-------------------|
| `grid-buffer` | grid/mod.rs, cell.rs | Alacritty, Ghostty, WezTerm | 1 |
| `gpu-rendering` | gpu/renderer.rs, gpu/atlas.rs, gpu/pipeline.rs | Alacritty, WezTerm, Ghostty | 2 |
| `input-encoding` | key_encoding.rs, app.rs (input dispatch) | Alacritty, Ghostty, WezTerm | 3 |
| `pty-management` | pty.rs, tab.rs (PTY lifecycle) | Alacritty, WezTerm, Ghostty | 4 |
| `vte-handling` | term_handler.rs | Alacritty, WezTerm, Ghostty | 5 |
| `selection` | selection.rs | Alacritty, WezTerm, Ghostty | 6 |
| `font-rendering` | render.rs, gpu/atlas.rs | Alacritty, WezTerm, Ghostty | 7 |
| `color-system` | palette.rs, cell.rs (color fields) | Alacritty, Ghostty, Crossterm | 8 |
| `tab-architecture` | tab.rs, tab_bar.rs, drag.rs, app.rs | WezTerm, Ghostty | 9 |

If `$ARGUMENTS` is empty or unrecognized, use `AskUserQuestion` to present the domain list.

---

## Execution

### Step 1: Resolve Domain

Parse `$ARGUMENTS`. Match against the domain table. Determine:
- Which ori_term files Agent A should read (from the file map below)
- Which reference repos Agent B should read (from the file map below)
- Which prior-art-ref.md section to reference

### Step 2: Launch Two Research Agents in Parallel

Send a **single message with 2 Task tool calls**, both with `run_in_background: true`.

#### Agent A: Read ori_term's Current Implementation

```
Task(
  subagent_type: "Explore",
  description: "Read ori_term: {domain}",
  run_in_background: true,
  prompt: <see Agent A template below, filled with domain-specific file list>
)
```

#### Agent B: Read Reference Terminal Emulators

```
Task(
  subagent_type: "Explore",
  description: "Read Refs: {domain}",
  run_in_background: true,
  prompt: <see Agent B template below, filled with domain-specific repos/paths>
)
```

### Step 3: Collect Research Results

Read both agents' output files. Extract their summaries — do NOT re-investigate or expand them.

### Step 4: Launch Synthesis Agent

```
Task(
  subagent_type: "general-purpose",
  description: "Synthesize: {domain}",
  prompt: <see Agent C template below, with both summaries injected>
)
```

This agent writes the proposal to `plans/dpr_{domain}_{MMDDYYYY}.md`.

### Step 5: Report

Tell the user where the proposal was written. Give a 3-5 line summary of the key design insight.

---

## Agent Prompt Templates

### Agent A: ori_term Current State

```
You are analyzing ori_term's current implementation of {DOMAIN}.

ori_term is a GPU-accelerated terminal emulator in Rust, cross-compiled from WSL
targeting x86_64-pc-windows-gnu. It uses wgpu for rendering, fontdue for font
rasterization, ConPTY for shell processes, and winit for windowing.

Read these files thoroughly:

{DOMAIN-SPECIFIC ORI_TERM FILE LIST — see file map below}

Produce a structured summary covering:

## ori_term's Current {Domain} Design

### Architecture
How it's structured: key types, data flow, module organization.

### Strengths
3-5 bullet points on what works well.

### Gaps & Pain Points
3-5 bullet points on what's missing, awkward, or inconsistent.

### Key Types & Interfaces
The most important types/traits/functions with brief descriptions.

RULES:
- Actually READ each file — don't just grep for patterns
- Focus on DESIGN CHOICES, not code style or hygiene
- Keep output under 150 lines
- Be honest about gaps — this is for improvement, not validation
```

### Agent B: Reference Terminal Emulator Research

```
You are researching how established terminal emulators handle {DOMAIN}.

STEP 1: Read `.claude/skills/design-pattern-review/prior-art-ref.md` section {N}
for an overview of patterns in this domain.

STEP 2: Dive into these reference repos at ~/projects/reference_repos/console_repos/:

{DOMAIN-SPECIFIC REPO PATHS — see file map below, 2-3 repos}

For each terminal emulator, read the actual source files listed. Extract the key
design patterns — don't just describe the API surface, understand WHY they made
each design choice.

Produce a structured summary:

## Prior Art: {Domain}

### {Emulator 1} — {Their Key Pattern Name}
**Approach:** 1-2 sentence summary
**Key Design Choice:** The most important decision and why they made it
**Concrete Pattern:** Brief code structure or type layout
**Tradeoff:** What they gain vs what they sacrifice

### {Emulator 2} — {Their Key Pattern Name}
{same format}

### {Emulator 3} — {Their Key Pattern Name}
{same format}

### Cross-Cutting Patterns
Patterns appearing in 2+ emulators — these are likely universal best practices.

RULES:
- READ actual source files, not just file names
- Focus on the 2-3 most relevant repos, not all of them
- Extract DESIGN PATTERNS, not implementation details
- Keep output under 150 lines
- Note disagreements between emulators — where they chose differently, explain why
```

### Agent C: Best-of-Breed Synthesis

```
You are synthesizing a best-of-breed design for ori_term's {DOMAIN}.

You have two research summaries. Read them carefully:

--- ORI_TERM CURRENT STATE ---
{Agent A output — paste full summary here}

--- PRIOR ART ---
{Agent B output — paste full summary here}

YOUR TASK: Write a design proposal that combines the strongest patterns from
reference terminal emulators, adapted to ori_term's unique constraints:
- GPU-accelerated via wgpu (not OpenGL or Metal)
- Cross-compiled from WSL targeting Windows (x86_64-pc-windows-gnu)
- ConPTY for shell process management
- Frameless window with custom Chrome-style tab bar
- fontdue for font rasterization (no HarfBuzz/FreeType dependency)
- WGSL shaders
- Single-process, multi-tab architecture (not multiplexer)

Write the proposal to: plans/dpr_{DOMAIN}_{MMDDYYYY}.md

USE THIS EXACT FORMAT:

---
plan: "dpr_{DOMAIN}_{MMDDYYYY}"
title: "Design Pattern Review: {Domain Title}"
status: draft
---

# Design Pattern Review: {Domain Title}

## ori_term Today

{2-3 paragraphs: what exists, what works, what's missing. Be specific — cite
types, functions, modules. Don't just say "it works well", say why.}

## Prior Art

### {Emulator 1} — {Key Pattern Name}
{What they do, why it works, 1-2 paragraphs}

### {Emulator 2} — {Key Pattern Name}
{same}

### {Emulator 3} — {Key Pattern Name}
{same}

## Proposed Best-of-Breed Design

### Core Idea
{1-2 paragraphs: the combined design. What are we taking from each emulator
and how do they fit together?}

### Key Design Choices
{Numbered list. Each choice cites which emulator(s) inspired it and explains
why it's the right fit for ori_term specifically.}

### What Makes ori_term's Approach Unique
{Where ori_term's constraints (wgpu, ConPTY, frameless window, fontdue) create
opportunities that none of the reference emulators have. This is the novel
contribution.}

### Concrete Types & Interfaces
{Rust pseudocode sketching the key types, traits, or functions. This should be
concrete enough to start implementing from.}

## Implementation Roadmap

### Phase 1: Foundation
- [ ] {task with brief description}

### Phase 2: Core
- [ ] {task}

### Phase 3: Polish
- [ ] {task}

## References
{List each reference repo file that was studied}

RULES:
- Be CONCRETE — pseudocode over prose
- Cite your sources — every design choice should reference which emulator inspired it
- Don't just copy one emulator — combine the best of multiple
- Address ori_term's unique constraints explicitly
- The proposal should be implementable, not aspirational
```

---

## Domain-Specific File Maps

Use these to populate Agent A and Agent B prompts.

### grid-buffer

**Agent A (ori_term):**
- `src/grid/mod.rs` — grid structure, scrollback, cursor, reflow
- `src/cell.rs` — Cell (24 bytes), CellFlags, color storage
- `src/tab.rs` — Tab owns Grid (primary + alt screen)

**Agent B (Refs):**
- Alacritty: `alacritty_terminal/src/grid/mod.rs`, `grid/storage.rs`, `grid/row.rs`, `grid/resize.rs`
- Ghostty: `src/terminal/PageList.zig`, `src/terminal/Screen.zig`, `src/terminal/page.zig`
- WezTerm: `term/src/screen.rs`, `wezterm-surface/src/line/line.rs`, `wezterm-cell/src/lib.rs`
- **Prior Art Section:** 1

### gpu-rendering

**Agent A (ori_term):**
- `src/gpu/renderer.rs` — wgpu renderer, draw_frame, cell loop
- `src/gpu/atlas.rs` — glyph atlas (1024x1024 shelf packing)
- `src/gpu/pipeline.rs` — WGSL shader pipelines
- `src/gpu/render_overlay.rs` — overlay rendering (search, hints)
- `src/gpu/render_tab_bar.rs` — tab bar GPU rendering

**Agent B (Refs):**
- Alacritty: `alacritty/src/renderer/mod.rs`, `renderer/text/atlas.rs`, `renderer/text/glyph_cache.rs`, `display/damage.rs`
- Ghostty: `src/renderer/generic.zig`, `src/renderer/Metal.zig`, `src/renderer/cell.zig`, `src/renderer/row.zig`
- WezTerm: `wezterm-gui/src/renderstate.rs`, `wezterm-gui/src/glyphcache.rs`, `wezterm-gui/src/termwindow/render/mod.rs`, `wezterm-gui/src/shader.wgsl`
- **Prior Art Section:** 2

### input-encoding

**Agent A (ori_term):**
- `src/key_encoding.rs` — Kitty + legacy key encoding
- `src/app.rs` — input dispatch (keyboard_input, key handling)

**Agent B (Refs):**
- Alacritty: `alacritty/src/input/mod.rs`, `alacritty/src/input/keyboard.rs`
- Ghostty: `src/input/Binding.zig`, `src/input/key.zig`, `src/input/key_encode.zig`, `src/input/kitty.zig`
- WezTerm: `wezterm-gui/src/inputmap.rs`, `wezterm-gui/src/termwindow/keyevent.rs`, `wezterm-input-types/src/lib.rs`
- **Prior Art Section:** 3

### pty-management

**Agent A (ori_term):**
- `src/pty.rs` — PTY abstraction, ConPTY wrapper
- `src/tab.rs` — Tab (PTY lifecycle, read/write)
- `src/app.rs` — event loop integration

**Agent B (Refs):**
- Alacritty: `alacritty_terminal/src/tty/mod.rs`, `tty/windows/conpty.rs`, `tty/windows/child.rs`, `event_loop.rs`
- WezTerm: `pty/src/lib.rs`, `pty/src/unix.rs`, `pty/src/win/conpty.rs`, `pty/src/cmdbuilder.rs`
- Ghostty: `src/pty.zig`, `src/Command.zig`
- **Prior Art Section:** 4

### vte-handling

**Agent A (ori_term):**
- `src/term_handler.rs` — VTE Handler impl (~50 methods: CSI, OSC, ESC, etc.)
- `src/grid/mod.rs` — cursor movement, line feed, scroll region operations
- `src/tab.rs` — mode flags (alt screen, bracketed paste, etc.)

**Agent B (Refs):**
- Alacritty: `alacritty_terminal/src/term/mod.rs` (VTE handler, 112KB)
- WezTerm: `term/src/terminalstate/mod.rs`, `term/src/terminalstate/performer.rs`, `vtparse/src/lib.rs`
- Ghostty: `src/terminal/Terminal.zig`, `src/terminal/stream.zig`, `src/terminal/Parser.zig`
- **Prior Art Section:** 5

### selection

**Agent A (ori_term):**
- `src/selection.rs` — 3-point selection model
- `src/tab.rs` — selection integration with grid
- `src/app.rs` — mouse event dispatch for selection

**Agent B (Refs):**
- Alacritty: `alacritty_terminal/src/selection.rs`, `alacritty_terminal/src/vi_mode.rs`
- Ghostty: `src/terminal/Selection.zig`
- WezTerm: `wezterm-gui/src/selection.rs`
- **Prior Art Section:** 6

### font-rendering

**Agent A (ori_term):**
- `src/render.rs` — FontSet (fontdue, 4 style variants + fallback chain)
- `src/gpu/atlas.rs` — glyph atlas, shelf packing, cache eviction
- `src/gpu/renderer.rs` — glyph rendering in cell loop

**Agent B (Refs):**
- Alacritty: `alacritty/src/renderer/text/glyph_cache.rs`, `renderer/text/atlas.rs`, `renderer/text/builtin_font.rs`
- WezTerm: `wezterm-font/src/lib.rs`, `wezterm-font/src/shaper/harfbuzz.rs`, `wezterm-gui/src/glyphcache.rs`
- Ghostty: `src/font/Collection.zig`, `src/font/Atlas.zig`, `src/font/face.zig`, `src/font/shaper/`
- **Prior Art Section:** 7

### color-system

**Agent A (ori_term):**
- `src/palette.rs` — 270-entry color palette
- `src/cell.rs` — color fields in Cell struct
- `src/term_handler.rs` — SGR color handling

**Agent B (Refs):**
- Alacritty: `alacritty_terminal/src/term/color.rs`, `alacritty/src/display/color.rs`
- Ghostty: `src/terminal/color.zig`, `src/terminal/sgr.zig`, `src/terminal/style.zig`
- Crossterm: `src/style/types/color.rs`, `src/style/types/colored.rs`
- **Prior Art Section:** 8

### tab-architecture

**Agent A (ori_term):**
- `src/tab.rs` — Tab (Grid + PTY + VTE + mode flags)
- `src/tab_bar.rs` — tab bar rendering + hit-testing
- `src/drag.rs` — Chrome-style drag state machine
- `src/app.rs` — App owns HashMap<TabId, Tab>, window management

**Agent B (Refs):**
- WezTerm: `wezterm-gui/src/termwindow/mod.rs`, `wezterm-gui/src/tabbar.rs`
- Ghostty: `src/Surface.zig`
- **Prior Art Section:** 9

---

## Output Location

Proposals are written to: `plans/dpr_{domain}_{MMDDYYYY}.md`

Examples:
- `plans/dpr_grid-buffer_02122026.md`
- `plans/dpr_gpu-rendering_02122026.md`

---

## Orchestrator Discipline

**You are a thin coordinator.** Your jobs:

1. **Resolve** the domain from `$ARGUMENTS`
2. **Launch** Agents A and B in parallel (single message, both `run_in_background: true`)
3. **Collect** their output files when both complete
4. **Launch** Agent C with both summaries injected
5. **Report** the file path and a brief summary to the user

**Rules:**
- **NEVER read source code yourself** — Agents A and B do that
- **NEVER read prior-art-ref.md yourself** — Agent B does that
- **NEVER re-investigate agent findings** — trust their output
- **DO inject Agent A and B output into Agent C's prompt** — this is the one place where you pass content between agents
- **Keep your own context minimal** — you're a dispatcher, not an analyst
