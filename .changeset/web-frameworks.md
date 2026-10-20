---
"@canvist/web": minor
"@canvist/react": minor
"@canvist/vue": minor
"canvist_headless": test
---

# React and Vue bindings, and hydrating server-rendered HTML

- `@canvist/react` adds `useCanvistEditor`, `useEditorState` (through `useSyncExternalStore`), and `<CanvistEditor />`. `@canvist/vue` adds the same composables and a component that emits `ready` and `update`. Both are typed from the extensions and plugins you register.
- `editor.subscribe(listener)` in `@canvist/web` calls back after every document or selection change.
- `createEditor` hydrates server-rendered HTML: given an element holding canvist's `.cv-doc` markup, it reads the document from it, keeps it on screen until fonts settle, and swaps in the canvas in one frame, with no layout shift.
- `examples/web` shows a plain page, React, Vue, and a page rendered by the new `ssr_page` example in `canvist_headless`, with Playwright tests in every browser.
