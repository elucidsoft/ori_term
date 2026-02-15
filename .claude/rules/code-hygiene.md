---
paths:
  - "**/src/**"
---

# Code Hygiene Rules

## File Organization (top to bottom)

1. `//!` module docs
2. `mod` declarations
3. Imports (see Import Rules)
4. Type aliases
5. Type definitions (structs, enums)
6. Inherent `impl` blocks (immediately after their type)
7. Trait `impl` blocks (immediately after inherent impls)
8. Free functions
9. `#[cfg(test)] mod tests;` at bottom (tests in sibling `tests.rs` — see test-organization.md)

## Import Organization (3 groups, blank-line separated)

1. Standard library imports (`std::`, `core::`) — alphabetical
2. External crate imports (`winit`, `wgpu`, `vte`, `bitflags`, etc.) — alphabetical
3. Internal crate imports (`crate::`, `super::`, relative) — grouped by module

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose, not strict).

## Naming

**Functions** — verb-based prefixes:
- Predicates: `is_*`, `has_*`, `can_*`
- Conversions: `into_*` (consuming), `to_*` (borrowing), `as_*` (cheap ref), `from_*` (construct)
- Processing: `render_*` (GPU), `draw_*` (pixel/frame), `handle_*` (events), `encode_*` (key encoding)
- Factory: `new`, `with_*`

**Variables** — scope-scaled:
- 1 char in scopes <= 3 lines: `c`, `i`, `n`, `w`
- 2-4 chars in scopes <= 15 lines: `ch`, `col`, `row`, `buf`, `err`, `cell`, `tab`
- Descriptive (5+ chars) in larger scopes: `cursor_col`, `glyph_entry`, `scroll_offset`
- Standard abbreviations: `col`, `row`, `pos`, `len`, `ch`, `buf`, `err`, `idx`, `fg`, `bg`, `attr`

## Struct/Enum Field Ordering

1. Primary data (the core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious from the name.

## Comments

**Always**:
- `//!` module doc on every file
- `///` on all `pub` items
- Comment WHY, not WHAT
- `debug_assert!` to document preconditions (executable > prose)

**Never**:
- Decorative banners (`// ───`, `// ===`, `// ***`, `// ---`)
- Comments restating what code does
- Commented-out code
- `// TODO` without actionable context

**Section labels** in large enums/matches: plain `// Section name` without decoration.

## Derive vs Manual

- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs from derive (custom Debug output, selective fields, etc.)
- If you can't articulate WHY the manual impl differs from derive, use derive

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- No dead pub items (pub but unused outside crate)
- No dead code (functions, imports, enum variants never used)

## File Size

- **Source files (excluding `tests.rs`) must not exceed 500 lines.** If a file grows past this limit, break it up — extract related functionality into submodules. This is not a suggestion; treat it as a hard limit.
- Test files (`tests.rs`) are exempt from the 500-line limit.
- When splitting a file, follow the existing pattern: `foo.rs` becomes `foo/mod.rs` + `foo/submodule.rs` (or `foo.rs` + `foo/submodule.rs` with Rust 2018 paths).
- Common split points: separate `impl` blocks by concern, extract private helpers into a submodule, split types into their own files.

## Style

- No `#[allow(clippy)]` without `reason = "..."` (use `#[expect]` when possible)
- Functions target < 30 lines, max 50 (dispatch tables exempt)
- Consistent patterns across similar code within same file
- No dead/commented-out code
- No `println!`/`eprintln!` debugging — use `log` macros
- No `unwrap()` in library code — return `Result` or provide a default
