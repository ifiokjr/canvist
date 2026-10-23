---
"canvist_core": fix
"canvist_extensions": fix
"canvist_layout": fix
"@canvist/web": fix
---

# Keep `@canvist/web` lean when it compiles only the StarterKit

`@canvist/web` compiles the StarterKit alone. It now carries no code for extensions its schema cannot reach:

- Clipboard codecs and StarterKit theme rules for extension blocks and marks compile only with their features.
- Table grids, ink drawings, and kinetic motion compile only with `canvist_layout`'s `tables`, `ink`, and `kinetic` features. Layout's hot paths read a row's grid part through accessors that are always `None` without `tables`.
- The editor builds its extension metadata the first time it is read, not on every build. The browser editor never reads it.

Across the extension layers, its bundle budget in `tools/bench/size.json` moves from 350,000 to 359,000 gzipped bytes, and its wasm budget from 340,000 to 345,000, for engine growth every host shares:

- JSON object and list values in attributes and command arguments.
- List and record argument types.
- The clipboard core's new block and mark handling.
- Extensions vetoing updates, `can()` dry runs, and editor settings.
- Text alignment and direction in layout.
