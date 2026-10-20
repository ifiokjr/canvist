---
"@canvist/canvas": feat
canvist_canvas_wasm: feat
---

# Package `@canvist/canvas` for npm, starting from your own scene

`@canvist/canvas` is now an npm package like the other browser packages: a `package.json` with `repository`, `files`, and `exports`, a TypeScript build into `dist/`, and a tarball the release workflow checks. It is versioned with the SDK, and publishes once its npm name is bootstrapped and its trusted publisher set up.

- `mountCanvas(canvas, { scene })` starts from scene JSON, nested or flat, instead of always the demo scene. It defaults to one empty page; `demo: true` keeps the demo. Peers that start from the same scene share the same state, so merging them duplicates nothing.
- The handle gains `toJson()`, `encodeState()`, and `applyUpdate(update)`, to store a scene, start another canvas from it, and merge stored state.
- `canvist_canvas_wasm`: `App::with_scene(client, &scene)` and `App::encode_state()`, and `CanvasApp.create` takes the starting scene. `SceneJson` is re-exported from `@canvist/headless/scene`.
- `@canvist/headless`: `Registered`, `StarterKit`, and `Content` are now declared in its wasm-free type module, so a package that only needs the scene types (such as `@canvist/canvas`) type-checks without the WebAssembly built. Every entry point still exports them.
