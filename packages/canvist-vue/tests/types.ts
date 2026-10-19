/**
 * Compile-time checks, run by `tsc`: the composables are typed from the
 * extensions they register.
 */
import { bold, heading } from "@canvist/web";
import { h } from "vue";
import { CanvistEditor, useCanvistEditor, useEditorState } from "../src/mod.ts";

export function setup() {
	const { element, editor } = useCanvistEditor({ extensions: [bold, heading] });
	const canBold = useEditorState(
		editor,
		(current) => current.can.toggleBold(),
		false,
	);
	const enabled: boolean = canBold.value;

	editor.value?.commands.setHeading({ level: 2 });
	// @ts-expect-error italic is not registered
	editor.value?.commands.toggleItalic();

	return { element, enabled, node: h(CanvistEditor, { label: "Notes" }) };
}
