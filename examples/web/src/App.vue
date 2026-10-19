<script setup lang="ts">
/**
 * The editor in Vue: `useCanvistEditor` for the editor, and
 * `useEditorState` so the toolbar follows the selection.
 */
import { useCanvistEditor, useEditorState } from "@canvist/vue";
import { watchEffect } from "vue";
import { CONTENT } from "./content.ts";

const { element, editor } = useCanvistEditor({
	content: CONTENT,
	label: "Example document",
});
const canBold = useEditorState(editor, (current) => current.can.toggleBold(), false);
const isBold = useEditorState(editor, (current) => current.query.isBold(), false);

watchEffect(() => Object.assign(globalThis, { editor: editor.value }));

function toggleBold(): void {
	editor.value?.commands.toggleBold();
	editor.value?.focus();
}
</script>

<template>
	<div class="toolbar">
		<button type="button" :disabled="!canBold" :aria-pressed="isBold" @click="toggleBold">
			Bold
		</button>
	</div>
	<div ref="element" class="editor" />
</template>
