# @canvist/react

React bindings for [`@canvist/web`](../canvist-web), the canvist canvas editor. Commands, queries, and plugin APIs are typed from the extensions and plugins you pass.

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
