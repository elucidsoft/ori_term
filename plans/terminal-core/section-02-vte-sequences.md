---
section: "02"
title: VTE Escape Sequences
status: in-progress
goal: Expand VTE Performer to handle all commonly-used escape sequences for a functional terminal
sections:
  - id: "02.1"
    title: SGR (Select Graphic Rendition)
    status: complete
  - id: "02.2"
    title: Cursor & Erase Operations
    status: complete
  - id: "02.3"
    title: Scroll Regions
    status: complete
  - id: "02.4"
    title: Alternate Screen Buffer
    status: complete
  - id: "02.5"
    title: Terminal Modes (DECSET/DECRST)
    status: complete
  - id: "02.6"
    title: OSC Sequences
    status: mostly-complete
  - id: "02.7"
    title: Device Attributes & Reports
    status: mostly-complete
  - id: "02.8"
    title: Completion Checklist
    status: in-progress
---

# Section 02: VTE Escape Sequences

**Status:** Mostly Complete (02.1–02.5 complete, 02.6/02.7 mostly complete)
**Goal:** Make the VTE Performer comprehensive enough to run vim, htop, tmux, and
other real applications -- not just cmd.exe with basic output.

**Inspired by:**
- Ghostty's comprehensive Terminal.zig with 100+ handled sequences
- Alacritty's `term/mod.rs` Handler implementation (3300 lines)

**Implemented in:** `src/term_handler.rs`, `src/term_mode.rs` (29 lines)

**What was built:**
- Replaced `vte_performer.rs` with `term_handler.rs` implementing `vte::ansi::Handler` trait (~50 methods)
- Uses vte's high-level `Processor` which parses SGR/CSI/OSC and calls semantic methods
- **SGR (02.1):** All attributes (bold, dim, italic, underline variants, blink, inverse, hidden, strikeout), foreground/background/underline colors (named, indexed, RGB), reset codes
- **Cursor/Erase (02.2):** CUP, CUU/D/F/B, CR, LF, NEL, RI, BS, HT, TBC, ED (all modes), EL (all modes), ECH, DCH, ICH, IL, DL
- **Scroll Regions (02.3):** DECSTBM with proper boundary handling, SU/SD respecting region
- **Alternate Screen (02.4):** DECSET/DECRST 1049 with save/restore cursor, dual grid (primary + alt)
- **Terminal Modes (02.5):** 15 modes via TermMode bitflags (APP_CURSOR, INSERT, SHOW_CURSOR, ORIGIN, LINE_WRAP, MOUSE_REPORT/MOTION/ALL, SGR_MOUSE, UTF8_MOUSE, FOCUS_IN_OUT, BRACKETED_PASTE, ALTERNATE_SCROLL, ALT_SCREEN, LINE_FEED_NEW_LINE)
- **OSC (02.6):** OSC 0/1/2 (set title), OSC 4/104 (set/reset palette), OSC 10/11/12 (dynamic_color_sequence for fg/bg/cursor query), set_hyperlink stub, clipboard_store/clipboard_load stubs, push_title/pop_title (title stack)
- **DA (02.7):** DA (identify_terminal VT220), DA2 (secondary), DSR 5 (device OK), DSR 6 (cursor position), text_area_size_chars, text_area_size_pixels, report_mode/report_private_mode (DECRQM), configure_charset/set_active_charset, substitute

**Remaining:** OSC 7 (CWD), OSC 8 (hyperlinks fully wired), OSC 52 (clipboard read/write), OSC 133 (prompt markers), XTVERSION, REP (repeat char).

---

## 02.1 SGR (Select Graphic Rendition)

The most important missing sequence. Every terminal application uses SGR.

- [ ] Parse SGR parameter list in `csi_dispatch` when final byte is `m`
- [ ] Handle standard attributes:
  - [ ] `0` -- Reset all attributes to default
  - [ ] `1` -- Bold, `2` -- Dim, `3` -- Italic, `4` -- Underline
  - [ ] `4:0` -- no underline, `4:1` -- single, `4:2` -- double, `4:3` -- curly, `4:4` -- dotted, `4:5` -- dashed
  - [ ] `5` -- Blink (store flag, render optional), `7` -- Inverse, `8` -- Hidden
  - [ ] `9` -- Strikethrough
  - [ ] `21` -- Double underline (alternative)
  - [ ] `22` -- Normal intensity (not bold, not dim)
  - [ ] `23` -- Not italic, `24` -- Not underline, `25` -- Not blink
  - [ ] `27` -- Not inverse, `28` -- Not hidden, `29` -- Not strikethrough

- [ ] Handle foreground color:
  - [ ] `30-37` -- Standard ANSI foreground (named colors 0-7)
  - [ ] `38;5;N` -- 256-color foreground (indexed)
  - [ ] `38;2;R;G;B` -- Truecolor foreground (RGB)
  - [ ] `39` -- Default foreground
  - [ ] `90-97` -- Bright ANSI foreground (named colors 8-15)

- [ ] Handle background color:
  - [ ] `40-47` -- Standard ANSI background
  - [ ] `48;5;N` -- 256-color background
  - [ ] `48;2;R;G;B` -- Truecolor background
  - [ ] `49` -- Default background
  - [ ] `100-107` -- Bright ANSI background

- [ ] Handle underline color:
  - [ ] `58;5;N` -- 256-color underline
  - [ ] `58;2;R;G;B` -- Truecolor underline
  - [ ] `59` -- Default underline color

- [ ] Store current SGR state in `Cursor::template` cell attributes
- [ ] Apply template attributes to every new cell written via `put_char`

**Ref:** Alacritty `term/mod.rs` SGR handler, Ghostty `Terminal.zig` sgr method

---

## 02.2 Cursor & Erase Operations

Complete the cursor movement and erase operations.

- [ ] Cursor movement (extend existing):
  - [ ] `CUP` (H) -- already done, verify origin mode interaction
  - [ ] `CUU/CUD/CUF/CUB` (A/B/C/D) -- already done
  - [ ] `CNL` (E) -- Cursor Next Line (down N, column 0)
  - [ ] `CPL` (F) -- Cursor Previous Line (up N, column 0)
  - [ ] `CHA` (G) -- Cursor Horizontal Absolute (column N)
  - [ ] `VPA` (d) -- Vertical Position Absolute (row N)
  - [ ] `HVP` (f) -- Horizontal Vertical Position (same as CUP)

- [ ] Erase operations (extend existing):
  - [ ] `ED 0` -- Erase from cursor to end of display
  - [ ] `ED 1` -- Erase from start of display to cursor
  - [ ] `ED 2` -- Erase entire display (already done)
  - [ ] `ED 3` -- Erase scrollback buffer (already done)
  - [ ] `EL 0` -- Erase from cursor to end of line (already done)
  - [ ] `EL 1` -- Erase from start of line to cursor
  - [ ] `EL 2` -- Erase entire line

- [ ] Character operations:
  - [ ] `ICH` (@ ) -- Insert N blank characters at cursor
  - [ ] `DCH` (P) -- Delete N characters at cursor, shift left
  - [ ] `ECH` (X) -- Erase N characters at cursor (no shift)
  - [ ] `REP` (b) -- Repeat previous character N times

- [ ] Line operations:
  - [ ] `IL` (L) -- Insert N blank lines at cursor row
  - [ ] `DL` (M) -- Delete N lines at cursor row

- [ ] Tab operations:
  - [ ] `HTS` (ESC H) -- Set tab stop at current column
  - [ ] `TBC` (g) -- Clear tab stops (mode 0: current, mode 3: all)
  - [ ] `CHT` (I) -- Cursor Forward Tabulation (N tab stops)
  - [ ] `CBT` (Z) -- Cursor Backward Tabulation (N tab stops)

- [ ] Tab stop tracking:
  - [ ] Default tab stops every 8 columns
  - [ ] `BitVec` or `Vec<bool>` for custom tab stops
  - [ ] Tab stops resize with grid

**Ref:** Alacritty handler methods (goto, insert_blank, delete_chars, etc.)

---

## 02.3 Scroll Regions

DECSTBM defines a scrolling region. Required for vim, less, htop, etc.

- [ ] Store `scroll_region: (top: usize, bottom: usize)` in Grid or a TermState
- [ ] `DECSTBM` (r) -- Set Top and Bottom Margins
  - [ ] Default: full screen (0, rows-1)
  - [ ] Clamp to valid range
  - [ ] Move cursor to home position after setting
- [ ] Scroll operations respect the region:
  - [ ] `scroll_up` only scrolls within `top..=bottom`
  - [ ] `scroll_down` only scrolls within `top..=bottom`
  - [ ] `IL` / `DL` operate within the scroll region
  - [ ] Cursor movement at region boundaries triggers scroll, not at screen boundaries

**Ref:** Ghostty `Terminal.zig` scrolling_region, Alacritty scroll region handling

---

## 02.4 Alternate Screen Buffer

Essential for full-screen apps (vim, htop, less, tmux).

- [ ] Add `alternate_grid: Grid` alongside the primary grid
- [ ] `DECSET 1049` / `DECRST 1049` -- Switch to/from alternate screen
  - [ ] On enter: save cursor, switch to alternate grid, clear alternate
  - [ ] On exit: switch back to primary grid, restore cursor
- [ ] `DECSET 47` / `DECRST 47` -- Older alternate screen (no save/restore)
- [ ] `DECSET 1047` / `DECRST 1047` -- Alternate screen (clear on enter)
- [ ] Alternate screen has no scrollback
- [ ] Rendering must use whichever grid is active

**Ref:** Alacritty `TermMode::ALT_SCREEN`, Ghostty `ScreenSet` (primary + alternate)

---

## 02.5 Terminal Modes (DECSET/DECRST)

Terminal behavior is controlled by mode flags set/reset via CSI ? N h / CSI ? N l.

- [ ] Define `TermMode` bitflags struct
  ```rust
  bitflags! {
      struct TermMode: u32 {
          SHOW_CURSOR       = 0x0000_0001;  // DECSET 25
          APP_CURSOR         = 0x0000_0002;  // DECSET 1
          APP_KEYPAD         = 0x0000_0004;  // DECKPAM/DECKPNM
          LINE_WRAP          = 0x0000_0008;  // DECSET 7 (auto-wrap)
          ORIGIN             = 0x0000_0010;  // DECSET 6
          INSERT             = 0x0000_0020;  // SM 4
          ALT_SCREEN         = 0x0000_0040;  // DECSET 1049
          MOUSE_REPORT       = 0x0000_0080;  // DECSET 1000
          MOUSE_MOTION       = 0x0000_0100;  // DECSET 1002
          MOUSE_ALL          = 0x0000_0200;  // DECSET 1003
          SGR_MOUSE          = 0x0000_0400;  // DECSET 1006
          FOCUS_IN_OUT       = 0x0000_0800;  // DECSET 1004
          BRACKETED_PASTE    = 0x0000_1000;  // DECSET 2004
          UTF8_MOUSE         = 0x0000_2000;  // DECSET 1005
          ALTERNATE_SCROLL   = 0x0000_4000;  // DECSET 1007
          LINE_FEED_NEW_LINE = 0x0000_8000;  // SM 20
      }
  }
  ```

- [ ] Implement `csi_dispatch` for `?h` (DECSET) and `?l` (DECRST)
- [ ] Default modes: `LINE_WRAP | SHOW_CURSOR`
- [ ] Mode save/restore (XTSAVE `?s` / XTRESTORE `?r`)

**Ref:** Ghostty `modes.zig` packed ModeState, Alacritty TermMode bitflags

---

## 02.6 OSC Sequences

Operating System Commands for metadata and integration.

- [ ] `osc_dispatch` handler:
  - [ ] OSC 0 / OSC 2 -- Set window/tab title
    - [ ] Update `Tab::title` from OSC payload
    - [ ] Trigger tab bar redraw
  - [ ] OSC 1 -- Set icon name (store but may not display)
  - [ ] OSC 7 -- Current working directory (store in Tab)
  - [ ] OSC 8 -- Hyperlinks (`;params;uri ST`)
    - [ ] Track current hyperlink state
    - [ ] Attach to cells via CellExtra
  - [ ] OSC 52 -- Clipboard access (read/write)
    - [ ] Respect security: only allow write by default
  - [ ] OSC 10/11 -- Query/set default fg/bg colors
  - [ ] OSC 4 -- Query/set indexed color palette
  - [ ] OSC 104 -- Reset color palette
  - [ ] OSC 133 -- Semantic prompt markers (for shell integration)

**Ref:** Ghostty OSC handling, Alacritty OSC handler, WezTerm OSC support

---

## 02.7 Device Attributes & Reports

Proper responses make applications detect terminal capabilities.

- [ ] `DA` (Primary Device Attributes, CSI c):
  - [ ] Respond: `ESC[?62;22c` (VT220 with ANSI color)
- [ ] `DA2` (Secondary Device Attributes, CSI > c):
  - [ ] Respond with ori_term identification
- [ ] `DSR 6` (Cursor Position Report) -- already done, verify
- [ ] `DSR 5` (Device Status Report) -- respond OK
- [ ] `DECRQM` (Request Mode) -- report current mode state
- [ ] `XTVERSION` (CSI > q) -- report terminal version
- [ ] `DECID` (ESC Z) -- identify terminal

**Ref:** Ghostty `device_status.zig`, Alacritty identify_terminal()

---

## 02.8 Completion Checklist

- [x] SGR sets all 16+ attributes on cells with correct colors
- [ ] vim opens, displays syntax highlighting, and exits cleanly
- [ ] htop displays with correct colors and layout
- [ ] tmux works (requires alternate screen + scroll regions)
- [ ] less works (requires alternate screen)
- [x] ls --color shows colored output
- [x] Tab title updates from OSC sequences
- [x] Cursor visibility toggles (DECSET 25)
- [x] Origin mode works (DECSET 6)
- [x] Auto-wrap mode works (DECSET 7)
- [x] Insert mode works (SM 4)
- [x] All erase operations correct (ED 0/1/2/3, EL 0/1/2)
- [x] Scroll regions work for vim/htop scrolling
- [ ] Unit tests for each sequence category

**Exit Criteria:** vim, htop, tmux, and less all run correctly in the terminal
with proper colors, cursor positioning, and scroll behavior.
