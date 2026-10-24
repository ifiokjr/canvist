All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Package `@canvist/canvas` for npm, starting from your own scene

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #170](https://github.com/canvist/canvist/pull/170)

`@canvist/canvas` is now an npm package like the other browser packages: a `package.json` with `repository`, `files`, and `exports`, a TypeScript build into `dist/`, and a tarball the release workflow checks. It is versioned with the SDK, and publishes once its npm name is bootstrapped and its trusted publisher set up.

- `mountCanvas(canvas, { scene })` starts from scene JSON, nested or flat, instead of always the demo scene. It defaults to one empty page; `demo: true` keeps the demo. Peers that start from the same scene share the same state, so merging them duplicates nothing.
- The handle gains `toJson()`, `encodeState()`, and `applyUpdate(update)`, to store a scene, start another canvas from it, and merge stored state.
- `canvist_canvas_wasm`: `App::with_scene(client, &scene)` and `App::encode_state()`, and `CanvasApp.create` takes the starting scene. `SceneJson` is re-exported from `@canvist/headless/scene`.
- `@canvist/headless`: `Registered`, `StarterKit`, and `Content` are now declared in its wasm-free type module, so a package that only needs the scene types (such as `@canvist/canvas`) type-checks without the WebAssembly built. Every entry point still exports them.

### 🐛 Fixed

- **Publish `@canvist/canvas` to npm.** `@canvist/canvas`, the design canvas for the browser, now publishes to npm with the rest of the SDK, through npm trusted publishing. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #202](https://github.com/canvist/canvist/pull/202)

<details>
<summary><strong>📖 Documentation</strong></summary>

- **Mark the browser platforms Available.** The web, React, Vue, server-side rendering, and design canvas quick starts, the platform tables in the readme and the docs, the agent guide and skill, and the website's platform list now say "Available". The package readmes describe read-only editors, placeholders, menus, collaboration, server rendering, and the bundle budget. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/canvist/canvist/pull/173)

</details>
