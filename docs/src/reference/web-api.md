# Browser API reference

Use this page to look up every option and method of the browser packages: `@canvist/web` and its `/fonts`, `/menus`, and `/collab` entry points, `@canvist/headless/ssr`, and `@canvist/canvas`. Each one also carries these descriptions in its TypeScript declarations, so your editor shows them on hover.

## API stability

The browser packages follow semantic versioning with the rest of the SDK. Every change ships with a changeset that says what changed, and the [changelog](./readme.md#changelog) lists them. Until 1.0, a release that breaks an API raises the minor version (`0.1` to `0.2`) and says so under "Breaking Change". Members marked `@internal` in the declarations aren't part of the API and can change in any release.

## `createEditor(element, options)`

From `@canvist/web`. It returns a promise of an `Editor`. `@canvist/web/collab` exports a `createEditor` with the same options, and its editors can collaborate.

| Option        | Type                                    | Default             | What it does                                                                                                                              |
| ------------- | --------------------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `extensions`  | `ExtensionDescriptor[]`                 | TipTap's StarterKit | The extensions to register. Commands, queries, and document JSON are typed from them.                                                     |
| `plugins`     | `Plugin[]`                              | `[]`                | Plugins such as `fonts()` from `@canvist/web/fonts`. Their APIs are on `editor.plugins`.                                                  |
| `content`     | `{ format, input }` or ProseMirror JSON | the element's HTML  | The starting document. Without it, the editor reads server-rendered `.cv-doc` HTML inside the element, or starts empty.                   |
| `theme`       | `ThemeOptions`                          | StarterKit light    | `scheme`, `bodyFamily`, `codeFamily`, `size`, `lineHeight`, and `letterSpacing`. Server rendering must use the same one.                  |
| `editable`    | `boolean`                               | `true`              | `false` makes the editor read-only: selecting and copying still work, and commands still apply.                                           |
| `placeholder` | `string`                                | `""`                | Dimmed text where the first line starts while the document is empty. It's also the field's `aria-placeholder`.                            |
| `label`       | `string`                                | `"Editor"`          | The editor's accessible name.                                                                                                             |
| `input`       | `"auto"`, `"editcontext"`, `"textarea"` | `"auto"`            | How text arrives: `EditContext` where the browser has it, or a hidden textarea. `"textarea"` is the kill switch for EditContext problems. |
| `platform`    | `"apple"`, `"other"`                    | detected            | Whether `Mod` in key bindings is Cmd or Ctrl.                                                                                             |
| `breaks`      | `"chromium"`, `"icu"`                   | detected            | Which line-breaking tailoring layout follows, to match the browser's own.                                                                 |
| `client`      | `number`                                | `1`                 | The client id new blocks are allocated under.                                                                                             |
| `overscan`    | `number`                                | one screen          | Pixels beyond the viewport to lay out ahead of scrolling.                                                                                 |
| `wasm`        | `WasmSource`                            | next to the module  | Where to load the WebAssembly module from: bytes, a compiled module, a URL, or a response.                                                |

## `Editor`

| Member                                                 | What it does                                                                                                        |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| `commands.<name>(args)`, `can.<name>(args)`            | Run a registered command, or check whether it would apply. Both return `boolean`.                                   |
| `query.<name>(args)`                                   | Answer a registered query.                                                                                          |
| `run(name, args)`, `ask(name, args)`                   | The same, for names picked at runtime.                                                                              |
| `select(anchor, head?)`, `selection`                   | Set or read the selection. Positions are `{ path, offset }`, with offsets in UTF-16 code units.                     |
| `insertText(text)`, `undo()`, `redo()`                 | Type, undo, and redo, as the keyboard does. A shared editor undoes only its own changes.                            |
| `setContent(content)`, `export(format)`, `toJson()`    | Replace the document, or read it as `json`, `html`, `markdown`, or `text`.                                          |
| `toHtml()`, `toSvg()`                                  | Server-rendered HTML and CSS at the editor's width, or an SVG of the whole document.                                |
| `editable`, `setEditable(editable)`                    | Read or change read-only mode.                                                                                      |
| `setPlaceholder(text)`, `isEmpty`                      | Change the placeholder, or check for an empty document.                                                             |
| `focus()`, `focused`                                   | Focus the editor without scrolling, or check whether it has focus.                                                  |
| `subscribe(listener)`                                  | Call back after every change to the document, the selection, focus, or `editable`. Returns an unsubscribe function. |
| `caretRect()`, `selectionRect()`, `toClientRect(rect)` | The caret's or selection's rectangle in document coordinates, and conversion to viewport coordinates.               |
| `blockText()`                                          | The text of the block holding the selection's head.                                                                 |
| `ready()`, `hydrating`                                 | Wait until fonts and plugins have settled and the canvas shows, or check whether server HTML still stands in.       |
| `inputLatency()`, `stats()`, `fonts()`, `geometry()`   | Input-to-paint times, measurement cache statistics, used fonts, and laid-out geometry.                              |
| `render()`, `destroy()`                                | Paint now, or remove the editor and release its memory.                                                             |

## `@canvist/web/menus`

`bubbleMenu(editor, element, options)` keeps `element` over selected text, and `floatingMenu(editor, element, options)` keeps it beside the caret on an empty line. Both return a `Menu` with `visible`, `update()`, and `destroy()`.

| Option       | Type                                     | Default                               | What it does                                               |
| ------------ | ---------------------------------------- | ------------------------------------- | ---------------------------------------------------------- |
| `placement`  | `"top"`, `"bottom"`, `"left"`, `"right"` | `"top"` (bubble), `"left"` (floating) | The preferred side. The menu flips when there's no room.   |
| `offset`     | `number`                                 | `8`                                   | The gap between the menu and the selection, in CSS pixels. |
| `padding`    | `number`                                 | `8`                                   | The least distance from the viewport's edges.              |
| `shouldShow` | `(editor) => boolean`                    | see each function                     | Replaces the default rule for when the menu shows.         |

`placeMenu(element, rect, { placement, offset, padding })` does the positioning on its own.

## `@canvist/web/collab`

`connect(editor, options)` joins an editor from this module's `createEditor` to a room, and returns a `Connection`. The connection has `status` (`"connecting"`, `"connected"`, or `"disconnected"`), `synced`, `clientId`, `subscribe(listener)`, `whenSynced()`, and `disconnect()`.

| Option      | Type                     | Default    | What it does                                               |
| ----------- | ------------------------ | ---------- | ---------------------------------------------------------- |
| `url`       | `string`                 | (required) | The y-websocket server's URL. The room is added as a path. |
| `room`      | `string`                 | (required) | Every peer in a room edits one document.                   |
| `params`    | `Record<string, string>` | none       | Query parameters, such as a short-lived token.             |
| `clientId`  | `number`                 | random     | This peer's Yjs client id, unique in the room.             |
| `reconnect` | `boolean`                | `true`     | Reconnect with backoff when the connection drops.          |
| `WebSocket` | `typeof WebSocket`       | the global | The `WebSocket` class to use.                              |

## `@canvist/headless/ssr`

`createRenderer({ extensions, theme, fonts, wasm })` returns a `Renderer`: `html(content, { width })` returns `{ html, css }`, `page(content, { width })` returns a complete page, `svg(content, { width })` returns an SVG, and `registerFont(bytes)` adds a font for SVG text. `fonts` are TrueType or OpenType files. See [Server-side rendering](../quick-start/ssr.md).

## `@canvist/canvas`

`mountCanvas(canvas, options)` returns a `CanvasHandle` with `backend`, `tools`, `setTool(name)`, `tool()`, `toJson()`, `encodeState()`, `applyUpdate(update)`, `frameTimes()`, `requestFrame()`, and `destroy()`.

| Option      | Type               | Default            | What it does                                                   |
| ----------- | ------------------ | ------------------ | -------------------------------------------------------------- |
| `scene`     | `SceneJson`        | one empty page     | The starting scene. Give every peer in a room the same one.    |
| `demo`      | `boolean`          | `false`            | Start from canvist's demo scene instead.                       |
| `transport` | `Transport`        | none               | Where collaboration messages go, such as `BroadcastTransport`. |
| `preferGpu` | `boolean`          | `true`             | Use WebGPU where the browser has it.                           |
| `wasmUrl`   | `URL` or `string`  | next to the module | Where the WebAssembly module is.                               |
| `onChange`  | `(handle) => void` | none               | Called after the canvas changes, for toolbars.                 |
| `stress`    | `number`           | `0`                | Extra rectangles, for measuring frame times.                   |
