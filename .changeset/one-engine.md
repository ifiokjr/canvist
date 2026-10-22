---
website: none
---

# Keep one canvist engine in the docs and site data

The flat-text crates (`canvist`, `canvist_core`, `canvist_render`, `canvist_wasm`, `canvist_test`) and the `@canvist/canvist` package are gone, so `/docs` no longer renders their readmes. The platform data names `@canvist/web` as the web editor, and the docs, design notes, and changesets call canvist's engine by its name rather than "v1" or "v2".
