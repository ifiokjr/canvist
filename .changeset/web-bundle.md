---
"@canvist/web": patch
"canvist_web": patch
"canvist_layout": minor
"canvist_headless": patch
"canvist_core": patch
---

# Fit the web editor in its 350 KB budget

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.
