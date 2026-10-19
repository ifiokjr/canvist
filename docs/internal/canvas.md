# canvist canvas architecture

canvist's text editor owns every pixel of a document. The canvas is the same idea one level up: it owns every pixel of an infinite surface of shapes, frames, images, vector paths, and text frames. Its model, steps, collaboration, history, plugins, and headless server are the text editor's own, applied to a tree of nodes instead of a tree of blocks. You can build a design tool like Figma on it, but the aim is wider: a foundation flexible enough to build canvases that work differently, such as a whiteboard you draw on by tilting your phone, a generative poster tool whose shapes are code, or a game level editor where your collaborators are live.

This page records the decisions and why they were made. It sits beside [the architecture](./architecture.md), and reuses its decisions wherever it can.

## Goals

1. **A sibling, not a fork.** The canvas reuses `canvist_model`'s step discipline, `canvist_collab`'s Yjs binding, the typed-extension system, and `canvist_headless`. A text frame on the canvas is a full canvist text document, edited by the real text editor.
2. **Headless all the way down.** Scene state, geometry, hit testing, selection, transform handles, snapping, and tools are Rust with no UI. A host draws whatever chrome it wants, or uses the default painter.
3. **Every pixel is measured.** Pan and zoom at 60 fps (120 fps on capable displays) with 10,000 nodes, selection hit testing in under 1 ms at 100,000 nodes, a drag step in under 0.5 ms, and a remote operation applied in under 50 µs. Each budget is a release-mode test.
4. **Collaboration first.** Multiplayer, shared undo, and server-side generation are built in, wire-compatible with Yjs peers and servers.
5. **Pay for what you use.** The model is small and has no GPU dependencies. Each renderer backend and export format is a Cargo feature.

## Prior art

| Project        | Engine                                                                                                                                                                          | Collaboration                                                                                                                                                                                                        | What we take                                                                                                                                                                                                        |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Figma**      | C++ compiled to wasm, a custom renderer on WebGL, moving to WebGPU for compute shaders and clearer error handling                                                               | Not a true CRDT. A central server orders changes, and each property of each object is a last-writer-wins register. A node stores its parent and a fractional position; the server rejects moves that create a cycle. | Per-property registers, parent-plus-fractional-index ordering, and a renderer that owns its pipeline. Figma's LiveGraph is a real-time data service for things like comments and file lists, not the document CRDT. |
| **tldraw**     | TypeScript; shapes are React components in a transformed HTML/SVG layer, with culling                                                                                           | A record store; shapes carry `parentId` and a fractional `index`; tldraw sync is server-authoritative                                                                                                                | Shapes as records with a type and props, tools as state machines, and a headless editor API apps drive.                                                                                                             |
| **Excalidraw** | Canvas2D with rough.js                                                                                                                                                          | Whole elements reconciled by `version` and `versionNonce`; fractional indices since 2024                                                                                                                             | Reconciling whole elements loses concurrent edits to different properties. We keep property granularity.                                                                                                            |
| **Penpot**     | ClojureScript rendering through the SVG DOM, now adding a Rust + Skia renderer compiled to wasm (`render-wasm`)                                                                 | Server-mediated changes                                                                                                                                                                                              | The SVG DOM stops scaling at large files; the fix was a wasm renderer that owns its pixels. We start there.                                                                                                         |
| **Rive**       | A C++ runtime with its own GPU vector renderer                                                                                                                                  | n/a                                                                                                                                                                                                                  | Interactivity as data: state machines and bindings travel with the file. Our tools and node types are data plus code for the same reason.                                                                           |
| **Graphite**   | Rust, a procedural node-graph engine, wasm in the browser, rendered with vello and SVG                                                                                          | n/a (planned)                                                                                                                                                                                                        | Non-destructive, procedural geometry fits a Rust core. Our boolean and instance nodes are derived geometry, computed rather than stored.                                                                            |
| **Vello**      | GPU compute 2D renderer on wgpu. The sparse-strips family adds `vello_cpu` (SIMD, multithreaded) and `vello_hybrid` (CPU tiling, GPU fine rasterisation, with a WebGL2 backend) | n/a                                                                                                                                                                                                                  | One scene description, three rasterisers: GPU compute where WebGPU exists, hybrid on WebGL2, CPU for servers and PNG export. Xilem shows the matching pattern of cached scene fragments appended per frame.         |

The lesson across all of them: whoever owns the renderer and the data model owns performance. The DOM (tldraw, Penpot's first engine) is quick to build and slow to scale. A central server makes conflicts easy (Figma); Yjs gives the same per-property semantics without one, so canvist keeps working peer to peer, on a relay, or offline.

## Layers

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Hosts: web (wasm + WebGPU / WebGL2 / Canvas2D) · desktop · iOS · Android ·   │
│ Flutter · RN · Lynx · server (headless)                                      │
└───────────────┬───────────────────────────────────────────▲──────────────────┘
                │ pointer, pen, touch, wheel, keys, custom    │ pixels + overlay data
┌───────────────▼───────────────────────────────────────────┴──────────────────┐
│ canvist_paint: PaintList (culled, LOD) → vello · vello_hybrid · vello_cpu ·  │
│ SVG · PNG · PDF                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ canvist_canvas: camera, scene index (world transforms, R-tree, auto-layout,  │
│ instances, booleans, text layout), hit testing, selection, handles,          │
│ snapping, guides, rulers, tools (typed extensions), text frame sessions      │
├──────────────────────────────────────┬───────────────────────────────────────┤
│ canvist_scene: SceneSchema, Scene,   │ canvist_collab (feature `scene`):     │
│ Node, Props, Step, Transaction,      │ SceneBinding ⇄ yrs; text frames reuse │
│ fractional keys, JSON                │ the block binding; one undo manager   │
├──────────────────────────────────────┴───────────────────────────────────────┤
│ canvist_model (text frames) · canvist_editor · canvist_layout · kurbo        │
└──────────────────────────────────────────────────────────────────────────────┘
```

`canvist_scene` depends on `canvist_model`, `kurbo`, `rustc-hash`, `serde_json`, and `thiserror`. Nothing below `canvist_paint` knows about GPUs.

## Decision C1: a node tree of typed records with per-property values

**What:** a `Scene` is an arena of `Node`s addressed by stable `NodeId`s (the same `client.seq` packing as `BlockId`). Each node has:

- a **type** from the `SceneSchema`: page, frame, group, rectangle, ellipse, polygon (and star), path, boolean, image, text, component, instance, or any type an extension registers;
- a **position**: its parent and a fractional key that orders it among its siblings;
- **props**: typed core properties plus extension-defined custom properties.

The core props are fields, not a hash map, because rendering and hit testing read them for thousands of nodes per frame:

| Prop                        | Meaning                                                                                                                                                     |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `name`, `visible`, `locked` | Layer-panel state                                                                                                                                           |
| `transform`                 | Affine transform relative to the parent: translation, rotation, and skew. Scale lives in `size`, as in Figma                                                |
| `size`                      | Width and height of the node's own box                                                                                                                      |
| `opacity`, `blend`          | Layer opacity and one of the 16 separable and non-separable blend modes                                                                                     |
| `fills`, `strokes`          | Ordered paints: solid, linear, radial, sweep gradients, and images                                                                                          |
| `stroke`                    | Width, alignment (inside, center, outside), caps, joins, miter limit, dashes                                                                                |
| `effects`                   | Drop shadow, inner shadow, layer blur, background blur                                                                                                      |
| `radius`                    | Per-corner radius (rectangles, frames)                                                                                                                      |
| `clip`                      | Whether a frame clips its children                                                                                                                          |
| `constraints`               | How the node responds when its parent frame is resized: left, right, left-and-right, center, scale                                                          |
| `layout`                    | A frame's auto-layout: direction, gap, padding, alignment, wrap                                                                                             |
| `layout_child`              | A child's sizing in auto-layout (fixed, fill, hug) and whether it is absolutely positioned                                                                  |
| `shape`                     | Type-specific geometry: path data and fill rule, polygon sides and star ratio, ellipse arc, boolean operation, image asset, text sizing, instance component |
| `custom`                    | Extension-defined values, keyed by name                                                                                                                     |

**Why:**

- **Per-property values are the unit of conflict.** Two people changing a rectangle's fill and its position at once should both win. Figma, tldraw, and Yjs maps all land on this.
- **Typed fields keep the hot path cheap.** A frame of 10,000 nodes reads transforms and paints with plain struct reads, the same reason `canvist_model` keeps a Rust mirror of the Yjs document.
- **Size is separate from transform.** Resizing a rotated rectangle changes its width, not a scale factor, so stroke widths and corner radii stay true and the inspector shows real numbers.
- **Each node has a `rev`.** Caches in the canvas and painter key on it, as layout caches key on block revisions.

**Derived, not stored:** a boolean node's geometry is computed from its children, an instance's subtree is computed from its component plus overrides, auto-layout positions are computed from intent, and auto-sized text frames take their height from layout. Storing derived values would make every peer write them, and their writes would conflict. Constraints are different: they apply when a person resizes a frame, so the resize emits ordinary steps for the children.

**Components and instances:** an instance stores its component's id and a set of overrides. Each override is its own prop, keyed `o:<component node id>:<prop>`, so overrides merge per property like everything else. The index expands an instance into a virtual subtree on demand and caches it by the component's subtree revision.

## Decision C2: invertible steps that map one-to-one to yrs

**What:** steps are the only way to change a scene:

| Step          | Change                                                                     | Inverse                                                   |
| ------------- | -------------------------------------------------------------------------- | --------------------------------------------------------- |
| `InsertNodes` | Insert node records, parents before children, each with its parent and key | `RemoveNodes` of the inserted roots                       |
| `RemoveNodes` | Remove nodes and their descendants                                         | `InsertNodes` with every removed record and text document |
| `Move`        | Change a node's parent and key: reparenting and reordering                 | `Move` back                                               |
| `SetProp`     | Set one property                                                           | `SetProp` with the old value                              |
| `Text`        | Apply one `canvist_model::Step` to a text frame's document                 | `Text` with the inner step's inverse                      |

Applying a step returns its inverse and the nodes it touched, and whether the tree's structure changed. A `Transaction` applies steps atomically, rolls back on error, and commits a `SceneCommit`.

**Why:** the same reasons as Decision 2 of the text editor. Undo needs no snapshots, the collaboration binding translates without diffing, and the canvas invalidates exactly the touched nodes. Because `Text` wraps the text editor's own steps, a scene transaction can create a text frame and type into it as one undo step, and a headless script can rewrite copy across a hundred frames atomically.

**Fractional keys:** siblings are ordered by `(key, id)`. Keys are base-62 strings with the property that a key always exists strictly between any two (`a0 < a0V < a1`). Inserting or moving between two siblings creates one key and touches no other node. Two peers inserting at the same place at once produce equal or interleaved keys; the id breaks the tie, so every peer agrees. When a client finds two equal neighbours, it picks a key past the run of equal keys.

## Decision C3: the Yjs shape, conflicts, and cycles

**What:** a scene is one Yjs document:

- The root is a `Y.Map` named `"nodes"`, from node id to a node `Y.Map`. The tree is stored flat; a node points at its parent.
- Each node map holds:
  - `"type"`: the type name;
  - `"pos"`: `[parent, key, clock, client, previousParent]`, one atomic value, so a reparent can never take its parent from one peer and its key from another;
  - `"props"`: a `Y.Map` from property name to value, each key a last-writer-wins register;
  - `"text"`, for text frames only: a `Y.Array` of block maps in exactly the shape `canvist_collab` already uses for documents.
- The reserved parent `"root"` holds pages.

**Why the text lives inside the node:** one Yjs document means one update stream, so y-websocket, Hocuspocus, and canvist Cloud serve a whole design file unchanged. Creating a text frame and its first paragraph is one atomic Yjs transaction, so no peer can see a frame without its text. One yrs undo manager scoped to `"nodes"` covers shapes and the text inside them, so collaborative undo of "I added a frame and typed in it" is one step.

**Concurrency rules,** all deterministic functions of the shared state, so peers converge:

- **Concurrent property edits:** the last writer in Yjs order wins, per property.
- **Delete versus edit:** a node deleted while a peer edits it stays deleted; the edit lands on the tombstone.
- **Delete versus insert:** a node whose parent is missing is an orphan. Orphans are hidden from the model but kept in the shared document, like the text binding's rejected blocks, and reappear if the parent comes back (for example when the delete is undone). `SceneBinding::orphans()` reports them.
- **Concurrent moves that form a cycle:** two peers move A into B and B into A. The scene is computed from the shared state; any cycle is broken at the node whose `pos` has the greatest `(clock, client, id)`, which falls back to its `previousParent`; if the result still cycles, its greatest member falls back to the first page. Both rules are evaluated per node from the shared state alone, so the order in which peers receive updates cannot change the result. The clock is a Lamport counter carried in `pos`. This is Kleppmann's rule (the later move loses), evaluated over state rather than an operation log, which yrs does not expose.
- **Concurrent moves of one node:** last writer wins on `pos`. Unlike the text editor's block moves, nothing is copied, so edits inside a moved node always survive. This is the main reason the scene is a flat map and not nested arrays.

## Decision C4: text frames embed a full canvist text document

**What:** a `text` node owns a `canvist_model::Document`. The scene stores documents beside the node tree, keyed by node id, and `RemoveNodes` carries them in its inverse. The canvas lays them out with `canvist_layout` at the frame's width and paints the resulting `DisplayList` inside the frame's transform.

Editing a text frame borrows the real `canvist_editor::Editor`:

1. Double-clicking a text frame opens a `TextSession`. The scene lends the frame's document to an `Editor` built with `EditorBuilder::build_with`; while it is lent, renderers and hit testing read it from the session.
2. Keys, IME, clipboard, and selection go to the editor exactly as in the text editor. Each commit is forwarded to the collaboration binding, so collaborators see characters as they are typed.
3. Closing the session returns the document and records one scene history entry, built from the session's composed undo steps. Remote changes during the session go through `Editor::apply_external`.

**Why:** the text editor's schema, extensions, marks, lists, and links all work inside a frame because it is the same editor. No API changes are needed in `canvist_editor` or `canvist_model`.

## Decision C5: plugins are the same typed extensions

**What:** a `SceneExtension` contributes any of:

- **node types** (`NodeSpec`): a name, which children it accepts, default props, custom prop schemas, and optionally a Rust geometry function, so a plugin can add a star, a connector arrow, or a shape generated from a seed;
- **tools**: state machines that turn pointer, pen, touch, keyboard, and custom input into steps and overlay data;
- **commands**: named, typed, with `can` dry runs and chains;
- **inspectors as data**: a list of sections and fields (prop, type, label, control hint, range, unit) plus a query that returns the current value for a selection, including `Mixed`. Hosts render any UI from it; agents read it to learn what can be edited;
- **export formats**: a name, options schema, and a function from a scene and node set to bytes.

Registration returns a typed handle, as in Decision 8: `let shapes = builder.add(Shapes::default())`, then `shapes.insert_rectangle(..)`. Every extension declares machine-readable metadata in the same format as text extensions, so `@canvist/canvas` infers its types: `canvas.tools.pen`, `canvas.commands.insertStar({ points: 5 })`, and `canvas.insert({ type: "star", ... })` only type-check when the star extension is registered.

**Input is open-ended:** tools receive `Custom { kind, value }` events beside pointer and key events. A host can feed device motion, MIDI, or a gamepad to a tool. Rotating your phone to sweep a brush is a tool, not a fork.

## Decision C6: rendering with one paint list and three rasterisers

**What:** the canvas produces a `PaintList` for the viewport: fills, strokes, images, glyph runs, layers (opacity, blend, clip), and effects, in world coordinates, culled to the viewport. Backends turn it into pixels or files:

| Backend         | Where                                                         | Feature |
| --------------- | ------------------------------------------------------------- | ------- |
| `vello` on wgpu | WebGPU in browsers, Metal, Vulkan, DX12 on desktop and mobile | `gpu`   |
| `vello_hybrid`  | WebGL2 in browsers without WebGPU                             | `webgl` |
| `vello_cpu`     | Servers, PNG export, Canvas2D fallback (`putImageData`)       | `cpu`   |
| SVG string      | Export, SSR, snapshot tests                                   | always  |
| PDF             | Export                                                        | `pdf`   |

**Caching:** each node's paint operations are cached by `rev` and world transform. A frame concatenates cached fragments of visible nodes and re-encodes only changed ones; the vello backend uses `Scene::append` for the same effect. During a drag only the dragged nodes re-encode.

**Culling and level of detail:** an R-tree over world bounds returns the visible nodes in `O(log n + visible)`. Below a size threshold on screen, a node draws as its bounding box in its main colour; tiny text draws as greeked line bars; blurs and shadows are skipped while the camera moves and restored when it settles. For scenes far larger than the GPU can encode per frame, the next step is a tile cache (render tiles at power-of-two zoom levels, reuse them while panning), which the benchmarks decide.

**The text `DisplayList` fits inside:** a text frame's display list becomes glyph runs, rectangles, and decorations in the paint list, transformed by the frame. The SVG renderer reuses the same structure. Text is measured and shaped with `parley` on every platform for the canvas, so a frame wraps identically in the browser, in a PNG made on the server, and in a PDF.

**Why vello:** it is pure Rust on wgpu, so one renderer covers WebGPU in wasm and native GPU APIs on every platform. Its sparse-strips siblings give a WebGL2 path and a CPU path that draw the same scenes with the same semantics, so a server thumbnail matches the canvas. Writing our own GPU vector renderer is Figma-scale work that vello already did.

## Decision C7: a headless interaction toolkit

Everything a design tool's interaction needs is geometry and state in Rust:

- **Camera:** pan, zoom about a point, momentum with exponential decay, pinch from two pointers, trackpad and wheel deltas, zoom-to-fit, screen ⇄ world transforms. Momentum is stepped by the host's frame clock, so it is deterministic in tests.
- **Scene index:** world transforms, world bounds, paint order, and an R-tree, updated incrementally from each commit's touched nodes.
- **Hit testing:** R-tree candidates, then exact tests in reverse paint order: fill containment by winding number, stroke distance within a screen-space tolerance, text frames by their layout, locked and hidden nodes skipped, and groups resolved to their top-level selectable ancestor (deep select with a modifier).
- **Selection and marquee:** a selection set; marquee intersection or containment through the R-tree.
- **Transform handles:** eight resize handles, rotation zones outside the corners, and skew on edges with a modifier, sized in screen pixels at any zoom. Drags handle aspect lock, resize from center, flipping past the anchor, and rotation snapping. The result is steps on the selected nodes, honouring constraints of their children.
- **Snapping and smart guides:** sorted edge and center lines of nearby nodes, the pixel grid, and a layout grid. Snaps return guide lines and spacing labels as data.
- **Rulers:** tick positions and labels for the current camera.
- **Input:** pointer events carry device kind (mouse, pen with pressure and tilt, touch), buttons, modifiers, and time.
- **Overlay:** selection outlines, handles, the marquee, guides, hover outlines, and the pen tool's preview, as data. Hosts draw it however they like, or pass it to the default painter.

## Decision C8: export and server rendering

`canvist_headless` gains a `scene` feature: load a scene from JSON or Yjs state, run commands and steps, and export SVG, PNG (through `vello_cpu`), and PDF. Servers make thumbnails with the same paint list the canvas draws. `@canvist/headless` exposes the same calls with types generated from extension metadata, so an agent can generate a design, check it against the inspector schema, and render it, all without a browser.

## Performance budgets

| Metric                                             | Budget                   | Today         |
| -------------------------------------------------- | ------------------------ | ------------- |
| Pan/zoom frame (paint list + encode), 10k nodes    | < 8 ms (120 fps capable) | (renderer)    |
| Selection hit test, 100k nodes                     | < 1 ms                   | (canvas)      |
| Drag-transform step (steps + index update), 1 node | < 0.5 ms                 | (canvas)      |
| Apply a remote property change                     | < 50 µs                  | 3.4 µs        |
| Encode a local property change, 10k nodes          | < 50 µs                  | 2.1 µs        |
| Set one property (model), 100k nodes               | < 2 µs                   | 0.15 µs       |
| Reorder a layer in its frame (model), 100k nodes   | < 2 µs                   | 0.61 µs       |
| Insert 10k nodes in one transaction                | < 20 ms                  | 7.1 ms        |
| Export / load 10k nodes as JSON                    | < 50 ms / < 50 ms        | 29 ms / 17 ms |
| Build the scene index from scratch, 100k nodes     | < 100 ms                 | (canvas)      |
| Marquee select over 10k nodes                      | < 2 ms                   | (canvas)      |

Each is a release-mode test in its crate's `tests/perf_budgets.rs`. The web demo adds a Playwright test that records frame times during a scripted pan and drag.

## What changes in existing crates

All additive:

- **`canvist_collab`** gains a `scene` feature with `SceneBinding`. To bind a text frame's `Y.Array` inside the scene's Yjs document, the block binding's internals split into a mirror that works on whatever array its owner passes in, and the document-level parts (update outbox, undo manager, reorder buffer). `Binding`'s public API does not change. `CollabError` gains two variants behind the feature.
- **`canvist_headless`** gains a `scene` feature.
- **`canvist_editor`, `canvist_model`, `canvist_layout`:** no changes.

## Recorded tradeoffs

- **Scene documents keep deleted content.** yrs 0.28 frees a deleted nested type even while the undo manager still points at content inside it: it only keeps what this peer's tracked transactions deleted. Undoing after a collaborator deletes a node then reads freed memory (a property test found it as a segfault). Scene documents therefore turn off yrs garbage collection, so deleted nodes stay as tombstones with their content. Documents grow with their editing history, as Yjs documents with `gc: false` do. The fix belongs upstream; once yrs keeps undo-referenced content alive, scenes can collect garbage again. A direct probe of the document `Binding`, which keeps collection on, did not reproduce it.
- **A text frame's array is looked up in every transaction.** Holding a reference to a nested yrs type across transactions can outlive the type, so the scene binding stores item ids and resolves arrays when it needs them.

## Rejected alternatives

- **Nested Yjs arrays for children,** as the text binding does. Yjs has no move, so every reparent and layer reorder would delete and copy the node, losing concurrent edits inside it. Design tools reorder constantly.
- **A separate Yjs document per text frame.** It gives lazy loading for free, but creating a frame would no longer be atomic, standard servers would need a multiplexing protocol, and undo would span several managers.
- **The DOM or SVG DOM as the renderer.** Quick to build and the reason Penpot is moving away from it.
- **Storing auto-layout results.** Every peer would write the same derived positions and their writes would conflict.
- **Our own GPU vector renderer.** vello covers the hard parts (GPU path rasterisation, compute on WebGPU, a WebGL2 path, and a matching CPU path).
