/**
 * Compile-time checks, run by `tsc`: the hooks and component are typed from
 * the extensions they register.
 */
import { bold, heading } from "@canvist/web";
import {
	CanvistEditor,
	useCanvistEditor,
	useEditorState,
} from "../src/mod.tsx";

export function Typed() {
	const { ref, editor } = useCanvistEditor({ extensions: [bold, heading] });
	const canBold: boolean = useEditorState(
		editor,
		(current) => current.can.toggleBold(),
		false,
	);

	editor?.commands.setHeading({ level: 2 });
	// @ts-expect-error italic is not registered
	editor?.commands.toggleItalic();
	// @ts-expect-error heading levels are numbers
	editor?.commands.setHeading({ level: "2" });

	return (
		<>
			<button type="button" disabled={!canBold}>Bold</button>
			<div ref={ref} />
			<CanvistEditor
				extensions={[bold]}
				label="Notes"
				onUpdate={(current) => {
					current.commands.toggleBold();
					// @ts-expect-error heading is not registered here
					current.commands.setHeading({ level: 1 });
				}}
			/>
		</>
	);
}
