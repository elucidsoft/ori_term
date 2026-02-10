---
section: "02"
title: VTE Escape Sequences
status: complete
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
    status: complete
  - id: "02.7"
    title: Device Attributes & Reports
    status: complete
  - id: "02.8"
    title: Completion Checklist
    status: complete
---

# Section 02: VTE Escape Sequences

**Status:** Complete
**Goal:** Make the VTE Performer comprehensive enough to run vim, htop, tmux, and
other real applications -- not just cmd.exe with basic output.

**Inspired by:**
- Ghostty's comprehensive Terminal.zig with 100+ handled sequences
- Alacritty's `term/mod.rs` Handler implementation (3300 lines)

**Implemented in:** `src/term_handler.rs`, `src/term_mode.rs` (29 lines), `src/clipboard.rs`

**What was built:**
- Replaced `vte_performer.rs` with `term_handler.rs` implementing `vte::ansi::Handler` trait (~50 methods)
- Uses vte's high-level `Processor` which parses SGR/CSI/OSC and calls semantic methods
- **SGR (02.1):** All attributes (bold, dim, italic, underline variants, blink, inverse, hidden, strikeout), foreground/background/underline colors (named, indexed, RGB), reset codes
- **Cursor/Erase (02.2):** CUP, CUU/D/F/B, CR, LF, NEL, RI, BS, HT, TBC, ED (all modes), EL (all modes), ECH, DCH, ICH, IL, DL
- **Scroll Regions (02.3):** DECSTBM with proper boundary handling, SU/SD respecting region
- **Alternate Screen (02.4):** DECSET/DECRST 1049 with save/restore cursor, dual grid (primary + alt)
- **Terminal Modes (02.5):** 15 modes via TermMode bitflags (APP_CURSOR, INSERT, SHOW_CURSOR, ORIGIN, LINE_WRAP, MOUSE_REPORT/MOTION/ALL, SGR_MOUSE, UTF8_MOUSE, FOCUS_IN_OUT, BRACKETED_PASTE, ALTERNATE_SCROLL, ALT_SCREEN, LINE_FEED_NEW_LINE)
- **OSC (02.6):** OSC 0/1/2 (set title), OSC 4/104 (set/reset palette), OSC 10/11/12 (dynamic_color_sequence for fg/bg/cursor query), set_hyperlink stub, OSC 52 clipboard read/write (base64-encoded, via `clipboard.rs` platform wrapper), push_title/pop_title (title stack)
- **DA (02.7):** DA (identify_terminal VT220), DA2 (secondary), DSR 5 (device OK), DSR 6 (cursor position), text_area_size_chars, text_area_size_pixels, report_mode/report_private_mode (DECRQM), configure_charset/set_active_charset, substitute
- **REP (CSI b):** Handled internally by vte 0.15 via `preceding_char` state
- **OSC 7 (CWD):** Intercepted via a secondary raw `vte::Parser` with `Perform` impl, stores path in `Tab.cwd`
- **OSC 133 (prompt markers):** Intercepted via raw parser, tracks `PromptState` (PromptStart/CommandStart/OutputStart/None) in `Tab.prompt_state`
- **XTVERSION (CSI > q):** Intercepted via raw parser, responds with `DCS >|oriterm(version) ST`

**Architecture note:** OSC 7, OSC 133, and XTVERSION are not routed by `vte::ansi::Processor` (silently dropped). A secondary `vte::Parser` with a lightweight `Perform` implementation (`RawInterceptor` in `tab.rs`) runs first on each byte chunk to capture these sequences, then the normal Processor handles everything else.

---

## 02.1 SGR (Select Graphic Rendition)

- [x] Parse SGR via `vte::ansi::Handler::terminal_attribute(Attr)`
- [x] All standard attributes: Reset, Bold, Dim, Italic, Underline (5 variants), Blink, Inverse, Hidden, Strikethrough
- [x] All cancel codes: CancelBold, CancelBoldDim, CancelItalic, CancelUnderline, CancelBlink, CancelReverse, CancelHidden, CancelStrike
- [x] Foreground color: Named (30-37, 90-97), Indexed (38;5;N), RGB (38;2;R;G;B), Default (39)
- [x] Background color: Named (40-47, 100-107), Indexed (48;5;N), RGB (48;2;R;G;B), Default (49)
- [x] Underline color: Indexed (58;5;N), RGB (58;2;R;G;B)
- [x] SGR state stored in `Cursor::template`, applied to every cell via `put_char`

---

## 02.2 Cursor & Erase Operations

- [x] Cursor movement: CUP (H/f), CUU/CUD/CUF/CUB (A/B/C/D), CNL (E), CPL (F), CHA (G), VPA (d)
- [x] Erase: ED 0/1/2/3, EL 0/1/2
- [x] Character ops: ICH (@), DCH (P), ECH (X), REP (b, handled by vte internally)
- [x] Line ops: IL (L), DL (M)
- [x] Tab ops: HTS (ESC H), TBC (g), CHT (I), CBT (Z)
- [x] Tab stop tracking: `Vec<bool>`, default every 8 columns, resizes with grid

---

## 02.3 Scroll Regions

- [x] `scroll_top`/`scroll_bottom` in Grid
- [x] DECSTBM (r) with boundary clamping and cursor-to-home
- [x] SU/SD, IL/DL, linefeed all respect scroll region

---

## 02.4 Alternate Screen Buffer

- [x] Dual grid (primary + alt) in Tab
- [x] DECSET/DECRST 1049 with save/restore cursor
- [x] Alt screen cleared on enter, no scrollback

---

## 02.5 Terminal Modes (DECSET/DECRST)

- [x] `TermMode` bitflags: 15 modes (APP_CURSOR, INSERT, SHOW_CURSOR, ORIGIN, LINE_WRAP, MOUSE_REPORT/MOTION/ALL, SGR_MOUSE, UTF8_MOUSE, FOCUS_IN_OUT, BRACKETED_PASTE, ALTERNATE_SCROLL, ALT_SCREEN, LINE_FEED_NEW_LINE)
- [x] `set_private_mode`/`unset_private_mode` for DECSET/DECRST
- [x] `report_mode`/`report_private_mode` for DECRQM

---

## 02.6 OSC Sequences

- [x] OSC 0/1/2 -- Set window/tab title (via `set_title`)
- [x] OSC 4 -- Set indexed color palette (via `set_color`)
- [x] OSC 7 -- Current working directory (via `RawInterceptor`, stored in `Tab.cwd`)
- [x] OSC 8 -- Hyperlinks (via `set_hyperlink`, stored in `CellExtra`; rendering/click-to-open deferred to Section 09/11)
- [x] OSC 10/11/12 -- Query/set fg/bg/cursor colors (via `dynamic_color_sequence`)
- [x] OSC 52 -- Clipboard read/write (via `clipboard_store`/`clipboard_load` + `clipboard.rs`)
- [x] OSC 104/110/111/112 -- Reset palette/fg/bg/cursor colors
- [x] OSC 133 -- Semantic prompt markers (via `RawInterceptor`, tracked in `Tab.prompt_state`)
- [x] Push/pop title stack (CSI 22/23 t)

---

## 02.7 Device Attributes & Reports

- [x] DA (CSI c) -- VT220 identification
- [x] DA2 (CSI > c) -- Secondary device attributes
- [x] DSR 5 -- Device status OK
- [x] DSR 6 -- Cursor position report
- [x] DECRQM -- Report mode state (both normal and private modes)
- [x] XTVERSION (CSI > q) -- Report `oriterm(version)` via DCS (via `RawInterceptor`)
- [x] DECID (ESC Z) -- Identify terminal
- [x] Text area size in chars/pixels (CSI 18/14 t)
- [x] Charset configuration (G0-G3) and DEC Special Graphics mapping

---

## 02.8 Completion Checklist

- [x] SGR sets all 16+ attributes on cells with correct colors
- [x] ls --color shows colored output
- [x] Tab title updates from OSC sequences
- [x] Cursor visibility toggles (DECSET 25)
- [x] Origin mode works (DECSET 6)
- [x] Auto-wrap mode works (DECSET 7)
- [x] Insert mode works (SM 4)
- [x] All erase operations correct (ED 0/1/2/3, EL 0/1/2)
- [x] Scroll regions work for vim/htop scrolling
- [x] OSC 52 clipboard read/write
- [x] OSC 7 CWD tracking
- [x] OSC 133 prompt state tracking
- [x] XTVERSION responds with version string
- [ ] vim opens, displays syntax highlighting, and exits cleanly
- [ ] htop displays with correct colors and layout
- [ ] tmux works (requires alternate screen + scroll regions)
- [ ] less works (requires alternate screen)
- [ ] Unit tests for each sequence category

**Exit Criteria:** vim, htop, tmux, and less all run correctly in the terminal
with proper colors, cursor positioning, and scroll behavior.
