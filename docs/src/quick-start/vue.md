# Quick start: Vue

In this guide you add a canvist editor to a Vue app with `@canvist/vue`, and build a toolbar that follows the selection.

**Status: preview.** `@canvist/vue` wraps [`@canvist/web`](./web.md), which is a preview. Its API may still change before 1.0.

## 1. Install

```sh
npm install @canvist/vue @canvist/web
```

With Vite, exclude canvist from dependency pre-bundling, as [the web quick start](./web.md#3-set-up-your-bundler) shows.

## 2. Add an editor and a toolbar

`useCanvistEditor` creates the editor in the element bound to `element` when the component mounts, and destroys it on unmount. `useEditorState` returns a ref that updates after every change:

```vue
<script setup lang="ts">
import { useCanvistEditor, useEditorState } from "@canvist/vue";

const { element, editor } = useCanvistEditor({
	content: { format: "markdown", input: "# Notes\n\nStart typing." },
	label: "Notes",
});
const canBold = useEditorState(editor, (current) => current.can.toggleBold(), false);
const isBold = useEditorState(editor, (current) => current.query.isBold(), false);

function toggleBold(): void {
	editor.value?.commands.toggleBold();
	editor.value?.focus();
}
</script>

<template>
	<button type="button" :disabled="!canBold" :aria-pressed="isBold" @click="toggleBold">
		Bold
	</button>
	<div ref="element" style="max-width: 640px" />
</template>
```

- **The editor is `null` until its WebAssembly has loaded.** Give `useEditorState` a fallback for that moment.
- **Options are read once.** Change the document later with `editor.value.setContent`.
- **Types follow your extensions.** Pass `extensions: [bold, heading]`, and `editor.value.commands.toggleItalic()` is a type error.

## 3. Or use the component

`CanvistEditor` takes the same options as props, and emits `ready` and `update` with the editor:

```vue
<script setup lang="ts">
import { CanvistEditor } from "@canvist/vue";
</script>

<template>
	<CanvistEditor label="Comment" @update="(editor) => console.log(editor.export('markdown'))" />
</template>
```

Its `editable` and `placeholder` props follow their values, so `<CanvistEditor :editable="!locked" />` switches read-only on and off. The other options are read once.

To hydrate server-rendered HTML, pass it as `html`. The editor reads the document from it and swaps in the canvas once fonts are ready, with no layout shift. [Server-side rendering](./ssr.md#nuxt) shows a Nuxt page.

## 4. Add a bubble menu

`BubbleMenu` keeps its slot over selected text, and `FloatingMenu` beside the caret on an empty line. Both take the editor and the options of [`@canvist/web/menus`](./web.md#6-menus-that-follow-the-selection):

```vue
<script setup lang="ts">
import { BubbleMenu, useCanvistEditor } from "@canvist/vue";

const { element, editor } = useCanvistEditor({ placeholder: "Write…" });
</script>

<template>
	<div ref="element" style="max-width: 640px" />
	<BubbleMenu :editor="editor" placement="top">
		<button type="button" @click="editor?.commands.toggleItalic()">Italic</button>
	</BubbleMenu>
</template>
```

`examples/web/src/App.vue` in the repository is a complete app.
