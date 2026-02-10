---
section: "09"
title: Selection & Clipboard
status: not-started
goal: Implement Windows Terminal-style text selection and clipboard with multi-click, block select, copy formatting, and smart paste
sections:
  - id: "09.1"
    title: Selection Model & Anchoring
    status: not-started
  - id: "09.2"
    title: Mouse Selection
    status: not-started
  - id: "09.3"
    title: Keyboard Selection
    status: not-started
  - id: "09.4"
    title: Word Delimiters & Boundaries
    status: not-started
  - id: "09.5"
    title: Copy Operations
    status: not-started
  - id: "09.6"
    title: Paste Operations
    status: not-started
  - id: "09.7"
    title: Selection Rendering
    status: not-started
  - id: "09.8"
    title: Completion Checklist
    status: not-started
---

# Section 09: Selection & Clipboard

**Status:** Not Started
**Goal:** Implement text selection and clipboard modeled after Windows Terminal,
which has the best selection/clipboard UX of any terminal emulator.

**Modeled after:** Windows Terminal's selection and clipboard implementation.
Key source files: `Selection.cpp`, `Clipboard.cpp`, `ControlInteractivity.cpp`,
`textBuffer/TextBuffer.cpp`.

**Current state:** No selection support. No clipboard integration. Mouse events
only handle scroll wheel and tab bar interactions.

---

## 09.1 Selection Model & Anchoring

Windows Terminal uses a 3-point selection model: anchor, pivot, and endpoint.

- [ ] Define `Selection` struct:
  ```rust
  pub struct Selection {
      pub anchor: SelectionPoint,    // initial click position (fixed)
      pub pivot: SelectionPoint,     // pivot for double-click expansion (prevents losing selected word)
      pub end: SelectionPoint,       // current drag endpoint (moves)
      pub mode: SelectionMode,
      pub active: bool,
  }
  ```
- [ ] `SelectionPoint { col: usize, row: i64, side: Side }` — row is absolute (signed, includes scrollback)
  - [ ] `Side::Left` / `Side::Right` for half-cell precision at boundaries
- [ ] `SelectionMode` enum:
  - [ ] `Character` — character-by-character (single click + drag)
  - [ ] `Word` — word selection (double-click)
  - [ ] `Line` — full line selection from left edge to right edge (triple-click)
  - [ ] `Block` — rectangular block selection (Alt+click+drag)
- [ ] Normalize selection: ensure start <= end for iteration
- [ ] Selection across scrollback: points use absolute row positions
- [ ] Selection invalidation: clear on output that affects selected region
- [ ] Multi-click detection:
  - [ ] Track last click position and timestamp
  - [ ] Use system double-click time (default ~500ms) for multi-click window
  - [ ] Click counter cycles: 1 -> 2 -> 3 -> 1 (single -> double -> triple -> reset)
  - [ ] Clicks must be in same cell position to count as multi-click

**Ref:** Windows Terminal `Selection` class (anchor/pivot/end pattern)

---

## 09.2 Mouse Selection

Windows Terminal-style mouse selection with drag threshold and mode toggling.

- [ ] **Drag threshold**: Selection only starts after cursor moves >= 1/4 cell width from
  initial click position (prevents accidental selection on slight mouse movement)
  - [ ] Track touchdown position separately from selection anchor
  - [ ] Only initiate selection once threshold exceeded

- [ ] **Single click + drag**: Character selection
  - [ ] Convert pixel position to cell coordinates (account for display_offset)
  - [ ] Clear any existing selection
  - [ ] Set anchor at click position
  - [ ] Drag extends endpoint

- [ ] **Double-click**: Word selection
  - [ ] Expand selection to word boundaries (see 09.4)
  - [ ] Set pivot to expanded word boundaries
  - [ ] Subsequent drag expands by words (pivot ensures original word stays selected)

- [ ] **Triple-click**: Line selection
  - [ ] Select entire logical line (follows wrapped lines)
  - [ ] Start at left boundary (col 0), end at right boundary
  - [ ] Handle WRAPLINE: climb up/down through wrapped rows for full logical line

- [ ] **Alt+click+drag**: Toggle selection mode
  - [ ] If in line/character mode: switch to block selection
  - [ ] If in block mode: switch to character selection

- [ ] **Shift+click**: Extend existing selection
  - [ ] If selection exists: extend endpoint to clicked position
  - [ ] If beyond anchor: include clicked cell
  - [ ] If before anchor: start from clicked position
  - [ ] Respect double-wide character boundaries

- [ ] **Auto-scroll during drag**: When dragging above/below viewport:
  - [ ] Scroll viewport in drag direction
  - [ ] Continue extending selection into scrollback

- [ ] **Double-wide character handling**:
  - [ ] Selection won't split double-wide characters
  - [ ] Automatically adjust selection endpoint to cell boundary

**Ref:** Windows Terminal `ControlInteractivity.cpp` mouse handling

---

## 09.3 Keyboard Selection

Mark mode for keyboard-driven selection (like Windows Terminal).

- [ ] Enter mark mode: configurable shortcut (e.g., Ctrl+Shift+M)
  - [ ] Show visual cursor at current position
  - [ ] Arrow keys move selection cursor (not terminal cursor)

- [ ] **Shift+Arrow keys**: Extend selection by one cell
- [ ] **Shift+Ctrl+Arrow keys**: Extend selection by word boundaries
- [ ] **Shift+Page Up/Down**: Extend selection by one screen
- [ ] **Shift+Home**: Extend to start of line (or start of input line)
- [ ] **Shift+End**: Extend to end of line (or end of input line)
- [ ] **Shift+Ctrl+Home**: Extend to start of buffer (top of scrollback)
- [ ] **Shift+Ctrl+End**: Extend to end of buffer
- [ ] **Ctrl+A**: Select all
  - [ ] If cursor is in input line: select input line
  - [ ] Otherwise: select entire buffer (visible + scrollback)
- [ ] **Escape**: Cancel selection, exit mark mode

**Ref:** Windows Terminal keyboard selection in mark mode

---

## 09.4 Word Delimiters & Boundaries

Configurable word boundary detection for double-click selection and Ctrl+arrow.

- [ ] **Default word delimiters**: `[]{}()=\,;"'-` plus space (always a delimiter)
- [ ] **Delimiter classes** (Windows Terminal approach):
  - [ ] Class 0: Regular characters (part of word)
  - [ ] Class 1: Whitespace (space, tab) — consistent word boundary
  - [ ] Class 2: Other delimiters — also word boundaries
  - [ ] Two classes allow asymmetric word navigation behavior
- [ ] `is_word_delimiter(ch: char) -> bool`
- [ ] `delimiter_class(ch: char) -> u8` (0, 1, or 2)
- [ ] **Word boundary detection** (`word_by_word_selection`):
  - [ ] Move in direction until hitting class transition (delimiter-to-non-delimiter)
  - [ ] Expand to encompass entire word including trailing delimiters
- [ ] Configurable via settings (future: Section 13)

**Ref:** Windows Terminal `Selection::WordByWordSelection`, `DelimiterClass()`

---

## 09.5 Copy Operations

Windows Terminal copies multiple clipboard formats simultaneously.

- [ ] **Copy triggers**:
  - [ ] Ctrl+Shift+C — copy selection
  - [ ] Ctrl+C — smart: copy if selection exists, send SIGINT (0x03) if not
  - [ ] Ctrl+Insert — copy selection
  - [ ] Enter — copy selection (in mark mode)
  - [ ] CopyOnSelect setting: auto-copy when mouse released after selection (does NOT clear selection)
  - [ ] Right-click: copy if selection exists (when context menu disabled)

- [ ] **Clipboard formats** (placed on clipboard simultaneously):
  - [ ] `CF_UNICODETEXT` — plain text (always)
  - [ ] `HTML Format` — HTML with inline styles (if CopyFormatting enabled)
    - [ ] Per-cell foreground/background colors
    - [ ] Font name and size
    - [ ] Bold rendering for BOLD cells
    - [ ] Underline colors
  - [ ] `Rich Text Format` — RTF with same styling (if CopyFormatting enabled)

- [ ] **Copy modifiers**:
  - [ ] Shift held during copy: collapse multi-line selection to single line
  - [ ] Alt held during copy: force HTML/RTF formatting regardless of settings

- [ ] **Text extraction from selection**:
  - [ ] Walk selected cells, concatenate characters
  - [ ] Skip WIDE_CHAR_SPACER cells (include the char cell, not the spacer)
  - [ ] Handle wrapped lines: join lines with WRAPLINE flag (no newline between them)
  - [ ] Unwrapped lines: add newline between them
  - [ ] Block selection: add newlines between rows, trim trailing spaces per row
  - [ ] Handle grapheme clusters: combine base + zerowidth chars

- [ ] Selection NOT cleared after copy (user must press Escape or click elsewhere)

**Ref:** Windows Terminal `Clipboard::CopyTextToSystemClipboard`, `TextBuffer::GenHTML/GenRTF`

---

## 09.6 Paste Operations

Windows Terminal-style paste with filtering and bracketed paste support.

- [ ] **Paste triggers**:
  - [ ] Ctrl+Shift+V — paste from clipboard
  - [ ] Ctrl+V — paste (when no VT conflict)
  - [ ] Shift+Insert — paste
  - [ ] Right-click — paste (when no selection and context menu disabled)

- [ ] **Character filtering on paste** (configurable `FilterOnPaste` setting):
  | Character | Behavior |
  |-----------|----------|
  | Tab (`\t`) | Strip (prevents tab expansion) |
  | Non-breaking space (U+00A0, U+202F) | Convert to regular space |
  | Smart quotes (U+201C, U+201D) | Convert to straight quotes (`"`) |
  | Em-dash (U+2014) | Convert to hyphen (`-`) |
  | En-dash (U+2013) | Convert to hyphen (`-`) |

- [ ] **Line ending handling**:
  - [ ] Convert Windows CRLF to CR for terminal
  - [ ] Filter duplicate `\n` if preceded by `\r` (collapse CRLF to CR)
  - [ ] Filter ESC characters when bracketed paste mode enabled

- [ ] **Bracketed paste** (XTERM DECSET 2004):
  - [ ] When enabled by application: wrap paste in `\x1b[200~` ... `\x1b[201~`
  - [ ] Allows applications to differentiate pasted text from typed text
  - [ ] Strip ESC chars from pasted content within brackets

- [ ] **Multi-line paste warning** (future UI feature):
  - [ ] Detect newlines in pasted content
  - [ ] Optionally warn user before sending multi-line paste to shell
  - [ ] Configurable: always warn, never warn, warn if > N lines

- [ ] **File drag-and-drop paste**:
  - [ ] Handle file drops onto terminal window
  - [ ] Extract file path(s)
  - [ ] Auto-quote paths containing spaces: `"C:\path with spaces\file.txt"`
  - [ ] Write path(s) to PTY as if typed

**Ref:** Windows Terminal `Clipboard::PasteTextFromClipboard`, `FilterCharacterOnPaste`

---

## 09.7 Selection Rendering

Visual highlighting of selected text.

- [ ] **Selection colors**: configurable selection foreground and background
  - [ ] Default: inverted colors (swap fg/bg of selected cells)
  - [ ] Alternative: semi-transparent overlay color
- [ ] **Render approach**:
  - [ ] During cell rendering: check if cell is within selection bounds
  - [ ] If selected: apply selection fg/bg colors instead of cell colors
  - [ ] For GPU rendering (future): selection as separate quad pass over cell backgrounds
- [ ] **Double-wide character handling**: highlight entire double-wide cell, not half
- [ ] **Selection across wrapped lines**: highlight continues across wrap boundaries
- [ ] **Block selection rendering**: only highlight cells within rectangular bounds

**Ref:** Windows Terminal Atlas Engine selection rendering, `PaintSelection()`

---

## 09.8 Completion Checklist

- [ ] Single click + drag selects text character-by-character
- [ ] Drag threshold prevents accidental selection on slight mouse movement
- [ ] Double-click selects words (configurable delimiters)
- [ ] Triple-click selects full logical lines (follows wraps)
- [ ] Alt+drag does block/rectangular selection
- [ ] Shift+click extends existing selection
- [ ] Keyboard selection with Shift+arrows, Ctrl+Shift+arrows
- [ ] Ctrl+A selects all
- [ ] Ctrl+Shift+C copies selection
- [ ] Ctrl+C smart behavior (copy if selection, SIGINT if not)
- [ ] CopyOnSelect option (auto-copy on mouse release)
- [ ] Copy places plain text + HTML + RTF on clipboard
- [ ] Ctrl+Shift+V pastes from clipboard
- [ ] Bracketed paste mode wraps pasted text
- [ ] FilterOnPaste strips/converts special characters
- [ ] File drag-and-drop auto-quotes paths with spaces
- [ ] Selection visually highlighted with configurable colors
- [ ] Wide characters selected as complete units
- [ ] Soft-wrapped lines joined correctly in copied text
- [ ] Selection across scrollback works
- [ ] Right-click: copy if selected, paste if not

**Exit Criteria:** Selection and clipboard works identically to Windows Terminal.
Users coming from Windows Terminal should feel completely at home with the
selection, copy, and paste behavior.
