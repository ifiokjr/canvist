---
"canvist_core": patch
---

Remove all unwrap()/expect() calls from production code

Eliminated every `unwrap()` and `expect()` call from all three production
crates (`canvist_core`, `canvist_render`, `canvist_wasm`), replacing them with
safe alternatives:

- `Option::map_or` for conditional best-span comparisons
- `let Some(...) = ... else { return }` for early returns
- Direct indexing with safety comments where emptiness is already checked
- `unwrap_or(0.0)` for optional numeric values

This ensures no panic paths exist in production code. Test code retains
`unwrap()` for clarity since panics are the desired behavior for assertions.
