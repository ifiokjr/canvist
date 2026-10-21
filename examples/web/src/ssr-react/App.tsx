/**
 * The server-rendered React page, rendered on the server by `server.tsx`
 * and hydrated in the browser by `client.tsx`, as a Next.js page or
 * server component would be.
 *
 * The server passes canvist's rendered HTML and stylesheet as props. The
 * browser hydrates with the same props, and `CanvistEditor` then reads the
 * document from the HTML and swaps in the canvas once Inter has loaded.
 */
import { CanvistEditor, useEditorState } from "@canvist/react";
import type { Editor, StarterKit } from "@canvist/web";
import { type ReactElement, useState } from "react";
import { THEME, WIDTH } from "../ssr-content.ts";

/** What the server renders the page from. */
export interface PageProps {
	/** canvist's `.cv-doc` HTML. */
	readonly html: string;
	/** Its stylesheet. */
	readonly css: string;
}

export function App({ html, css }: PageProps): ReactElement {
	const [editor, setEditor] = useState<Editor<StarterKit> | null>(null);
	const isBold = useEditorState(
		editor,
		(current) => current.query.isBold(),
		false,
	);

	return (
		<>
			{/* React escapes text children, which would break CSS selectors. */}
			<style dangerouslySetInnerHTML={{ __html: css }} />
			<div className="toolbar">
				<button
					type="button"
					disabled={editor === null}
					aria-pressed={isBold}
					onClick={() => {
						editor?.commands.toggleBold();
						editor?.focus();
					}}
				>
					Bold
				</button>
			</div>
			<CanvistEditor
				className="editor"
				style={{ width: WIDTH }}
				html={html}
				theme={THEME}
				label="Server-rendered document"
				onReady={(ready) => {
					setEditor(ready);
					Object.assign(globalThis, { editor: ready });
				}}
			/>
			<p id="after">
				This paragraph sits under the editor. It must not move when the editor
				hydrates.
			</p>
		</>
	);
}
