/**
 * Type-level tests, checked by `tsc --noEmit` and never run: the editor's
 * commands, queries, and content follow the extensions you register.
 */
import {
	bold,
	CanvistEditor,
	type Editor,
	heading,
	useCanvistEditor,
} from "../src/mod.ts";

export function registered(editor: Editor<[typeof bold, typeof heading]>) {
	editor.commands.toggleBold();
	editor.commands.setHeading({ level: 3 });
	editor.run("toggleHeading", { level: 1 });
	// Paragraphs are always there.
	editor.commands.setParagraph();
	const level: number | null = editor.query.headingLevel();
	const bolded: boolean = editor.ask("isBold");

	// @ts-expect-error italic is not registered
	editor.commands.toggleItalic();
	// @ts-expect-error italic is not registered
	editor.run("toggleItalic");
	// @ts-expect-error levels are 1 to 6
	editor.commands.setHeading({ level: 7 });
	// @ts-expect-error setHeading needs a level
	editor.commands.setHeading();
	// @ts-expect-error isItalic is not registered
	editor.query.isItalic();

	return [level, bolded];
}

export function content() {
	return (
		<view>
			<CanvistEditor
				extensions={[bold]}
				content={{
					type: "doc",
					content: [{
						type: "paragraph",
						content: [{ type: "text", text: "Hi", marks: [{ type: "bold" }] }],
					}],
				}}
			/>
			<CanvistEditor
				extensions={[bold]}
				content={{
					type: "doc",
					// @ts-expect-error headings are not registered
					content: [{ type: "heading", attrs: { level: 1 } }],
				}}
			/>
			<CanvistEditor
				extensions={[bold]}
				onReady={(editor) => {
					editor.commands.toggleBold();
					// @ts-expect-error heading is not registered
					editor.commands.toggleHeading({ level: 2 });
				}}
			/>
		</view>
	);
}

export function Hook() {
	const { editor } = useCanvistEditor();
	// Every StarterKit extension by default.
	editor?.commands.toggleTaskList();
	editor?.commands.toggleCodeBlock({ language: "rust" });

	return null;
}
