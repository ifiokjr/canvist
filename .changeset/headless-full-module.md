---
"canvist_extensions": feat
"canvist_headless": feat
"@canvist/headless": feat
---

# Ship every extension in `@canvist/headless/full`, a second WebAssembly module

`@canvist/headless` compiles in TipTap's StarterKit only, so its module stays at 1,093 KB raw (448 KB gzipped), inside its 1,200 KB budget. Every other extension is in `@canvist/headless/full`, a second module with the same API, as `@canvist/headless/scene` is for canvas scenes:

```ts
import { createEngine, highlight, starterKit } from "@canvist/headless/full";

const engine = await createEngine([...starterKit, highlight]);
```

- `@canvist/headless` exports the core and StarterKit descriptors. Its `createEngine` only accepts StarterKit extensions, so passing another one is a type error. `@canvist/headless/full` also exports every other descriptor, `extras`, and `allExtensions`.
- The emoji table is data rather than code. The full module loads it the first time an engine registers `emoji`, so neither module compiles it in.
- `bench/overhead.ts` checks both modules: 1,200 KB for the StarterKit one and 1,400 KB for the full one, which comes to 1,188 KB (484 KB gzipped).
- `canvist_extensions` adds the `emoji-data` feature, which compiles the gemoji table in, and `extras`, which is `full` without it. Without `emoji-data`, `emoji::load` supplies the table at runtime, and `emoji::is_loaded` says whether one is. `canvist_headless` passes `extras` through. `full` still includes everything, so the Dart package keeps the table compiled in.
