/**
 * The editor in React: `useCanvistEditor` for the editor, and
 * `useEditorState` so the toolbar re-renders when the selection moves.
 */
import { useCanvistEditor, useEditorState } from "@canvist/react";
import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { CONTENT } from "./content.ts";

function App() {
	const { ref, editor } = useCanvistEditor({
		content: CONTENT,
		label: "Example document",
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

	useEffect(() => {
		Object.assign(globalThis, { editor });
	}, [editor]);

	return (
		<>
			<div className="toolbar">
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
			</div>
			<div ref={ref} className="editor" />
		</>
	);
}

createRoot(document.querySelector("#app")!).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
