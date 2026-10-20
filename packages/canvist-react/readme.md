<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/quick-start/react.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/react"><img src="https://img.shields.io/npm/v/%40canvist%2Freact?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/web.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/react

React bindings for [`@canvist/web`](../canvist-web), the canvist canvas editor. Commands, queries, and plugin APIs are typed from the extensions and plugins you pass.

```sh
npm install @canvist/react
```

```tsx
import { useCanvistEditor, useEditorState } from "@canvist/react";

function Notes() {
	const { ref, editor } = useCanvistEditor({ label: "Notes" });
	const isBold = useEditorState(
		editor,
		(current) => current.query.isBold(),
		false,
	);

	return (
		<>
			<button
				aria-pressed={isBold}
				onClick={() => editor?.commands.toggleBold()}
			>
				Bold
			</button>
			<div ref={ref} />
		</>
	);
}
```

- `useCanvistEditor(options)` creates the editor in the element `ref` is attached to and destroys it on unmount, Strict Mode included.
- `useEditorState(editor, select, fallback)` re-renders when `select(editor)` changes, through `useSyncExternalStore`.
- `<CanvistEditor />` takes the editor's options as props, plus `onReady`, `onUpdate`, and `html` for server-rendered HTML to hydrate.

See [Quick start: React](../../docs/src/quick-start/react.md).
