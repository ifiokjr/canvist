# Quick start: React

In this guide you add a canvist editor to a React app with `@canvist/react`, and build a toolbar that re-renders when the selection changes.

**Status: preview.** `@canvist/react` wraps [`@canvist/web`](./web.md), which is a preview. Its API may still change before 1.0.

## 1. Install

```sh
npm install @canvist/react @canvist/web
```

With Vite, exclude canvist from dependency pre-bundling, as [the web quick start](./web.md#3-set-up-your-bundler) shows.

## 2. Add an editor and a toolbar

`useCanvistEditor` creates the editor in the element you attach `ref` to, and destroys it on unmount. `useEditorState` re-renders the component when the value it reads changes:

```tsx
import { useCanvistEditor, useEditorState } from "@canvist/react";

export function Notes() {
	const { ref, editor } = useCanvistEditor({
		content: { format: "markdown", input: "# Notes\n\nStart typing." },
		label: "Notes",
	});
	const canBold = useEditorState(
		editor,
		(current) => current.can.toggleBold(),
		false,
	);
	const isBold = useEditorState(
		editor,
		(current) => current.query.isBold(),
		false,
	);

	return (
		<>
			<button
				type="button"
				disabled={!canBold}
				aria-pressed={isBold}
				onClick={() => {
					editor?.commands.toggleBold();
					editor?.focus();
				}}
			>
				Bold
			</button>
			<div ref={ref} style={{ maxWidth: 640 }} />
		</>
	);
}
```

- **The editor is `null` until its WebAssembly has loaded.** Give `useEditorState` a fallback for that moment.
- **Options are read once.** Change the document later with `editor.setContent`.
- **Types follow your extensions.** Pass `extensions: [bold, heading]`, and `editor.commands.toggleItalic()` is a type error.
- **A selector must return a primitive or a stable object**, as with React's `useSyncExternalStore`.

## 3. Or use the component

`CanvistEditor` takes the same options as props, plus `onReady` and `onUpdate`:

```tsx
import { CanvistEditor } from "@canvist/react";

export function Comment() {
	return (
		<CanvistEditor
			label="Comment"
			content={{ format: "markdown", input: "" }}
			onUpdate={(editor) => console.log(editor.export("markdown"))}
			style={{ maxWidth: 640 }}
		/>
	);
}
```

To hydrate server-rendered HTML, pass it as `html`. The editor reads the document from it and swaps in the canvas once fonts are ready, with no layout shift.

`examples/web/src/react.tsx` in the repository is a complete app.
