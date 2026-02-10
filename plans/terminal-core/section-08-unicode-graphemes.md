---
section: "08"
title: Unicode & Graphemes
status: not-started
goal: Correct Unicode handling including grapheme clusters, width calculation, and complex scripts
sections:
  - id: "08.1"
    title: Grapheme Cluster Segmentation
    status: not-started
  - id: "08.2"
    title: Character Width
    status: not-started
  - id: "08.3"
    title: Combining Characters & ZWJ
    status: not-started
  - id: "08.4"
    title: Completion Checklist
    status: not-started
---

# Section 08: Unicode & Graphemes

**Status:** Not Started
**Goal:** Handle Unicode correctly: grapheme cluster segmentation, proper character
width calculation, combining marks, emoji with ZWJ, and CJK characters.

**Inspired by:**
- Ghostty's grapheme handling with UAX #29 segmentation and clamped width (0-2)
- Alacritty's `unicode-width` usage for character width
- WezTerm's extensive grapheme and emoji support

**Current state:** `unicode-width` crate used in `term_handler.rs` for wide char
detection (`UnicodeWidthChar::width(c) == Some(2)`). Basic wide char support with
WIDE_CHAR + WIDE_CHAR_SPACER flags. No combining mark handling, no ZWJ sequences.

---

## 08.1 Grapheme Cluster Segmentation

Determine grapheme cluster boundaries for incoming text.

- [ ] Add `unicode-segmentation` dependency for UAX #29 grapheme break detection
- [ ] When receiving input characters, detect grapheme cluster boundaries:
  - [ ] Base character: occupies a cell
  - [ ] Extending characters (combining marks): attach to previous cell
  - [ ] ZWJ sequences: potentially render as single glyph
- [ ] Handle multi-codepoint grapheme clusters:
  - [ ] Store base char in `Cell.c`
  - [ ] Store additional codepoints in `CellExtra.zerowidth`
  - [ ] Render entire grapheme cluster as one glyph (requires font shaping)
- [ ] Handle regional indicators (flag emoji): two codepoints -> one glyph
- [ ] Handle variation selectors (VS15 text, VS16 emoji presentation)

**Ref:** Ghostty `terminal/grapheme.zig`, UAX #29 specification

---

## 08.2 Character Width

Correctly determine display width of characters.

- [ ] Use `unicode-width` for basic width determination
- [ ] Width categories:
  - [ ] Width 0: combining marks, zero-width joiners, control characters
  - [ ] Width 1: ASCII, Latin, most scripts
  - [ ] Width 2: CJK ideographs, fullwidth forms, most emoji
- [ ] Handle ambiguous-width characters:
  - [ ] East Asian Ambiguous Width (UAX #11): configurable as 1 or 2
  - [ ] Default: 1 (Western convention), option for 2 (CJK convention)
- [ ] Emoji width edge cases:
  - [ ] Single emoji: width 2
  - [ ] Emoji with VS15 (text): width 1
  - [ ] Emoji with VS16 (emoji): width 2
  - [ ] ZWJ sequences: width 2 (single glyph)
- [ ] Tab width: configurable (default 8), expand to next tab stop
- [ ] Validate width against terminal expectations (clamp to 0-2)

**Ref:** Ghostty width clamping, `unicode-width` crate, UAX #11

---

## 08.3 Combining Characters & ZWJ

Handle complex character sequences.

- [ ] Combining marks (U+0300-U+036F, etc.):
  - [ ] Attach to previous cell via `Cell::push_zerowidth()`
  - [ ] Render combined glyph (base + combining marks)
  - [ ] If no previous cell, treat as spacing character
- [ ] Zero-Width Joiner (U+200D):
  - [ ] Used in emoji ZWJ sequences (family, skin tone, etc.)
  - [ ] Buffer incoming characters until grapheme cluster boundary
  - [ ] Render entire ZWJ sequence as one glyph if font supports it
  - [ ] Fallback: render as separate characters
- [ ] Backspace over grapheme clusters:
  - [ ] Backspace should erase the entire grapheme cluster, not just last codepoint
- [ ] Selection across grapheme clusters:
  - [ ] Select entire cluster, not partial codepoints

**Ref:** Ghostty grapheme handling, WezTerm `CellAttributes`

---

## 08.4 Completion Checklist

- [ ] CJK characters display at double width correctly
- [ ] Combining accents render on top of base characters
- [ ] Basic emoji render at correct width
- [ ] ZWJ emoji sequences render as single glyphs (with font support)
- [ ] Backspace erases complete grapheme clusters
- [ ] Ambiguous-width characters are configurable
- [ ] Tab stops expand to correct positions
- [ ] No off-by-one errors in cursor positioning with wide chars
- [ ] Box-drawing characters align properly at cell boundaries

**Exit Criteria:** Unicode text renders correctly. CJK, emoji, and combining marks
all display at proper widths with correct cursor positioning.
