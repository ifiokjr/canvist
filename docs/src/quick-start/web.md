# Quick start: Web

In this guide you put a live canvist editor on a web page with no build step. It takes about two minutes.

**Status: preview.** The browser editor you load below is a preview. It types, selects, and paints to a canvas, but its features are fixed: you can't pick extensions yet, and its API will change. The extension-based browser package, `@canvist/core`, is coming soon. It will use the same extensions and typed commands as [the server packages](./server-typescript.md), so the [concepts](../concepts/documents.md) you learn now carry over.

Need canvist in a web app today? Use [`@canvist/headless`](./server-typescript.md) on your server or in a worker to import, transform, and export content. It's ready for production.

## Try the preview

Save this as `index.html`:

```html
<!doctype html>
<meta charset="utf-8">
<title>My canvist editor</title>
<canvas id="editor" width="800" height="400"></canvas>
<script type="module">
  import { createEditor } from "https://esm.sh/gh/ifiokjr/canvist@main/packages/canvist/src/mod.ts";

  const editor = await createEditor("editor");
  editor.insertText("Hello, canvist!");
  editor.render();
</script>
```

Serve the folder with any static server, then open it and start typing:

```sh
npx serve .
```

The page loads about 1.2 MB of WebAssembly the first time.

## What's coming

`@canvist/core` gives you a canvas editor built from the extensions you choose, with:

- input through the browser's `EditContext` where it exists, so IME, dictation, and autocorrect work;
- an accessibility tree for screen readers;
- server-rendered HTML that the canvas replaces with no layout shift;
- `@canvist/react` and `@canvist/vue` wrappers.

Watch [the releases](https://github.com/ifiokjr/canvist/releases) to hear when it ships.
