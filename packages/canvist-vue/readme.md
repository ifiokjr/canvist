<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/vue.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@canvist/vue"><img src="https://img.shields.io/npm/v/%40canvist%2Fvue?color=3B2CF5" alt="npm version" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/web.yml/badge.svg?branch=main" alt="web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/vue

Vue bindings for [`@canvist/web`](../canvist-web), the canvist canvas editor. Commands, queries, and plugin APIs are typed from the extensions and plugins you pass.

```sh
npm install @canvist/vue
```

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
- `<CanvistEditor />` takes the editor's options as props, emits `ready` and `update`, and takes `html` for server-rendered HTML to hydrate. `editable` and `placeholder` follow their props.
- `<BubbleMenu :editor="editor">` and `<FloatingMenu :editor="editor">` keep their slot next to the selection.

See [Quick start: Vue](../../docs/src/quick-start/vue.md).
