# canvist_test

> Integration test utilities for the canvist canvas editor.

## Current status

This crate currently provides shared path/workspace helpers and smoke tests for core logic.

- Browser automation is currently executed from `packages/canvist` via Deno + Playwright (including CI workflow coverage).
- `crates/canvist_test/tests/smoke.rs` contains non-browser smoke tests only.
- Playwright-driven Rust crate tests are planned, but not implemented in this crate yet.

## Test coverage in this crate

Current smoke tests validate core document behaviors without a browser:

1. Document creation and text insertion
2. Styling and selection operations
3. Deletion behavior
4. JSON serialization roundtrip
