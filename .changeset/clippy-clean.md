---
"canvist_core": patch
---

Clean up all clippy warnings across the workspace

Applied `cargo clippy --fix` and manually resolved remaining warnings:

- Inlined format arguments throughout test code
- Used `is_none_or` instead of `map_or(true, ...)` for option comparisons
- Replaced `unwrap_or_else` with `unwrap_or` for constant defaults
- Fixed loop variable indexing with iterator-based patterns
- Added `#[allow]` for intentional patterns (struct bools, single-char params, doc markdown)
- Removed unused dead code attributes
- Added underscore separators to long hex literals (FNV-1a constants)
- Simplified comparisons (`i < sel_end` instead of `i + 1 <= sel_end`)
