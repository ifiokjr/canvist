---
canvist: minor
canvist_core: minor
canvist_render: minor
canvist_wasm: minor
---

### Features

- **canvist_wasm**: Added `insert_text_at(offset, text)` and `delete_range(start, end)` for cursor-aware editing
- **@canvist/canvist**: New Deno/JSR/npm package with TypeScript API wrapping the WASM editor
  - `createEditor(canvasId)` — high-level editor with keyboard input, cursor, backspace, delete, Enter, arrow keys
  - `initWasm()` — explicit WASM initialization
  - Full demo page with canvas editor and contenteditable comparison
  - Playwright tests passing on Chromium, Firefox, and WebKit (Safari)

### Fixes

- **CI**: Fixed semver-checks to skip crates not yet published to crates.io
- **CI**: Added disk space cleanup step to prevent nix/devenv setup failures
- **CI**: Fixed clippy/lint to exclude wasm and test crates on native targets
- **CI**: Added Playwright CI workflow for cross-browser testing
