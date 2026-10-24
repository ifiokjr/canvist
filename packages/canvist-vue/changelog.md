All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Read-only editors, placeholders, and menus that follow the selection

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/canvist/canvist/pull/156)

`@canvist/web` gains the editor features TipTap users reach for first.

- **`editable`**: `createEditor(element, { editable: false })` and `editor.setEditable(editable)` make an editor read-only. People can still select with the pointer and navigation keys and copy, but typing, pasting, cutting, and dropping change nothing, no caret shows, and the field has `aria-readonly`. Commands and the API still change the document, as in TipTap. `editor.editable` reads it.
- **`placeholder`**: dimmed text where the first line starts while the document is empty, set in the body font and line height so it sits where typing begins, and the field's `aria-placeholder`. `editor.setPlaceholder(text)` changes it, and `editor.isEmpty` reports an empty document.
- **`@canvist/web/menus`**: `bubbleMenu(editor, element, options)` keeps an element over selected text, and `floatingMenu` beside the caret on an empty line. They follow edits, scrolling, and resizing, flip to the other side when there is no room, keep the editor's focus when pressed, and take `placement`, `offset`, `padding`, and `shouldShow`. `placeMenu(element, rect, options)` is the positioning on its own.
- **Selection geometry**: `editor.selectionRect()` is the selection's bounding box in document coordinates, `editor.toClientRect(rect)` converts to viewport coordinates, and `editor.blockText()` is the text of the block holding the selection. `caretRect()` and `selectionRect()` now lay out first when an edit is waiting for the next frame, so they describe the edited document.
- **Focus**: `editor.focused` reads focus, and `subscribe` listeners also hear about focus and `editable` changes.
- `@canvist/react` and `@canvist/vue`: `CanvistEditor`'s `editable` and `placeholder` props follow their values, and `BubbleMenu` and `FloatingMenu` components wrap the menus.

#### React and Vue bindings, and hydrating server-rendered HTML

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #141](https://github.com/canvist/canvist/pull/141)

- `@canvist/react` adds `useCanvistEditor`, `useEditorState` (through `useSyncExternalStore`), and `<CanvistEditor />`. `@canvist/vue` adds the same composables and a component that emits `ready` and `update`. Both are typed from the extensions and plugins you register.
- `editor.subscribe(listener)` in `@canvist/web` calls back after every document or selection change.
- `createEditor` hydrates server-rendered HTML: given an element holding canvist's `.cv-doc` markup, it reads the document from it, keeps it on screen until fonts settle, and swaps in the canvas in one frame, with no layout shift.
- `examples/web` shows a plain page, React, Vue, and a page rendered by the new `ssr_page` example in `canvist_headless`, with Playwright tests in every browser.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

<details>
<summary><strong>📖 Documentation</strong></summary>

- **Mark the browser platforms Available.** The web, React, Vue, server-side rendering, and design canvas quick starts, the platform tables in the readme and the docs, the agent guide and skill, and the website's platform list now say "Available". The package readmes describe read-only editors, placeholders, menus, collaboration, server rendering, and the bundle budget. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/canvist/canvist/pull/173)

</details>
