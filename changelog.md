# Changelog

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

Grouped release for `sdk`.

### 💥 Breaking Change

#### Rename `canvist_editor` to `canvist_core`

_Packages:_ 🔴 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #192](https://github.com/canvist/canvist/pull/192)

The editing engine (editor state, commands, selection motion, keymaps, input rules, and history) is now the `canvist_core` crate, and `canvist_headless` re-exports it as `canvist_headless::canvist_core`. Through the `canvist` crate it is `canvist::core`. Nothing else about the API changed, so updating the dependency and the paths is the whole migration:

```rust
// Before
// or, through the crate apps install
use canvist::core::Editor;
// After
use canvist_core::Editor;
use canvist_editor::Editor;
use canvist_headless::canvist_core::Platform;
use canvist_headless::canvist_editor::Platform;
```

### 🚀 Feature

#### Package `@canvist/canvas` for npm, starting from your own scene

_Packages:_ 🟠 _@canvist/canvas_, 🟠 _canvist_canvas_wasm_, 🟢 _@canvist/headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #170](https://github.com/canvist/canvist/pull/170)

`@canvist/canvas` is now an npm package like the other browser packages: a `package.json` with `repository`, `files`, and `exports`, a TypeScript build into `dist/`, and a tarball the release workflow checks. It is versioned with the SDK, and publishes once its npm name is bootstrapped and its trusted publisher set up.

- `mountCanvas(canvas, { scene })` starts from scene JSON, nested or flat, instead of always the demo scene. It defaults to one empty page; `demo: true` keeps the demo. Peers that start from the same scene share the same state, so merging them duplicates nothing.
- The handle gains `toJson()`, `encodeState()`, and `applyUpdate(update)`, to store a scene, start another canvas from it, and merge stored state.
- `canvist_canvas_wasm`: `App::with_scene(client, &scene)` and `App::encode_state()`, and `CanvasApp.create` takes the starting scene. `SceneJson` is re-exported from `@canvist/headless/scene`.
- `@canvist/headless`: `Registered`, `StarterKit`, and `Content` are now declared in its wasm-free type module, so a package that only needs the scene types (such as `@canvist/canvas`) type-checks without the WebAssembly built. Every entry point still exports them.

#### Release the SDK to every registry from one release pull request

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)

Releases move from knope to monochange. One version covers every SDK package, and the release pull request publishes each one:

- The Rust crates `canvist_model`, `canvist_core`, `canvist_extensions`, `canvist_layout`, `canvist_collab`, and `canvist_headless` go to crates.io, and `@canvist/headless` goes to npm and JSR. All three registries use trusted publishing.
- The Android library goes to Maven Central as `io.github.ifiokjr:canvist-android`.
- The Swift package's `CanvistFFI.xcframework.zip`, its checksum, and `Canvist.swift` are attached to the GitHub release.
- The Dart packages are ready to reserve their pub.dev names. `canvist_flutter` now depends on `canvist: ^0.1.0` and resolves the local package through `pubspec_overrides.yaml`.

#### Add details, callouts, and math

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #175](https://github.com/canvist/canvist/pull/175)

Three block extensions, each behind its own Cargo feature and included in `full`:

- `Details` (`details`): TipTap's `details` block, holding a `detailsSummary` and a `detailsContent`, with an `open` attribute. Use `setDetails`, `unsetDetails`, `toggleDetailsOpen`, and the `detailsOpen` query. Enter in a summary opens the details and moves into the content; Backspace at the start of a summary unwraps the details, and no edit leaves a details without its summary or content. Layout draws a closed one as its summary beside a `▸` disclosure marker and lays out none of the hidden content.
- `Callout` (`callout`): Remirror's tinted box, with a `type` of `info`, `warning`, `error`, `success`, or `blank`. Use `toggleCallout`, `setCalloutType`, and the `calloutType` query. Typing `:::warning` at the start of a line makes one, and the StarterKit theme colours each type.
- `Mathematics` (`mathematics`): TipTap's `inlineMath` embed and `blockMath` block, holding LaTeX in `latex`. Use `insertInlineMath` and `insertBlockMath`, or type `$$x^2$$` and `$$$x^2$$$`. Layout draws inline LaTeX as its source and reserves a slot for block formulas, so the host can typeset them.

HTML reads and writes TipTap's markup:

- `<details>` with a `<summary>` and `<div data-type="detailsContent">`;
- `<div data-callout-type>`;
- `<span data-type="inline-math">` and `<div data-type="block-math">`, with the source in `data-latex`.

Markdown writes callouts as GitHub alerts (`> [!WARNING]`), details as GitHub's `<details>` HTML, and formulas as `$x$` and `$$` blocks, and reads all three back. A price such as `$5` stays text.

Server-rendered HTML marks a callout as a `role="note"` labelled with its type, which otherwise only its colour shows, and a block formula as `role="math"` labelled with its LaTeX.

#### Add ink drawings and kinetic type, drawn by the canvas itself

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_core_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, 🟠 _canvist_paint_, 🟠 _canvist_web_, ⚪ _canvist_canvas_, ⚪ _canvist_canvas_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

Two canvas-native extensions, each behind its own Cargo feature and included in `full`.

**`Ink` (`ink`)** adds `ink` blocks: freehand drawings in a space 1000 units wide and `height` units tall, scaled with their column.

- Strokes are `points` lists of `x, y` pairs with an optional `color` and `width`.
- Commands: `insertInk`, `addInkStroke`, `undoInkStroke`, and `clearInk`. Query: `inks`.
- Each stroke is one undo step and one Yjs update.
- HTML writes a `<figure data-type="ink" role="img" aria-label="Drawing">` with an SVG of the drawing and the strokes as JSON. Markdown writes the same HTML, and both read it back.

**`Kinetic` (`kinetic`)** adds the `kinetic` mark, whose `effect` is `wave`, `shake`, `pulse`, or `rainbow`.

- Commands: `setKinetic`, `toggleKinetic` (another effect switches to it), and `unsetKinetic`. Query: `kineticEffect`.
- HTML writes `<span data-kinetic="…">`. Markdown keeps the text.

`canvist_layout`:

- `DisplayItem::Stroke` is new, and the SVG renderer, `canvist_web`'s Canvas2D painter, and `canvist_paint` draw it with round caps and joins. A stroke holds a range of the new `DisplayList::points`, read with `DisplayList::stroke_points`, and its bounds, so no item but a text run owns memory and lists of text stay as fast to build as before.
- `LeafStyle::Ink` lays out drawings. `Motion` and `AttrBinding::Motion` set how text moves.
- `DisplayOptions::time` moves kinetic text glyph by glyph, and `DisplayList::animated` says when a later frame would differ. Without a time, text is still, as it should be for readers who prefer reduced motion.
- The HTML renderer draws ink as inline SVG in a figure named as an image, and animates kinetic text with CSS, off under `prefers-reduced-motion`.
- New default features `ink` and `kinetic` hold this code. `canvist_web` builds without them, as it does without `tables`.

`canvist_core`: `ValueType::sample` gives lists two items, so lists of pairs get valid samples.

`canvist_canvas` and `canvist_canvas_wasm` turn on `canvist_layout`'s new `ink` and `kinetic` features.

#### Add find and replace, a table of contents, comments, block moves, and clear formatting

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #178](https://github.com/canvist/canvist/pull/178)

Five document tools, each behind its own Cargo feature and included in `full`:

- **`FindAndReplace` (`findAndReplace`).** `setSearchTerm`, `setCaseSensitive`, `nextSearchResult`, `previousSearchResult`, `replace`, `replaceAll`, and the `searchResults` query.
  - The term lives in the editor's settings, outside the document and its history.
  - Matching ignores case unless asked not to.
  - `replaceAll` is one undo step.
- **`TableOfContents` (`tableOfContents`).** The `tableOfContents` query lists headings with their level, text, outline number, and the one the selection is under. `goToHeading` moves the caret to one.
- **`Comments` (`comments`).** A `comment` mark whose `threads` attribute lists thread ids, so comments can overlap.
  - Commands: `setComment` and `unsetComment`. Queries: `activeComments` and `comments`.
  - HTML writes `<span data-type="comment" data-threads="…">`.
  - The StarterKit theme tints commented text.
- **`DragHandle` (`dragHandle`).** `moveBlockUp` and `moveBlockDown` (Alt-↑ and Alt-↓), and `moveBlocksTo` for drops. They move the blocks a text or block selection covers, such as a selected horizontal rule. A block alone in its container moves its container.
- **`ClearFormatting` (`clearFormatting`).** `unsetAllMarks`, `clearNodes` (paragraphs, lifted out of quotes and lists), and `clearFormatting` on `Mod-\`.

#### Add text alignment, typography, character count, and placeholders

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_core_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, ⚪ _@canvist/web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #159](https://github.com/canvist/canvist/pull/159)

Four editing extensions beyond the StarterKit, each behind its own Cargo feature in `canvist_extensions` and part of `full`:

- `TextAlign`: a `textAlign` attribute on paragraphs and headings, with `setTextAlign`, `toggleTextAlign`, `unsetTextAlign`, a `textAlign` query, and `Mod-Shift-l`, `e`, `r`, and `j`. HTML reads and writes TipTap's `style="text-align: …"`.
- `Typography`: TipTap's 22 smart punctuation rules plus Remirror's `<->`, configurable with `without` and `with_output`. Rules that end in a digit fire on the space or punctuation after them.
- `CharacterCount`: `characters` (grapheme clusters), `words`, and `characterLimit` queries, and `setCharacterLimit`. Edits that would go past the limit don't apply.
- `Placeholder`: hint text for empty blocks, with TipTap's `showOnlyCurrent` and `includeChildren`, a `placeholder` query, and `setPlaceholder`.

`canvist_extensions::extras()` lists every extension beyond the StarterKit with its default configuration.

Supporting changes:

- `canvist_core`:
  - `Extension::update_filters` lets an extension refuse a local update after its fix-ups, like TipTap's `filterTransaction`. The update is undone, the command reports that it did not apply, and `Editor::can` answers the same way. The filters are collected when the editor is built, so an editor whose extensions have none pays nothing per update.
  - `EditorState::settings`, `EditorTransaction::set_setting`, and `commands::set_setting` keep per-editor values such as a limit or placeholder text outside the document, its history, and collaboration. Dry runs restore them.
  - Extension metadata now lists only the nodes the built schema has.
  - `InputRule::on` names the typed characters that can complete a rule whose pattern ends in a group or class, so it doesn't run on every punctuation keystroke.
  - Input rules no longer panic when their 64-byte look-back starts inside a multi-byte character, which `**bold**` after a line of `€` or CJK text could trigger.
- `canvist_layout`:
  - Lines align to a block's `textAlign` attribute: left, centre, right, or justified by widening spaces. Server-rendered HTML writes the matching `text-align`.
  - A row now rereads its `dir` and `textAlign` attributes when the block changes. Before, changing `dir` kept the old direction until the row was rebuilt.
  - `DisplayOptions::placeholder` draws placeholder text, faded, in an empty block.
- Clipboard: pasted block attributes keep only the ones the block type declares.

The `@canvist/web` bundle budget moves to 354,000 gzipped bytes, for update vetoes, editor settings, `can()` dry runs, and text alignment, which every host runs.

#### Add subscript, superscript, highlight, and TipTap's text style kit

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_core_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_headless_wasm_, 🟠 _canvist_layout_, 🟠 _canvist_model_, ⚪ _@canvist/web_, ⚪ _canvist_canvas_, ⚪ _canvist_canvas_wasm_, ⚪ _canvist_paint_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #155](https://github.com/canvist/canvist/pull/155)

canvist's first extensions beyond TipTap's StarterKit, each behind its own Cargo feature in `canvist_extensions` (`full` enables every one), with TipTap's JSON names, commands, and keys:

- `Subscript` (`Mod-,`) and `Superscript` (`Mod-.`), which exclude each other.
- `Highlight`: `==text==`, `Mod-Shift-h`, and a `color` attribute (multicolour by default; `Highlight { multicolor: false }` matches TipTap's default).
- `TextStyle` with `Color`, `BackgroundColor`, `FontFamily`, `FontSize`, and `LineHeight`, which each add one attribute to the `textStyle` mark, with set and unset commands and a query for the shared value.

Supporting changes:

- `canvist_model`: `SchemaBuilder::block_attrs` and `mark_attrs` add attributes to types other extensions register, like TipTap's global attributes. `MarkSpec::excludes_registered` excludes marks only when the schema has them. `EmbedSpec::attrs` sets an embed's defaults.
- `canvist_core`: `commands::update_mark_attrs` rewrites one mark's attributes run by run, keeping each run's others.
- `canvist_layout`: `TextStyle::baseline_shift` and `TextPatch::baseline_shift` raise or lower text like CSS `vertical-align`, in canvas layout and server-rendered HTML alike. `AttrBinding::LineHeight` reads a mark's line height. The StarterKit theme styles `subscript`, `superscript`, and `textStyle.backgroundColor` and `lineHeight`. `TextStyle` has a new public field, so code that builds one literally must set `baseline_shift`.
- Clipboard formats: HTML reads and writes `<sub>`, `<sup>`, `<mark data-color>`, and `<span style>`; Markdown writes `==text==` and GitHub's `<sub>` and `<sup>` tags and reads them back. Pasted marks keep only the attributes their type declares.
- `canvist_headless`: the `full` feature registers every extension by name in `Engine::named`, which `canvist_headless_wasm` and the Dart bridge enable. The browser editor still compiles the StarterKit only. `Engine::named` now registers names in one fixed order whatever order they are given in, so the same names always build the same schema and every binding nests and serializes marks alike.
- `@canvist/headless/full` and the Dart `canvist` package export a typed descriptor and handle for each new extension, generated from the new full metadata snapshot. `allExtensions` (TypeScript) and `Extensions.starterKit` (Dart) list them; `starterKit` is unchanged.
- Conformance fixtures can list `extensions` to add to the StarterKit, and every runner supports them.

The StarterKit theme's rules for blocks, marks, and embeds beyond the StarterKit sit behind a new `canvist_layout` feature, `theme-extensions`. `canvist_canvas`, `canvist_canvas_wasm`, and `canvist_paint` turn it on. `@canvist/web` builds without it, and its bundle budget moves to 352,000 gzipped bytes for the engine growth every host shares.

#### Ship every extension in `@canvist/headless/full`, a second WebAssembly module

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #169](https://github.com/canvist/canvist/pull/169)

`@canvist/headless` compiles in TipTap's StarterKit only, so its module stays at 1,060 KB raw (444 KB gzipped), inside its 1,200 KB budget. Every other extension is in `@canvist/headless/full`, a second module with the same API, as `@canvist/headless/scene` is for canvas scenes:

```ts
import { createEngine, highlight, starterKit } from "@canvist/headless/full";

const engine = await createEngine([...starterKit, highlight]);
```

- `@canvist/headless` exports the core and StarterKit descriptors. Its `createEngine` only accepts StarterKit extensions, so passing another one is a type error. `@canvist/headless/full` also exports every other descriptor, `extras`, and `allExtensions`.
- The emoji table is data rather than code. The full module loads it the first time an engine registers `emoji`, so neither module compiles it in.
- `bench/overhead.ts` checks both modules: 1,200 KB for the StarterKit one and 1,400 KB for the full one, which comes to 1,321 KB (551 KB gzipped).
- `canvist_extensions` adds the `emoji-data` feature, which compiles the gemoji table in, and `extras`, which is `full` without it. Without `emoji-data`, `emoji::load` supplies the table at runtime, and `emoji::is_loaded` says whether one is. `canvist_headless` passes `extras` through. `full` still includes everything, so the Dart package keeps the table compiled in.

#### Load, transform, and export canvas scenes on a server

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_headless_, 🟠 _canvist_headless_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #136](https://github.com/canvist/canvist/pull/136)

`canvist_headless`'s new `scene` feature adds `SceneEngine` and `SceneDoc`. They load canvas scenes from JSON (flat or nested) or Yjs state, run canvas commands, edit inspector fields, transform scenes in transactions with undo, and export the page or chosen nodes as SVG or PNG with the renderer the canvas draws with. With `collab`, a scene shares and merges Yjs updates. Scene failures carry the existing stable error codes.

`@canvist/headless/scene` exposes the same in TypeScript, with `SceneJson` types for every node, property, paint, effect, and shape. It is a separate WebAssembly module, so the document entry point's size is unchanged.

#### Render documents on JavaScript servers with `@canvist/headless/ssr`

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_headless_, 🟠 _canvist_headless_wasm_, 🟠 _canvist_layout_, 🟢 _@canvist/web_, 🟢 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/canvist/canvist/pull/154)

`@canvist/headless/ssr` renders a document as semantic HTML and a stylesheet laid out exactly as `@canvist/web` paints it, so a page shows the text before any JavaScript runs and the live editor hydrates from it without moving a pixel. It works in Node, Deno, Bun, and Workers, and so in Next.js server components and Nuxt server routes.

```ts
import { createRenderer } from "@canvist/headless/ssr";

const renderer = await createRenderer({
	theme: { bodyFamily: "Inter, sans-serif" },
});
const { html, css } = renderer.html({ format: "markdown", input: "# Hello" }, {
	width: 640,
});
```

- `renderer.html(content, { width })` returns `{ html, css }`, `renderer.page(content, { width })` a complete page, and `renderer.svg(content, { width })` an SVG image for previews. Content is text in a format or ProseMirror JSON, the same `Content` type `@canvist/web` takes.
- `createRenderer({ extensions, theme, fonts })` takes the browser editor's extensions and `ThemeOptions`, and font files for SVG text. `registerFont(bytes)` adds one later.
- Rendering is a third WebAssembly module, `wasm-ssr/`, so the document entry point stays small.
- `ThemeOptions` and `Content` are now exported by `@canvist/headless`, and `@canvist/web` re-exports them.
- `canvist_layout` gains a `json` feature with `StarterKitOptions::from_json`, which reads the theme options as both packages pass them. `canvist_headless`'s `ssr` feature enables it and re-exports `canvist_layout`.
- The `ssr_page` Rust example is gone: `examples/web` now renders its server-rendered pages with `@canvist/headless/ssr` on each request, including a React page rendered with `renderToString` and hydrated with `hydrateRoot`.

#### Add `@canvist/headless`, the document model for JavaScript runtimes with no DOM

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist_headless_wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #110](https://github.com/canvist/canvist/pull/110)

Add `@canvist/headless` (`packages/canvist-headless`), canvist's document model for JavaScript runtimes with no DOM, over a new `canvist_headless_wasm` crate (wasm-bindgen, no `web-sys`).

- Loads in Node, Deno, and Bun from disk, and in Workers from an imported `WebAssembly.Module`. A `build` step emits JavaScript for npm, and CI installs the packed tarball as a dependency.
- Types come from the extension metadata (`src/extensions.generated.ts`, generated from the `canvist_extensions` snapshot): an engine's documents only offer the registered extensions' commands, queries, blocks, and marks, with argument types such as heading levels `1 | … | 6`. `tests/types.check.ts` pins this with `tsc --noEmit` and `@ts-expect-error`.
- The shared conformance fixtures pass in Node and Deno (30 of 30), alongside API tests that sync with JavaScript Yjs.
- `examples/server` is a Deno server that converts Markdown to JSON over HTTP and runs a y-websocket-compatible sync endpoint, tested end to end with real Yjs clients.
- `bench/overhead.ts` records per-call overhead and the wasm size against budgets.

#### Add images, YouTube and Twitch videos, audio, and file attachments

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #169](https://github.com/canvist/canvist/pull/169)

Five media blocks, each behind its own Cargo feature (`media` enables them all, and `full` includes them). Each is an atomic block whose attributes say what to show. Layout reserves a slot for it, sized by its `height` attribute, and the host draws the picture, player, or file card there:

- `Image` (`image`): `setImage` with `src`, `alt`, `title`, `width`, and `height`. Typing `![alt](src "title")` inserts one, even with typography's curly quotes.
- `Youtube` (`youtube`) and `Twitch` (`twitch`): `setYoutubeVideo` and `setTwitchVideo`, with TipTap's attributes. As in TipTap, they refuse URLs that are not `http(s)` on YouTube's or Twitch's hosts, so a `javascript:` URL never becomes a frame.
- `Audio` (`audio`): `setAudio` with `controls`, `autoplay`, `loop`, and `muted`.
- `File` (`file`): Remirror's attachment card, with `insertFile`, `url`, `fileName`, `fileType`, and `fileSize`.

All five are data-driven by one implementation. Attributes equal to their default are left out, as everywhere else.

- HTML reads and writes TipTap's markup: `<img>`, `<audio>`, and `<iframe>` inside `<div data-youtube-video>` or `<div data-twitch-video>`, with a `title` so assistive technology can name the frame. File cards use `<div data-type="file">`. An image inside a paragraph splits it, and iframes from other sites are dropped.
- Markdown writes and reads images as `![alt](src "title")`; other media become links, a file without a name linked by its URL.
- The StarterKit theme reserves slots for video, audio, and file blocks.

Media commands type their source as a required string in TypeScript and Dart, as Rust already required; before, `setImage({ src: null })` type-checked and failed at runtime. Server-rendered image slots carry `role="img"` and the image's `alt` text as their accessible name. An empty `alt` marks a decorative image, which gets none.

#### A native rich-text editor for React Native

_Packages:_ 🟠 _@canvist/headless_, 🟠 _@canvist/react-native_, 🟠 _canvist_ffi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #193](https://github.com/canvist/canvist/pull/193)

`@canvist/react-native` puts the iOS and Android `CanvistView`s in React Native apps as a Fabric component, not a WebView, so typing, input methods, autocorrect, dictation, selection handles, VoiceOver, and TalkBack work as they do in the native editors. Commands, queries, and documents are typed from the extensions you register, with the same API as `@canvist/web`, and calls go through a synchronous TurboModule, so a query answers at once:

```tsx
const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

<CanvistView {...props} style={{ flex: 1 }} />;
editor?.commands.toggleHeading({ level: 2 });
editor?.commands.toggleItalic(); // type error: italic is not registered
```

`CanvistEditor` wraps both in one component with `onReady` and `onUpdate`. A call from JavaScript into the editor takes 6 to 47 µs at p95 on an iOS Simulator and 210 to 344 µs on an Android emulator (Release builds); keystrokes never cross into JavaScript. The example app in `examples/react-native` is a notes screen with a formatting toolbar, and its Maestro flows run on a simulator and an emulator in CI.

- `@canvist/headless/native` is the typed editor over any host that runs canvist natively: a small synchronous bridge of names and JSON, with failures rethrown as `CanvistError`s. `@canvist/lynx` will share it.
- The Swift and Android libraries gain what JavaScript hosts share: `CanvistHostView`, a `CanvistView` started from an element's first props and reporting changes and focus; `HostedEditors`, a thread-safe registry of editors by id with failures as `[canvist:<code>] <reason>`; JSON ranges (`selectionJSON()`, `select(rangeJSON:)`); `CanvistView.onFocusChange`; and `ExtensionSet(names:)`.
- `platforms/apple/Canvist.podspec` builds the Swift package with CocoaPods.
- `canvist-android` compiles for the Kotlin 2.2 language and standard library, so apps on Kotlin 2.2, React Native 0.88's version, can use it.

#### Add mentions, hashtags, and emoji on a shared suggestion system

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_collab_, 🟠 _canvist_core_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, 🟠 _canvist_model_, ⚪ _@canvist/web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #166](https://github.com/canvist/canvist/pull/166)

Three extensions, each behind its own Cargo feature and part of `full`, share one suggestion engine, like TipTap's Suggestion utility and Remirror's suggesters. Typing `@`, `#`, or `:` after a space starts a suggestion. A `…Suggestion` query reports `{ char, query }`, a command replaces the trigger with the pick and a space, and Escape or a dismiss command hides it until the trigger is typed again.

- `Mention`: TipTap's `mention` embed with `id`, `label`, and `mentionSuggestionChar`, plus `insertMention`, `dismissMentionSuggestion`, and `mentionSuggestion`. Backspace after a mention turns it back into its trigger.
- `Hashtag`: a `hashtag` mark with a `tag`, marked as you type and paste, with `insertHashtag`, `dismissHashtagSuggestion`, `hashtagSuggestion`, and a `hashtags` list query.
- `Emoji`: TipTap's `emoji` embed with `name`, plus canvist's `emoji` attribute holding the character. It covers GitHub's 1,870 gemoji, `:shortcode:` as you type, `setEmoji`, `dismissEmojiSuggestion`, `emojiSuggestion`, and `searchEmoji({ query, limit })`.

Supporting changes:

- `canvist_model`: `Value::Map` holds nested attributes. JSON objects in attributes become maps instead of `null`, and they round-trip.
- `canvist_core`: `ValueType::List` and `ValueType::Record` describe structured query answers. Bindings generate typed lists and records from them.
- `canvist_collab`: maps in attributes travel through Yjs instead of being dropped.
- `canvist_layout`: `EmbedStyle::Label` gains a `fallback` attribute, drawn when the label's attribute is missing: after the prefix, or as `:value:` without one. The StarterKit theme draws an emoji without its character as `:name:` and a mention without a label as `@id`, as every export does. Code that builds a `Label` literally must set `fallback`.
- `canvist_headless`: query and command arguments accept JSON objects.
- Clipboard: HTML reads and writes TipTap's mention and emoji spans and canvist's hashtag spans. A mention without a label reads back without one. Plain text and Markdown write what a mention or emoji shows.
- `@canvist/headless` types list and record answers. The Dart `canvist` package answers with typed records, such as `({String char, String query})`, and its queries take arguments.

The `@canvist/web` bundle budget moves to 356,000 gzipped bytes and its wasm budget to 342,000, for JSON object and list values and list and record argument types, which every host runs.

The StarterKit theme's mention rule moves behind `canvist_layout`'s `theme-extensions` feature, beside the emoji rule, so hosts that compile only the StarterKit, such as `@canvist/web`, carry neither.

The Dart package's Flutter editor views turn on `canvist_layout`'s `theme-extensions` feature, so they style the extensions the package ships.

#### Add tables

_Packages:_ 🟠 _@canvist/headless_, 🟠 _canvist-dart_, 🟠 _canvist_extensions_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, 🟠 _canvist_model_, ⚪ _@canvist/web_, ⚪ _canvist_canvas_, ⚪ _canvist_canvas_wasm_, ⚪ _canvist_paint_, ⚪ _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #177](https://github.com/canvist/canvist/pull/177)

`Table` (feature `table`, included in `full`) is TipTap's TableKit. It adds a `table` of `tableRow`s holding `tableCell`s and `tableHeader`s, each cell holding blocks, with TipTap's `colspan`, `rowspan`, and `colwidth` attributes.

- **Commands:**
  - `insertTable`;
  - `addColumnBefore`, `addColumnAfter`, `deleteColumn`;
  - `addRowBefore`, `addRowAfter`, `deleteRow`;
  - `deleteTable`, `mergeCells`, `splitCell`, `mergeOrSplit`;
  - `toggleHeaderRow`, `toggleHeaderColumn`, `toggleHeaderCell`;
  - `setCellAttribute`, `goToNextCell`, `goToPreviousCell`, `fixTables`.
- **Query:** `tableCell` returns where the selection head sits, with its spans and the table's size.
- **Selections:** a selection from one cell to another selects the rectangle of cells between them.
- **Keys:** Tab moves to the next cell and adds a row past the last. Backspace and Delete never join text across a cell's edge, and deleting a selection across cells clears them.
- **Headers:** toggling the header row or column leaves the corner cell to the other one when it is on, as TipTap does, so toggling twice changes nothing.
- **Repairs:** short rows are padded with empty cells after every structural change, including a gap left of a cell that spans down from the row above.

`canvist_model` adds `TableMap`, which places a table's cells on its grid from their spans.

Layout places tables on a grid:

- Columns take their `colwidth`, or an even share of what is left.
- Each row is as tall as its tallest cell, and spanning cells grow their last row.
- Grid lines are drawn as CSS's collapsed borders.
- Hit testing, arrow keys, incremental relayout, and splicing all follow the grid.
- A keystroke in a table of 1,000 cells relays out in about 7 µs.

The HTML renderer writes a fixed-layout `<table>` with the same column widths, and gives header cells a `scope`: `col` across the first row and `row` down the first column. The StarterKit theme draws hairlines and tints header cells.

HTML reads and writes `<table>` as TipTap does, including bare text in cells and a row that a merged cell spans entirely. Markdown writes a GitHub pipe table when the table fits one exactly, and HTML when it does not. It reads both back, with column alignment.

Grid layout sits behind a new default `canvist_layout` feature, `tables`, which `canvist_canvas`, `canvist_canvas_wasm`, and `canvist_paint` turn on. `canvist_web` builds without it, and the `@canvist/web` bundle budget moves to 357,000 gzipped bytes and its wasm budget to 343,000, for the grid hooks every host carries.

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Packages:_ 🟠 _@canvist/headless_, 🟠 _@canvist/web_, 🟠 _canvist_headless_, 🟠 _canvist_layout_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

#### A native rich-text editor for Lynx

_Packages:_ 🟠 _@canvist/lynx_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #198](https://github.com/canvist/canvist/pull/198)

`@canvist/lynx` puts the iOS and Android `CanvistView`s in Lynx apps as a `<canvist-editor>` element, with the same typed API as `@canvist/web` and `@canvist/react-native`: `CanvistEditor`, `useCanvistEditor`, and `useEditorState` in ReactLynx, and commands, queries, and documents typed from the extensions you register. Calls go through a synchronous `CanvistEditors` native module, which answers with a JSON envelope because Lynx reports native exceptions to the host rather than to JavaScript; failures still arrive as `CanvistError`s with their codes.

On iOS, the `CanvistLynx` pod registers the element and provides `CanvistLynxModule`; on Android, `CanvistLynx.behavior()` and `CanvistLynx.registerModule()` do the same. Both build on the `CanvistHostView` and `HostedEditors` that React Native uses.

`examples/lynx` is the notes screen from the React Native example in ReactLynx, with iOS and Android host apps; its Maestro flows run on a simulator and an emulator in CI. A call from JavaScript into the editor takes 50 to 400 µs at p95 on an iOS Simulator and 260 to 610 µs on an Android emulator (Release builds, averaged over batches, as Lynx's JavaScript engine has no high-resolution clock).

#### Read-only editors, placeholders, and menus that follow the selection

_Packages:_ 🟠 _@canvist/react_, 🟠 _@canvist/vue_, 🟠 _@canvist/web_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/canvist/canvist/pull/156)

`@canvist/web` gains the editor features TipTap users reach for first.

- **`editable`**: `createEditor(element, { editable: false })` and `editor.setEditable(editable)` make an editor read-only. People can still select with the pointer and navigation keys and copy, but typing, pasting, cutting, and dropping change nothing, no caret shows, and the field has `aria-readonly`. Commands and the API still change the document, as in TipTap. `editor.editable` reads it.
- **`placeholder`**: dimmed text where the first line starts while the document is empty, set in the body font and line height so it sits where typing begins, and the field's `aria-placeholder`. `editor.setPlaceholder(text)` changes it, and `editor.isEmpty` reports an empty document.
- **`@canvist/web/menus`**: `bubbleMenu(editor, element, options)` keeps an element over selected text, and `floatingMenu` beside the caret on an empty line. They follow edits, scrolling, and resizing, flip to the other side when there is no room, keep the editor's focus when pressed, and take `placement`, `offset`, `padding`, and `shouldShow`. `placeMenu(element, rect, options)` is the positioning on its own.
- **Selection geometry**: `editor.selectionRect()` is the selection's bounding box in document coordinates, `editor.toClientRect(rect)` converts to viewport coordinates, and `editor.blockText()` is the text of the block holding the selection. `caretRect()` and `selectionRect()` now lay out first when an edit is waiting for the next frame, so they describe the edited document.
- **Focus**: `editor.focused` reads focus, and `subscribe` listeners also hear about focus and `editable` changes.
- `@canvist/react` and `@canvist/vue`: `CanvistEditor`'s `editable` and `placeholder` props follow their values, and `BubbleMenu` and `FloatingMenu` components wrap the menus.

#### React and Vue bindings, and hydrating server-rendered HTML

_Packages:_ 🟠 _@canvist/react_, 🟠 _@canvist/vue_, 🟠 _@canvist/web_, ⚪ _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #141](https://github.com/canvist/canvist/pull/141)

- `@canvist/react` adds `useCanvistEditor`, `useEditorState` (through `useSyncExternalStore`), and `<CanvistEditor />`. `@canvist/vue` adds the same composables and a component that emits `ready` and `update`. Both are typed from the extensions and plugins you register.
- `editor.subscribe(listener)` in `@canvist/web` calls back after every document or selection change.
- `createEditor` hydrates server-rendered HTML: given an element holding canvist's `.cv-doc` markup, it reads the document from it, keeps it on screen until fonts settle, and swaps in the canvas in one frame, with no layout shift.
- `examples/web` shows a plain page, React, Vue, and a page rendered by the new `ssr_page` example in `canvist_headless`, with Playwright tests in every browser.

#### Fit the web editor in its 350 KB budget

_Packages:_ 🟢 _@canvist/web_, 🟢 _canvist_core_, 🟢 _canvist_headless_, 🟢 _canvist_web_, 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.

#### Collaborate over y-websocket out of the box with `@canvist/web/collab`

_Packages:_ 🟠 _@canvist/web_, 🟠 _canvist_core_, 🟠 _canvist_headless_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #165](https://github.com/canvist/canvist/pull/165)

`@canvist/web/collab` joins a browser editor to a room on any server that speaks y-websocket's protocol: y-websocket's own server, canvist's `receiveSync` servers, or canvist Cloud Sync. The editor is the Yjs peer itself, so there is no `Y.Doc` to keep in step, and JavaScript Yjs peers in the room see the same document.

```ts
import { connect, createEditor } from "@canvist/web/collab";

const editor = await createEditor(element);
const connection = connect(editor, {
	url: "wss://sync.example.com",
	room: "notes",
});
await connection.whenSynced();
```

- `createEditor` from `@canvist/web/collab` loads a second WebAssembly module (`wasm-collab/`) with Yjs inside, so editors that don't collaborate never download it. `loadCollab(source)` loads it from elsewhere.
- `connect(editor, { url, room, params, clientId, reconnect, WebSocket })` returns a `Connection` with `status`, `synced`, `clientId`, `subscribe`, `whenSynced`, and `disconnect`. It reconnects with backoff and catches up both ways.
- Undo is per person: `Mod-z`, `editor.undo()`, and the `undo` command revert only this peer's changes, grouped by word as when editing alone.
- `canvist_headless`: `join_message()` and `Engine::join_sync` join a shared document over y-sync with no state of its own; `Doc::receive_sync_changes` also returns the commit of each remote change; `Doc::history_step` undoes or redoes and returns the update; `Doc::shared_history_key` and `Doc::shared_history_command` identify History's keys and commands on a shared document, which `press` and `run_named` now answer with the shared history. `Doc::edit` joins any follow-up the shared state needed to the update it returns, and integrating a collaborator's change no longer advances the document's clock (y-websocket echoes each update to its sender, which split typing into one undo step per key).
- `canvist_core`: `UndoGrouping` is the rule `UndoHistory` groups changes by, on its own, so a shared history can group the same way.

#### Edit on the web: EditContext input, IME, clipboard, drag and drop, accessibility

_Packages:_ 🟠 _@canvist/web_, 🟠 _canvist_headless_, 🟠 _canvist_web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #140](https://github.com/canvist/canvist/pull/140)

- Text arrives through `EditContext` in Chromium, with the paragraph and caret bounds the platform's input methods need, and through a hidden textarea everywhere else. `input: "textarea"` forces the textarea, as a kill switch for `EditContext` problems.
- Compositions from input methods and dead keys show as preedit text and commit in place. Key bindings follow the platform (`Mod` is Cmd on Apple devices), detected from the browser or set with `platform`.
- Click, Shift-click, double-click, and triple-click select; dragging extends by the unit the press selected. Copy, cut, and paste carry HTML and plain text; the selection can be dragged to move or copy it, and drops from other applications land where they fall.
- A visually hidden copy of the document, the same semantic HTML the server renders, is refreshed on idle for screen readers and find-in-page.
- Every input event is painted in its own task. `editor.inputLatency()` reports input-to-paint times; the browser tests hold their 95th percentile under 8 ms and type a 1,000-key burst without dropping a key.
- `Engine::with_platform` in `canvist_headless` chooses what `Mod` means in key bindings.

#### Add editor views, server rendering, and key platforms to the Dart package

_Packages:_ 🟠 _canvist-dart_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #152](https://github.com/canvist/canvist/pull/152)

- `doc.view(measurer)` returns an `EditorView`: incremental layout of the document, a display list per frame (`frame`, which returns no display when nothing changed), keys, typing, IME composition, the text an input method edits, pointer and handle selection, clipboard formats, typed commands with wall-clock undo grouping, caret and selection geometry, and accessibility text. It shares the document, so commands run on the `Doc` and collaborators' updates reach the layout too.
- `TextMeasurer` is the host's half of layout: Rust calls it synchronously, once per relayout batch, through a `NativeCallable`. `FixedMeasurer` gives predictable layout for tests and servers. A measurer that throws fails the view call that needed it.
- `doc.toHtml(width)`, `doc.toSvg(width)`, and `doc.serverDisplay(width)` render on the server, as the editor lays out.
- `EngineBuilder.build` and `Engine.starterKit` take a `KeyPlatform`, so `Mod` is Cmd on Apple devices.
- Budgets for a keystroke through a view to its next frame (250 µs at 10,000 paragraphs, measured at about 65 µs) and an unchanged frame.

#### Report an engine's key platform and keep shared undo in the shared history

_Packages:_ 🟠 _canvist-dart_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #167](https://github.com/canvist/canvist/pull/167)

- `Engine.platform` says which platform's key bindings an engine follows.
- An `EditorView` on a shared document sends `Mod-z`, `Mod-Shift-z`, `Mod-y`, and the `undo` and `redo` commands through the shared history, so undo never reverts a collaborator's changes.
- `EditorView.caretRect` and `selectionEnds` lay out the blocks they need, so a host can scroll to a caret far outside the viewport.

#### Add `canvist`, a pure Dart package, and rebuild `canvist_flutter` on it

_Packages:_ 🟠 _canvist-dart_, 🟠 _canvist_flutter_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #113](https://github.com/canvist/canvist/pull/113)

Add `canvist`, a pure Dart package (`platforms/dart`, no Flutter) over `canvist_headless`, and rebuild `canvist_flutter` on top of it.

- The Rust adapter (`canvist_dart`, formerly `canvist_flutter`) binds `canvist_headless` through flutter_rust_bridge and is compiled by the Dart build hook, so `dart run`, `dart test`, and `dart build cli` work on macOS and Linux with the Dart SDK alone. A small loader finds the library through the Dart VM's native-assets mapping, which flutter_rust_bridge's file-name loader cannot.
- `EngineBuilder.add(Extensions.bold)` returns typed handles, generated from the extension metadata, whose methods return typed `Command`s and `Query<T>`s. Engines, documents, formats, UTF-16 positions, snapshots, `CanvistException` with shared `ErrorCode`s, and the Yjs peer and y-sync server API mirror the Rust and TypeScript packages.
- `dart test` passes the shared conformance fixtures (30 of 30), API tests, and an end-to-end test of the `dart:io` example server (Markdown conversion plus a y-websocket endpoint) with two peers over WebSockets. A new `dart` workflow runs them on Linux and macOS without Flutter.
- `canvist_flutter` now depends on and re-exports `canvist`; its example app and integration tests use the real engine instead of the placeholder handle.
- `Engine::named` registers StarterKit extensions by name, shared by the TypeScript and Dart bindings.

#### Paint ink strokes on iOS, Android, and in Flutter

_Packages:_ 🟠 _canvist-dart_, 🟠 _canvist_ffi_, 🟠 _canvist_flutter_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

Display lists now carry ink strokes past the web canvas to the native and Flutter hosts.

- `canvist_ffi` adds `DisplayItem::Stroke` with the stroke's points, width, colour, and bounds. The iOS and Android `DisplayPainter`s draw it as a polyline with round caps and joins.
- `canvist-dart` adds `ViewItemKind.stroke`, with `ViewItem.points` as `x, y` pairs and `ViewItem.lineWidth`. The item's rectangle bounds the points and caps.
- `canvist_flutter`'s `DisplayPainter` draws strokes the same way. A stroke of one point is a dot.

#### Add `canvist`, the one crate to install for canvist in Rust

_Packages:_ 🟠 _canvist-rust_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #190](https://github.com/canvist/canvist/pull/190)

`cargo add canvist` replaces adding `canvist_headless`, `canvist_extensions`, `canvist_core`, and the rest one by one. Each engine crate is a module of `canvist`, so `canvist_headless::Engine` is `canvist::headless::Engine`, and Cargo features choose the parts a build compiles.

- Default: `headless` and `starter-kit`, which is what a server, a script, or a test needs. `canvist::model`, `canvist::core`, and `canvist::extensions` are in every build.
- `extras` and `full` add every extension beyond the StarterKit; `collab`, `journal`, and `ssr` turn on the matching `canvist_headless` features and add `canvist::collab`.
- `layout` and `parley` add `canvist::layout`; `canvas`, `paint`, `png`, and `gpu` add the design canvas (`canvist::scene`, `canvist::canvas`, `canvist::paint`); `headless-scene` adds `canvist::headless::SceneEngine`.

#### Add `canvist_canvas`, the canvas's headless interaction core

_Packages:_ 🟠 _canvist_canvas_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #133](https://github.com/canvist/canvist/pull/133)

`canvist_canvas` turns pointer, wheel, and key input into scene edits, with no UI of its own. Hosts feed it input and draw the scene and its overlay.

- A camera with zoom about a point, momentum, and pinch; an R-tree spatial index; and exact hit testing by fill, stroke, and text layout.
- Selection, marquee, transform handles that keep scale out of the transform, and snapping with guides.
- Tools for selection, panning, shapes, frames, the pen, and text, registered as typed extensions alongside commands and inspector fields. Machine-readable metadata describes them for bindings and agents.
- Auto-layout, constraints, and text frames laid out with `canvist_layout`.

#### Render the canvas, and edit text frames with the real text editor

_Packages:_ 🟠 _canvist_canvas_, 🟠 _canvist_canvas_wasm_, 🟠 _canvist_core_, 🟠 _canvist_paint_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #134](https://github.com/canvist/canvist/pull/134)

`canvist_paint` (new) draws a canvas page through one `Painter` contract. Backends:

- vello on wgpu: WebGPU in browsers, and Metal, Vulkan, and DX12 natively;
- `vello_cpu`, for PNG export and the browser fallback;
- SVG, with text kept as text.

The scene walker culls to the viewport and has level of detail. Text is shaped with parley and the bundled Atkinson Hyperlegible font, so a frame wraps the same on the canvas, in a PNG, and in SVG. With 10,000 nodes on screen, encoding a GPU frame takes about 1 ms and a 1080p CPU frame about 4 ms.

`canvist_canvas` adds `TextSession` (feature `editor`). It opens a text frame in a `canvist_core::Editor`, places the caret where you click, draws the caret and selection in the overlay, and hands the document back on close. It also adds `Canvas::tool_named`.

`canvist_core` adds `Editor::into_state`, so a host that lent a document to an editor can take it back.

`canvist_canvas_wasm` (new, unpublished) runs the canvas in the browser on WebGPU with a CPU fallback, behind the `@canvist/canvas` demo.

#### Collaborate on canvas scenes with `SceneBinding`

_Packages:_ 🟠 _canvist_collab_, 🟠 _canvist_scene_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #131](https://github.com/canvist/canvist/pull/131)

A new `scene` feature adds `SceneBinding`, which mirrors `canvist_scene` steps to a Yjs document and back with the same guarantees as the document binding: exact collaborative undo of your own changes, reordering of updates that arrive early, and convergence however updates interleave.

- Each node is a `Y.Map` in one flat `"nodes"` map. Its position is an atomic `[parent, key, clock, client, previousParent]` register, and each property is its own last-writer-wins entry.
- Text frames keep their text inside the scene's Yjs document, in exactly the document binding's shape, so creating a frame with its first paragraph is one atomic change.
- Orphans, unknown types, and nodes their parent rejects are hidden but kept. When concurrent moves form a cycle, the later move loses.
- The block binding's mirror is split from its Yjs document so both bindings share it. `Binding`'s API is unchanged.

`canvist_scene` gains `Transaction::text_external`, which applies a collaborator's text change to a text frame through the frame's own document, and `Prop::fits`, which checks a property value against a node kind.

#### Add `canvist_cloud`, starting with Sync: a multi-tenant y-sync relay

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #116](https://github.com/canvist/canvist/pull/116)

- Stock `y-websocket` and Hocuspocus clients connect to `/v1/sync/{document}` and `/v1/hocuspocus`, with presence, read-only sessions, and per-document access from short-lived HS256 tokens that API keys mint.
- Documents persist to Postgres as merged Yjs updates, compact into snapshots, and leave memory once idle. Updates that write outside the `blocks` root or put bare values in it are refused.
- Debounced, signed `document.updated` webhooks; REST endpoints to list, read, write, and delete documents; health and readiness probes; and a graceful shutdown that persists every room.
- `canvist_collab::Reorder` is now public, so hosts with a bare yrs document apply out-of-order updates safely.

#### Add `canvist_collab`, the Yjs collaboration binding for `canvist_model` documents

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #89](https://github.com/canvist/canvist/pull/89)

- `Binding` translates committed steps into yrs operations (UTF-8 offsets become UTF-16) and remote yrs events into steps, with no tree diffing. A keystroke costs about 2 µs each way, independent of document size.
- A documented Yjs schema (`Y.Map` blocks, `Y.Text` with marks as formatting and embeds as values, nested `Y.Array`s) that JavaScript Yjs peers read and write directly.
- Out-of-order and duplicate updates converge: updates wait until what they build on arrives, working around yrs 0.27–0.28 integrating them early.
- Blocks that break the schema are hidden, not deleted, and reappear once valid. Concurrent moves of one block keep the copy with the smallest yrs item id.
- Shared undo and redo through a yrs `UndoManager` scoped to local changes, with explicit step grouping.
- Property tests for 2- and 4-peer convergence under reordered and duplicated delivery, a Deno test against JavaScript `yjs`, measurements of concurrent-split behaviour, and `divan` benchmarks.

#### Add syncing, presence, and a swappable history trait to `canvist_collab`

_Packages:_ 🟠 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #96](https://github.com/canvist/canvist/pull/96)

- `SyncSession` speaks the y-sync protocol (sync step 1 and 2, updates, awareness) over any transport, framed for y-websocket servers or for Hocuspocus, including its authentication step.
- `Presence` publishes the local user and selection through Yjs awareness as relative positions, in y-prosemirror's state shape, and resolves other peers' carets into `Collaborator`s.
- `History` is the trait an editor uses for undo; `Binding` implements it with a yrs `UndoManager` that only undoes local changes.
- Tests sync through a yrs relay server and through a real Hocuspocus server with a JavaScript `HocuspocusProvider`, carets included.

#### Add `canvist_headless`, the document model and editing engine with no UI

_Packages:_ 🟠 _canvist_collab_, 🟠 _canvist_core_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #108](https://github.com/canvist/canvist/pull/108)

Add `canvist_headless`, the canvist document model and editing engine with no UI, for servers, scripts, and tests.

- `Engine` registers extensions once (with typed handles) and is `Send + Sync`; each `Doc` is `Send`. Load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text, run typed or named commands against explicit selections, answer queries, paste, undo, and take `Snapshot`s.
- `Position` addresses text by block path and UTF-16 offset, the same in every language. `ErrorCode` gives every failure a stable name.
- The `collab` feature makes a `Doc` a Yjs peer and a y-sync server endpoint (`receive_sync` returns the reply and what to relay), and `validate` reports shared content the schema rejects. The `ssr` feature adds a `Renderer` for HTML and SVG through `canvist_layout` and `parley` with a bundled font. The default build has neither.
- `conformance/` holds language-neutral fixtures (input, operation script, expected output) that every binding runs; the Rust runner passes all 30.
- `canvist_core`: `Editor::apply_external` and `EditorState::apply_external` let a collaboration binding change the editor's own document, so a shared document is held once.
- `canvist_collab`: `Binding::rejected_blocks` counts shared items the schema hides, and the `sync` feature makes `Binding` and `SyncSession` `Send + Sync`.

#### Add clipboard formats to `canvist_extensions` behind the `clipboard` feature

_Packages:_ 🟢 _canvist_core_, 🟠 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #94](https://github.com/canvist/canvist/pull/94)

Add clipboard formats to `canvist_extensions` behind the `clipboard` feature (part of `starter-kit`).

- `clipboard::to_html`, `to_markdown`, and `to_plain_text` serialize a copied `Slice` in the markup TipTap renders and in CommonMark with GitHub task lists and strikethrough.
- `clipboard::from_html`, `from_markdown`, and `from_plain_text` parse pasted content back into a schema-valid slice: unknown markup degrades to text, bare paragraphs inside lists gain list items, and Google Docs' inline `font-weight`, `font-style`, and `text-decoration` styles become marks.
- The parsers are small and hand-written, adding no dependencies and no HTML5 or CommonMark engine to WebAssembly builds.
- `canvist_core`'s paste keeps the type of an open first block pasted into an empty block and keeps an explicit empty last block.

#### Add `canvist_core`, the platform-agnostic editing engine

_Packages:_ 🟠 _canvist_core_, 🟠 _canvist_model_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #90](https://github.com/canvist/canvist/pull/90)

Add `canvist_core`, the platform-agnostic editing engine, and `Transaction::allocate_id` in `canvist_model` so commands can create blocks inside a transaction.

- `EditorState` holds the document, selection, stored marks, and IME composition. Every change goes through `EditorState::change` or `EditorState::apply_steps`, which map the selection and composition, clear stale stored marks, and keep the document non-empty.
- Commands as plain functions: typing with mark inclusivity, deleting by grapheme, word, line, or document (joining blocks and lifting out of containers at block boundaries), marks, select all, block selections, and IME composition.
- Selection motion by grapheme cluster and word (UAX #29), line, and document, treating embeds and atomic blocks as single stops. Vertical motion goes through the `LineNavigator` trait, with a paragraph-based fallback for headless use.
- An outline notation for before/after command tests.

#### Add the `Editor`: history, keymaps, input and paste rules, and the `Extension` trait

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #92](https://github.com/canvist/canvist/pull/92)

- `Editor` and `EditorBuilder` assemble the schema, commands, key bindings, and rules from extensions, sorted by priority, and route every update through one place that runs extension fix-ups and records history.
- `History` is the boundary the collaboration crate replaces; `UndoHistory` groups typing (including IME composition) and deletions by time (500 ms by default), keeps other edits as their own steps, restores the selection, and limits depth.
- `Keymap` parses ProseMirror-style bindings such as `Mod-Shift-z`, with `Mod` meaning Cmd on Apple platforms and Ctrl elsewhere. `BaseKeymap` binds each platform's standard caret, deletion, Enter, select-all, and Escape keys.
- `InputRule` rewrites text as it is typed (block type, wrapping, marks, text replacement, and atomic blocks), runs only on whitespace or punctuation, and skips code blocks. Backspace right after a rule restores the typed text. `PasteRule` formats pasted text, for example into links.

#### Settle the `LineNavigator` interface that `canvist_layout` implements

_Packages:_ 🟠 _canvist_core_, 🟢 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #103](https://github.com/canvist/canvist/pull/103)

- `LineNavigator` now has `vertical(&mut self, doc, from: Caret, direction: VerticalDirection, goal_x: Option<f32>) -> Result<Option<VerticalMotion>, NavigationError>` and `line_edge(&mut self, doc, at: Caret, edge: LineEdge) -> Result<Caret, NavigationError>`. Methods take `&mut self` so layout can measure blocks lazily, and return errors instead of guessing when it cannot.
- `Caret` pairs a point with an `Affinity` (`Upstream` or `Downstream`), so the offset where a line soft-wraps can be the end of one line or the start of the next. `EditorState::affinity` tracks it: line and vertical motion set it, and every other change resets it to `Downstream`.
- `Env` holds the navigator mutably. Build one with `Env::new(now, &mut navigator)`, or `Env::headless(now)` for the `ParagraphNavigator` fallback. `Editor` methods now take `&mut Env`.
- `EditorError::Navigation` carries a navigator's error.

#### Add structural editing commands to `canvist_core`

_Packages:_ 🟠 _canvist_core_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #91](https://github.com/canvist/canvist/pull/91)

- Enter splits text blocks (a heading continues as a paragraph at its end), and Backspace and Delete join blocks across containers, adopting the rest of a joined list item or quote.
- Block types, block attributes, wrapping and lifting (such as blockquotes), and atomic blocks such as horizontal rules.
- Lists generic over list and item types: split, indent, outdent, join with the previous item, wrap, and toggle, converting between list types in place while keeping block ids.
- `Slice` for copy, cut, and paste: copying keeps partly selected containers around their children, and pasting merges open text at both ends and converts or flattens blocks the destination does not accept.
- Property tests that random command sequences keep every invariant and that every update's inverse restores the document exactly.

#### Add `canvist_extensions`, the StarterKit, with one Cargo feature per extension

_Packages:_ 🟠 _canvist_core_, 🟠 _canvist_extensions_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #93](https://github.com/canvist/canvist/pull/93)

Add `canvist_extensions`, the StarterKit, with each extension behind its own Cargo feature.

- Nodes: `Document`, `Paragraph`, `Heading`, `Blockquote`, `CodeBlock`, `HardBreak`, `HorizontalRule`, `BulletList`, `OrderedList`, `ListItem`, and `TaskList`, with TipTap's names, key bindings, and Markdown-style input rules.
- Marks: `Bold`, `Italic`, `Underline`, `Strike`, `Code`, and a non-inclusive `Link` with autolinking and linking on paste.
- Functionality: `History` keys and `TrailingNode`.
- `canvist_core` gains `UndoInputRule` (Backspace takes back an input rule before any other Backspace binding), `lift_empty_block`, and a public `wrap_in_list`, and its hot paths avoid scanning sibling lists so a keystroke costs the same at 100 or 100,000 blocks.
- A scripted-typing harness (`"## Hi⏎- one⏎two⇥"`) with JSON snapshots, property tests that random keystroke scripts keep every invariant and undo and redo exactly, and `divan` keystroke benchmarks at 100, 10,000, and 100,000 blocks.

#### Add the headless, typed extension API (architecture Decision 8)

_Packages:_ 🟠 _canvist_core_, 🟠 _canvist_extensions_, 🟠 _canvist_model_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #102](https://github.com/canvist/canvist/pull/102)

- `canvist_model`: `Document::savepoint`, `rollback_to`, and `release`. A rollback restores the content, the version counter, every block revision, and id allocation exactly, so a dry run leaves layout caches warm. Savepoints nest.
- `canvist_core`:
  - `EditorBuilder::add` returns an extension's typed handle, behind `TypedExtension` and an unforgeable `Registration`.
  - `CommandSpec` and `QuerySpec` declare named commands and queries with typed, checked arguments (`Field`, `ValueType`). `Editor::run_named`, `command_named`, and `query` serve dynamic hosts, and `Editor::metadata` and `metadata_json` describe every node, attribute, command, and query for binding generators.
  - `Editor::can` and `EditorState::dry_run` answer whether a command would apply without changing anything. `Editor::chain` runs commands all or nothing as one undo step, and `EditorState::atomic` backs it.
  - `Extension::commands` now returns `CommandSpec`s, and `queries` and `nodes` are new.
  - Input rules run only when the typed character can complete their pattern, and unanchored rules search a bounded `Lookback` (64 bytes by default, or the current word for autolinks). A keystroke at the end of a long line no longer scans 500 bytes per rule.
- `canvist_extensions`: every extension has a typed handle (`HeadingHandle::toggle(HeadingLevel::H2)`, `MarkHandle::is_active`, `LinkHandle::href`, ...), argument-checked commands, queries, and node metadata. `StarterKit::add_all` returns all the handles.

#### Add canvist_desktop, the editor for macOS, Windows, and Linux

_Packages:_ 🟠 _canvist_desktop_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #162](https://github.com/canvist/canvist/pull/162)

`canvist_desktop` (new) is a native rich-text editor component for any winit app. `DesktopEditor` lays a `canvist_headless` document out with `canvist_layout` and parley, paints it through `canvist_paint` (vello on the GPU, `vello_cpu` for PNGs), and takes input as `InputEvent`s, so tests drive it headlessly:

- keys, with `Mod` as Cmd on macOS and Ctrl elsewhere, input rules, paging, and the clipboard shortcuts;
- input methods: preedit text underlined in place, and `ime_area()` for the candidate window;
- the clipboard: HTML and plain text out, HTML preferred on paste;
- mouse and trackpad selection: click, double and triple click, Shift-click, drag, and autoscroll;
- scrolling and high-DPI, with only the visible blocks laid out and painted;
- an AccessKit tree of text runs with character positions, and screen reader actions;
- system fonts, plus `register_font` for fonts loaded from bytes;
- typed commands from the engine's handles.

The `winit` feature (default) adds `winit::EditorWindow`, which embeds the editor in any winit app (vello on wgpu, the system clipboard, IME placement, and the AccessKit adapter), and `winit::run` for a window that holds only the editor. The `collab` feature joins y-websocket and Hocuspocus rooms with `connect` and `receive_sync`.

A keystroke's frame takes about 0.14 ms at 10,000 paragraphs, under a 4 ms budget. `examples/desktop` is a small Markdown editor with a toolbar, open and save, and live collaboration.

#### A native rich-text editor for Android

_Packages:_ 🟠 _canvist_ffi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #176](https://github.com/canvist/canvist/pull/176)

`canvist-android` now ships `CanvistView`, a rich-text editor over the canvist engine. It paints on a `Canvas`, measures with `Paint`, and takes text through its own `InputConnection`, so Gboard composition, autocorrect, suggestions, and hardware keyboard shortcuts work as they do in an `EditText`. It also has selection handles with a magnifier, a floating text toolbar with formatting, TalkBack support through an `AccessibilityNodeProvider`, and scrolling with fling. Typed handles for every extension come from `StarterKit` and `Extensions`:

```kotlin
val kit = StarterKit()
val view = CanvistView(context, kit.extensions)
view.editor.load(DocFormat.MARKDOWN, "# Hello")
view.editor.run(kit.bold.toggleBold())
```

The sample app is now a full editor with a formatting toolbar and two editors collaborating.

#### A native rich-text editor for iOS

_Packages:_ 🟠 _canvist_ffi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #171](https://github.com/canvist/canvist/pull/171)

The `Canvist` Swift package now ships `CanvistView`, a native UIKit editor, and `CanvistEditor` to show it in SwiftUI.

- **Native text input.** Typing, input methods with marked text, autocorrect, and dictation go through `UITextInput`; the system draws the caret, selection handles, and loupe, and shows the edit menu, with a Format menu for the registered extensions. Hardware keyboard shortcuts run the editor's key bindings, and undo works with the system's gestures.
- **Platform text.** The document is measured with CoreText and painted with Core Graphics, follows Dark Mode and Dynamic Type, scrolls documents of any length, and keeps the caret above the keyboard.
- **VoiceOver** reads the document line by line, with a headings rotor.
- **Task checkboxes** toggle with a tap.
- The example app is a real editor with a toolbar of typed commands, and two editors collaborating over Yjs.

#### The full editor engine for iOS and Android

_Packages:_ 🟠 _canvist_ffi_, 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #157](https://github.com/canvist/canvist/pull/157)

`canvist_ffi`, the native binding behind the `Canvist` Swift package and the `canvist-android` Kotlin library, now runs the real editor instead of a placeholder string. An `Editor` owns a document with the StarterKit's commands, lays it out with `canvist_layout`, and hands the platform a display list to paint.

- **Typed commands in Swift and Kotlin.** Generated handles (`kit.bold.toggleBold()`, `kit.heading.toggleHeading(level: 2)`) build commands checked against the same metadata as Rust, TypeScript, and Dart. Queries decode to typed values.
- **Platform text.** `CoreTextMeasurer` and `PaintMeasurer` measure with each platform's own text engine, so lines wrap where UIKit and Android would wrap them.
- **Platform text input.** The document is exposed the way `UITextInput` and `InputConnection` address text, with marked text for IME composition, autocorrect replacements, and the geometry for carets, selection handles, and candidate windows.
- **Accessibility.** Blocks with their roles (headings, list items, quotes, code) and visual lines, for VoiceOver and TalkBack.
- **Collaboration.** Editors are Yjs peers, through updates or the y-sync protocol.

`canvist_headless` gains `Doc::can_undo` and `Doc::can_redo`, which answer from the shared history on a collaborative document.

#### Add the native build pipeline for iOS, Android and Flutter

_Packages:_ 🟠 _canvist_ffi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #87](https://github.com/canvist/canvist/pull/87)

- New `canvist_ffi` crate exports a placeholder `EditorHandle` (`new`, `insert_text`, `text`, `version`) through UniFFI. It is backed by a plain `String` until the editor engine replaces it.
- `platforms/apple`: `CanvistFFI.xcframework` (device + simulator arm64), generated Swift bindings in a `Canvist` Swift package, a SwiftUI sample app, and XCTest/XCUITest suites that run on an iOS Simulator.
- `platforms/android`: cargo-ndk builds for `arm64-v8a` and `x86_64`, generated Kotlin bindings in a `:canvist` Android library, a Compose sample app, and instrumented tests that run on an emulator.
- `platforms/flutter`: a `canvist_flutter` Dart package that binds the handle through flutter_rust_bridge v2 and builds Rust with Dart native-assets build hooks, plus an example app whose `integration_test` runs on both simulators.
- New devenv scripts `build:ios`, `build:android`, `test:ios`, `test:android` and `test:flutter`, a path-gated `native` CI workflow, and a native toolchain guide in the docs.

#### Add CanvistEditor, a rich-text editor widget painted from the Rust layout

_Packages:_ 🟠 _canvist_flutter_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #167](https://github.com/canvist/canvist/pull/167)

`canvist_flutter` is now an editor, not just the engine:

- `CanvistEditor` paints canvist's Rust layout with a `DisplayPainter`, measured by `ParagraphMeasurer` with Flutter's own text engine, so lines wrap where Flutter draws them. Only the viewport is laid out, and shaped text runs are cached.
- Input:
  - Text arrives through a delta-model `TextInputClient` that edits the block holding the caret. That covers IME composition, autocorrect, and dictation, and Backspace and Enter from soft keyboards join and split blocks.
  - Hardware keys run the extensions' key bindings.
  - Touch devices get selection handles and the adaptive toolbar.
  - Mouse and trackpad support drag, double click, triple click, shift-click, and a right-click menu.
  - Copy and paste keep formatting within the editor.
- Undo and redo, scrolling that keeps the caret in view, accessibility semantics (text field value, selection, and actions), dark mode, and text scaling. Assistive technology and UI automation that set the field's whole text get the smallest edit, so formatting survives.
- `CanvistController` holds the document and runs typed commands and queries from the engine's handles. It applies Yjs updates and y-sync messages, notifying listeners on every change. `CanvistController.keyPlatform` makes `Mod` Cmd on Apple devices.
- `CanvistStyle` and `CanvistFonts` choose fonts, size, and line height.
- The example app is a full editor with a formatting toolbar and a two-peer collaboration demo.
- Tests:
  - Widget tests cover text input, IME, keys, mouse and touch selection, the toolbar, scrolling, semantics, collaboration, and fast typing.
  - Golden tests compare the editor's layout and pixels with server rendering, both from the painter and from the live widget.
  - Integration tests run on the iOS Simulator and an Android emulator.
  - Patrol tests drive the example app through the device: the platform keyboard, native taps and swipes, long presses, the toolbar, two peers, and system dark mode. They also compare the editor's on-device pixels with server rendering.
  - Keystroke-to-frame budgets run on the host and on devices.

#### Export speed, counted instead of timed

_Packages:_ 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #196](https://github.com/canvist/canvist/pull/196)

The `bench / speed` check's headless export benchmarks now compare callgrind instruction counts instead of wall time. Their wall time on the Linux x86_64 runner measured where the linker placed the export hot loop: because the workspace links with LTO and one codegen unit, any code change could shift that loop to an address where the same instructions run slower, and which side won depended on the runner's CPU. Pull requests that never touched the export path moved the number between −26% and +35%, which is why its threshold had climbed to 40%.

Measured, not guessed: on one runner the divan binaries showed a revision 7% faster while a second build of the same sources showed it 13% slower, and callgrind counted the same instructions (within 0.2%) either way. The new `counts` bench suite, run under valgrind through gungraun (iai-callgrind's new name), reports exact counts that the same binary reproduces on every machine, so the `headless/export/*` threshold is gone and the 10% default applies. Wall time still gates every other suite, and the export budgets in the performance guide still bound wall time absolutely.

#### Record every commit for hosts that lay a document out

_Packages:_ 🟠 _canvist_headless_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #152](https://github.com/canvist/canvist/pull/152)

With the new `journal` feature, `Doc::record_commits` turns on a journal of every commit that changes the document: local calls, collaborators' updates, the shared undo history, and the repairs collaboration makes. `Doc::take_commits` drains it. A host keeping a layout applies each commit, so changes made outside its own `Doc::edit` calls, such as commands other code runs or updates from peers, still reach the layout.

`Renderer::display_list` returns the display list `Renderer::svg` paints, so native hosts can compare their own rendering with server rendering.

- **Return the commits a y-sync message applied.** `Doc::receive_sync_changes` handles a y-sync message as `receive_sync` does and also returns the commit of every update that changed the document, so interactive hosts lay out only what changed. _Packages:_ 🟠 _canvist_headless_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

#### Add `canvist_layout`, the layout engine

_Packages:_ 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #192](https://github.com/canvist/canvist/pull/192)

- Data-driven `Theme` rules for blocks, marks, and embeds, with a default matching TipTap's StarterKit.
- A batched `TextMeasurer` trait with a per-(font, segment) cache, a deterministic `FixedMeasurer`, and a `ParleyMeasurer` behind the `parley` feature.
- UAX #14 line breaking with CSS `pre-wrap` semantics, UAX #9 bidi, tabs, embeds, and hard breaks.
- `DocumentLayout` with container blocks, list markers, viewport virtualization with estimated heights, and scroll anchoring.
- Incremental relayout from a `Commit`. A keystroke re-breaks only the lines around the edit, even in a 100 KB paragraph, and a structural edit re-flattens only the root subtrees it touched.
- Hit testing, caret and selection rectangles, selection bounds for anchoring host UI, and a `LayoutNavigator` for vertical caret motion that implements `canvist_core::LineNavigator` behind the `editor` feature.
- A `DisplayList` with dirty-rectangle damage, plus `to_svg` and `to_html` renderers whose CSS reproduces the canvas line breaks.
- Release-mode performance budget tests and `divan` benchmarks at 100, 10k, and 100k blocks, plus long and heavily formatted paragraphs.

#### Measure with system fonts, and resync a layout without its commits

_Packages:_ 🟠 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

- `ParleyMeasurer::with_system_fonts` reads the fonts installed on the machine, for desktop hosts. Registered fonts add families without standing in for the generic ones.
- `DocumentLayout::resync` brings a layout up to date with a document it missed commits for, keeping measurements and the layouts of unchanged blocks.

#### Add `canvist_model`, the block-based document model

_Packages:_ 🟠 _canvist_model_

- `Schema` built from block, mark, and embed specs that extensions contribute.
- `Document` with stable `BlockId`s, nested containers, and `InlineText` that stores formatting as mark runs and embeds as single characters.
- Invertible `Step`s with point mapping. `Transaction` applies them atomically with rollback, and `Commit` records which blocks changed for incremental layout.
- ProseMirror/TipTap-compatible JSON import and export.
- Property tests for invariants and exact undo, a 20,000-keystroke typing stress test, and `divan` benchmarks covering 100 to 100,000 blocks.

#### Paint any display list, and shape with the system's fonts

_Packages:_ 🟠 _canvist_paint_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

- `paint_display_list` paints a `canvist_layout` display list through any `Painter`, with a `DisplayStyle` transform and optional fill. Canvas text frames now paint through it too, and `GREEK_PIXELS` moves with it.
- `Shaper::with_system_fonts` reads the fonts installed on the machine, for desktop hosts.
- `ShapedRun` gains `coords`, `embolden`, and `skew`, with `embolden_amount()` and `shear()`.

#### Add `canvist_scene`, the canvas's document model

_Packages:_ 🟠 _canvist_scene_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/canvist/canvist/pull/128)

`canvist_scene` models a canvas as a tree of typed nodes (pages, frames, groups, rectangles, ellipses, polygons and stars, vector paths, booleans, images, text frames, components, and instances), each with typed properties, ordered by fractional keys.

- Every change is one of five invertible steps (`InsertNodes`, `RemoveNodes`, `Move`, `SetProp`, `Text`), grouped in transactions that roll back on drop and invert exactly for undo.
- A text frame holds a full `canvist_model` document, so the text editor works inside it.
- Scenes read and write JSON in a canonical flat form and in a nested form, with optional ids and keys, that people and agents write.

### 📝 Changed

- **Keep descriptions out of the generated extension descriptors.** The descriptors in `@canvist/headless/extensions` no longer carry each command, query, and field's `doc` as data: it is a comment above each entry in the generated file. Types never read it, and as data it shipped in every browser bundle (627 bytes gzipped of `@canvist/web`). `doc` is now optional on `CommandSpec`, `QuerySpec`, and `FieldSpec`, and `engine.metadata()` is typed as `ExtensionDescriptor[]`, with every description, for tools such as agent tool schemas that need them. _Packages:_ 🟠 _@canvist/headless_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/canvist/canvist/pull/156)

### 🐛 Fixed

- **Publish `@canvist/canvas` to npm.** `@canvist/canvas`, the design canvas for the browser, now publishes to npm with the rest of the SDK, through npm trusted publishing. _Packages:_ 🟢 _@canvist/canvas_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #202](https://github.com/canvist/canvist/pull/202)
- **Read the bundled WebAssembly module from a file path.** `load()` converts the bundled module's `file:` URL to a path before reading it, so the package type-checks under JSR's `deno publish` as well as with `@types/node`. _Packages:_ 🟢 _@canvist/headless_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)
- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Packages:_ 🟢 _@canvist/headless_, 🟢 _@canvist/react_, 🟢 _@canvist/vue_, 🟢 _@canvist/web_, 🟢 _canvist_canvas_, 🟢 _canvist_canvas_wasm_, 🟢 _canvist_collab_, 🟢 _canvist_core_, 🟢 _canvist_extensions_, 🟢 _canvist_ffi_, 🟢 _canvist_layout_, 🟢 _canvist_model_, 🟢 _canvist_paint_, 🟢 _canvist_scene_, 🟢 _canvist_web_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

#### Screen readers meet the text field first, and placeholders have 4.5:1 contrast

_Packages:_ 🟢 _@canvist/web_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #168](https://github.com/canvist/canvist/pull/168)

- In Firefox and Safari, where text arrives through a hidden textarea, the textarea now sits right after the canvas, as EditContext's field does in Chromium, so screen readers reach the editor's text field before its document mirror.
- The placeholder shows the body text's colour at 62% instead of 50%, which gives 4.5:1 contrast on the StarterKit's light and dark pages.
- New browser tests: an axe audit (WCAG 2.2 AA) of editable, read-only, empty, and menu-showing editors; virtual screen reader smoke tests; and Safari composition, Android keyboard, and touch input sequences in Chromium, Firefox, and WebKit.

#### Keep `@canvist/web` lean when it compiles only the StarterKit

_Packages:_ 🟢 _@canvist/web_, 🟢 _canvist_core_, 🟢 _canvist_extensions_, 🟢 _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

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

#### Publish the Dart and Flutter packages to pub.dev

_Packages:_ 🟢 _canvist-dart_, 🟢 _canvist_flutter_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #150](https://github.com/canvist/canvist/pull/150)

The `canvist` package's Rust crate no longer inherits anything from the Cargo workspace. It names `canvist_headless` by the exact crates.io release it was generated against, which monochange bumps on every release, and the workspace's `[patch.crates-io]` builds the local sources inside this repository. So the build hook compiles the crate from the pub cache, where the workspace does not exist.

Both packages now ship a `LICENSE`, a `README.md`, and a repository link, and allow `flutter_rust_bridge` patch releases. CI validates both archives with `pub publish --dry-run` and builds and tests the `canvist` archive outside the repository. The release workflow publishes them after crates.io.

#### Fix three ways collaborating peers' documents could diverge

_Packages:_ 🟢 _canvist_collab_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #135](https://github.com/canvist/canvist/pull/135)

- A block copy that loses its id to a concurrently moved copy is now evicted from where it sits in the shared array, not from its old place in the model. Blocks inserted after it in the same update used to land out of order on the receiving peer.
- A hidden copy of a block, such as one inside a list that no longer allows it, no longer claims the block's id when reading a document from scratch. `Binding::document` used to hide the shown copy and delete it from the shared document.
- `Reorder::take_ready` is replaced by `Reorder::take_next`, which releases one update at a time against the document's current state vector. An update that yrs keeps pending, because it is inserted next to another client's item that has not arrived, no longer lets that client's later updates overtake it. The binding and canvist Sync rooms both apply updates this way.

- **Stop restarting Android keyboards while the editor has focus.** Android keyboards mark the word at the caret when their input starts, for suggestions. `CanvistEditor` sent its own state back without that mark, which made Android restart the keyboard, which marked the word again, several times a second for as long as the editor had focus. The keyboard and its suggestion strip churned, and suggestions and autocorrect kept resetting. The editor now leaves the word the keyboard marks alone, and composing over it works as before. _Packages:_ 🟢 _canvist_flutter_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #187](https://github.com/canvist/canvist/pull/187)
- **Keep a selection the app sets while the keyboard is still starting.** A keyboard's first updates after the editor gains focus describe the caret the editor opened with. When an app set a new selection in that moment, for example selecting everything as the editor opens, those late updates arrived after it and collapsed the selection, so the next format applied to nothing. `CanvistEditor` now recognises an update that only returns to a selection it has since replaced, and keeps its own. _Packages:_ 🟢 _canvist_flutter_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #195](https://github.com/canvist/canvist/pull/195)
- **Draw bold and italic text at the weight and slant layout measured.** The GPU and CPU painters drew every glyph from a font's default instance, so bold text in a variable font (such as the system UI font) painted at regular weight, and fonts without a bold or italic face drew neither. Glyph runs now carry the variation coordinates parley shaped with, and synthetic bold and italic when the font lacks them. _Packages:_ 🟢 _canvist_paint_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

<details>
<summary><strong>📖 Documentation</strong></summary>

#### Rewrite the docs for people and agents using canvist

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #130](https://github.com/canvist/canvist/pull/130)

- New structure: get started (one quick start per platform, each marked available, preview, or coming soon), core concepts, an extension catalogue, a build-your-own-editor guide, guides, an AI agent guide, canvist Cloud, and reference.
- Every Rust, TypeScript, and Dart sample runs in CI: Rust pages are doc tests of `canvist_book`, and `docs/check-snippets.ts` type-checks, analyzes, and runs the others, checking printed output against the comments.
- The website serves `/llms.txt` and `/llms-full.txt`, and groups the docs sidebar by `SUMMARY.md` parts. A draft `canvist` agent skill lives in `skills/canvist/`.
- Design records, research, the roadmap, and canvist Cloud operations move to `docs/internal/`, outside the published docs.

#### Document React Native and Lynx, and list them as previews

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #199](https://github.com/canvist/canvist/pull/199)

- New quick starts for [React Native](docs/src/quick-start/react-native.md) and [Lynx](docs/src/quick-start/lynx.md): install from the repository, show an editor, add a toolbar that follows the selection, save and load, and what calls from JavaScript cost.
- The platform tables in the README and the quick-start index, the website's platforms section, and the agent notes in `/llms.txt` list React Native and Lynx as previews, installed from GitHub.
- The agent guide and the `canvist` skill list React Native and Lynx, and the skill the iOS, Android, and desktop editors, among the packages that exist.

- **Mark the browser platforms Available.** The web, React, Vue, server-side rendering, and design canvas quick starts, the platform tables in the readme and the docs, the agent guide and skill, and the website's platform list now say "Available". The package readmes describe read-only editors, placeholders, menus, collaboration, server rendering, and the bundle budget. _Packages:_ ⚪ _@canvist/canvas_, ⚪ _@canvist/react_, ⚪ _@canvist/vue_, ⚪ _@canvist/web_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/canvist/canvist/pull/173)

</details>

<details>
<summary><strong>⚡ Performance</strong></summary>

- **Drop metadata descriptions from the browser editor's module.** `@canvist/web` builds its WebAssembly module with `--cfg canvist_omit_docs`, which leaves the command, query, and argument descriptions out of `canvist_core`'s metadata. Only bindings that generate types read them, and the browser module never does. With `wasm-opt --converge`, the editor ships 349.1 KB gzipped, down from 349.97 KB, under its 350 KB budget. _Packages:_ ⚪ _@canvist/web_, ⚪ _canvist_core_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/canvist/canvist/pull/154)

</details>

<details>
<summary><strong>🧪 Testing</strong></summary>

#### Test canvas and server-rendered HTML for pixel parity in every browser

_Packages:_ ⚪ _@canvist/web_, ⚪ _canvist_layout_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #138](https://github.com/canvist/canvist/pull/138)

- Playwright renders a fixture matrix (system and web fonts, a variable font, CJK with `unicode-range` subsets, Arabic and Hebrew, Devanagari, three emoji formats, letter spacing, sizes from 10 to 64 px) as the canvas, the SSR HTML, and SVG, at device pixel ratios 1 to 3, fractional zoom, and light and dark themes, in Chromium, Firefox, and WebKit on macOS and Linux.
- Line boxes, runs, and caret stops are compared exactly; pixels with pixelmatch and SSIM against per-browser limits, with every known difference documented. Each run uploads a side-by-side report.
- A Rust test holds `parley`, with the same Inter file, to Chromium's line breaks.

</details>
