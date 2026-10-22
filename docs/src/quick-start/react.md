# Quick start: React

In this guide you add a canvist editor to a React app with `@canvist/react`, and build a toolbar that re-renders when the selection changes.

**Status: available.** `@canvist/react` wraps [`@canvist/web`](./web.md), supports React 18.3 and 19, and is tested under Strict Mode and with server rendering (`renderToString` and `hydrateRoot`).

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

Its `editable` and `placeholder` props follow their values, so `<CanvistEditor editable={!locked} />` switches read-only on and off. The other options are read once.

To hydrate server-rendered HTML, pass it as `html`. The editor reads the document from it and swaps in the canvas once fonts are ready, with no layout shift. [Server-side rendering](./ssr.md#nextjs) shows a Next.js page.

## 4. Add a bubble menu

`BubbleMenu` keeps its children over selected text, and `FloatingMenu` beside the caret on an empty line. Both take the editor and the options of [`@canvist/web/menus`](./web.md#6-menus-that-follow-the-selection):

```tsx
import { BubbleMenu, useCanvistEditor } from "@canvist/react";

export function Notes() {
	const { ref, editor } = useCanvistEditor({ placeholder: "Write…" });

	return (
		<>
			<div ref={ref} style={{ maxWidth: 640 }} />
			<BubbleMenu editor={editor} placement="top">
				<button type="button" onClick={() => editor?.commands.toggleItalic()}>
					Italic
				</button>
			</BubbleMenu>
		</>
	);
}
```

`examples/web/src/react.tsx` in the repository is a complete app.
