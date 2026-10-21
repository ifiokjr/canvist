/**
 * The editor in React: `useCanvistEditor` for the editor, `useEditorState`
 * so the toolbar re-renders when the selection moves, a read-only switch,
 * and a `BubbleMenu` over selected text.
 */
import { BubbleMenu, useCanvistEditor, useEditorState } from "@canvist/react";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { CONTENT, PLACEHOLDER } from "./content.ts";

function App() {
	const { ref, editor } = useCanvistEditor({
		content: CONTENT,
		label: "Example document",
		placeholder: PLACEHOLDER,
	});
	const [readOnly, setReadOnly] = useState(false);
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
	const isItalic = useEditorState(
		editor,
		(current) => current.query.isItalic(),
		false,
	);

	useEffect(() => {
		Object.assign(globalThis, { editor });
	}, [editor]);

	useEffect(() => {
		editor?.setEditable(!readOnly);
	}, [editor, readOnly]);

	return (
		<>
			<div className="toolbar">
				<button
					type="button"
					disabled={readOnly || !canBold}
					aria-pressed={isBold}
					onClick={() => {
						editor?.commands.toggleBold();
						editor?.focus();
					}}
				>
					Bold
				</button>
				<label>
					<input
						type="checkbox"
						checked={readOnly}
						onChange={(event) => setReadOnly(event.target.checked)}
					/>{" "}
					Read-only
				</label>
			</div>
			<div ref={ref} className="editor" />
			<BubbleMenu editor={editor} className="bubble">
				<div role="toolbar" aria-label="Selection">
					<button
						type="button"
						aria-pressed={isItalic}
						onClick={() => editor?.commands.toggleItalic()}
					>
						Italic
					</button>
				</div>
			</BubbleMenu>
		</>
	);
}

createRoot(document.querySelector("#app")!).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
