# Test Organization Rules

## Sibling `tests.rs` Pattern

All unit tests live in dedicated sibling `tests.rs` files, not inline in source files.

### Structure

For **file modules** (`foo.rs`):
```
src/
  foo.rs          ← source, ends with `#[cfg(test)] mod tests;`
  foo/
    tests.rs      ← all tests for foo
```

For **directory modules** (`foo/mod.rs`):
```
src/
  foo/
    mod.rs        ← source, ends with `#[cfg(test)] mod tests;`
    tests.rs      ← all tests for foo
```

### Rules

1. **No inline test modules.** Never write `#[cfg(test)] mod tests { ... }` with inline test bodies. Always use `#[cfg(test)] mod tests;` (semicolon, no braces) and put tests in a separate file.

2. **One `tests.rs` per source file.** Each source file that has tests gets its own `tests.rs` sibling. Do not combine tests from multiple source files into a single test file.

3. **`#[cfg(test)] mod tests;` goes at the bottom of the source file.** After all production code, matching the file organization order in code-hygiene.md.

4. **Test files use `super::` imports.** The test file is a submodule, so it accesses the parent module's items via `super::`. Use `crate::` for items from other modules.

5. **No module wrapper in `tests.rs`.** The file IS the module. Write imports and `#[test]` functions directly at the top level of the file — no `mod tests { }` wrapper.

6. **Test helpers are local to the test file.** Helper functions like `grid_with_text()` live in the `tests.rs` file where they're used. If helpers are needed across multiple test files, extract to a shared `test_helpers` module (not yet needed).

### Import Style in Test Files

```rust
// 1. Standard library (if needed)
use std::sync::Arc;

// 2. External crates (if needed)
use vte::ansi::Color;

// 3. Parent module items via super::
use super::{Grid, SomeType};

// 4. Other crate items via crate::
use crate::cell::Cell;
use crate::index::{Column, Line};
```

### When Adding New Modules

When creating a new source file that will have tests:
1. Create the source file with production code.
2. Add `#[cfg(test)] mod tests;` at the bottom.
3. Create the sibling `tests.rs` file with the tests.
4. Verify with `cargo test`.
