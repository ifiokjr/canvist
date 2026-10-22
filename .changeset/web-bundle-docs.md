---
canvist_core: perf
"@canvist/web": perf
---

# Drop metadata descriptions from the browser editor's module

`@canvist/web` builds its WebAssembly module with `--cfg canvist_omit_docs`, which leaves the command, query, and argument descriptions out of `canvist_core`'s metadata. Only bindings that generate types read them, and the browser module never does. With `wasm-opt --converge`, the editor ships 349.1 KB gzipped, down from 349.97 KB, under its 350 KB budget.
