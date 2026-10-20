<script setup lang="ts">
/**
 * The editor in Vue: `useCanvistEditor` for the editor, `useEditorState`
 * so the toolbar follows the selection, a read-only switch, and a
 * `BubbleMenu` over selected text.
 */
import { BubbleMenu, useCanvistEditor, useEditorState } from "@canvist/vue";
import { ref, watchEffect } from "vue";
import { CONTENT, PLACEHOLDER } from "./content.ts";

const { element, editor } = useCanvistEditor({
	content: CONTENT,
	label: "Example document",
	placeholder: PLACEHOLDER,
});
const readOnly = ref(false);
const canBold = useEditorState(editor, (current) => current.can.toggleBold(), false);
const isBold = useEditorState(editor, (current) => current.query.isBold(), false);
const isItalic = useEditorState(editor, (current) => current.query.isItalic(), false);

watchEffect(() => Object.assign(globalThis, { editor: editor.value }));
watchEffect(() => editor.value?.setEditable(!readOnly.value));

function toggleBold(): void {
	editor.value?.commands.toggleBold();
	editor.value?.focus();
}
</script>

<template>
	<div class="toolbar">
		<button type="button" :disabled="readOnly || !canBold" :aria-pressed="isBold" @click="toggleBold">
			Bold
		</button>
		<label><input v-model="readOnly" type="checkbox"> Read-only</label>
	</div>
	<div ref="element" class="editor" />
	<BubbleMenu :editor="editor" class="bubble">
		<div role="toolbar" aria-label="Selection">
			<button type="button" :aria-pressed="isItalic" @click="editor?.commands.toggleItalic()">
				Italic
			</button>
		</div>
	</BubbleMenu>
</template>
