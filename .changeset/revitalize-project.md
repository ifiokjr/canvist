---
canvist: minor
canvist_core: minor
canvist_render: minor
canvist_wasm: minor
---

### Features

- **canvist**: Umbrella re-export crate with `prelude` module and optional `wasm` feature
- **canvist_core**: Full document model with node tree (paragraphs, text runs), selection/cursor model, atomic edit operations with transaction batching, composable text styling with fluent builder API, Unicode-aware text layout with line breaking, and real-time collaboration via Yjs CRDTs
- **canvist_render**: Platform-agnostic `Canvas` and `Renderer` traits, `Viewport` with zoom/scroll/coordinate transforms, and `FontCache` via fontdue
- **canvist_wasm**: WebAssembly + Canvas2D rendering backend with `CanvistEditor` JS class, DOM event bridge, and accessibility shadow DOM

### Infrastructure

- Updated Rust toolchain to 1.93.1 (edition 2024)
- Modernized devenv.nix/yaml with rust-overlay, ifiokjr-nixpkgs, and comprehensive dev scripts
- Added knope.toml for changeset-based releases matching mdt patterns
- Added cargo-deny, clippy, rustfmt, and dprint configurations
- Added CI workflows (lint, test, build, security, semver-checks) and release workflow
- Added Playwright-rs integration test infrastructure
- Added mdbook documentation with architecture guide, API overview, and contributing guide
