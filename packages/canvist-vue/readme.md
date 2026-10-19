# @canvist/vue

Vue bindings for [`@canvist/web`](../canvist-web), the canvist canvas editor. Commands, queries, and plugin APIs are typed from the extensions and plugins you pass.

```vue
<script setup lang="ts">
import { useCanvistEditor, useEditorState } from "@canvist/vue";

const { element, editor } = useCanvistEditor({ label: "Notes" });
const isBold = useEditorState(editor, (current) => current.query.isBold(), false);
</script>

<template>
	<button :aria-pressed="isBold" @click="editor?.commands.toggleBold()">Bold</button>
	<div ref="element" />
</template>
```

- `useCanvistEditor(options)` creates the editor in `element` on mount and destroys it on unmount.
- `useEditorState(editor, select, fallback)` returns a ref updated after every change.
- `<CanvistEditor />` takes the editor's options as props, emits `ready` and `update`, and takes `html` for server-rendered HTML to hydrate.

See [Quick start: Vue](../../docs/src/quick-start/vue.md).
