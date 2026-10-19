# How canvist works

Read this page if you're curious what happens between a keystroke and the pixels on screen. You don't need it to use canvist.

## One Rust core, thin hosts

```text
┌─────────────────────────────────────────────────────────────┐
│ Your app: toolbar, menus, dialogs                           │
├─────────────────────────────────────────────────────────────┤
│ Host for your platform                                      │
│ native text input, IME, clipboard, accessibility, painting  │
├─────────────────────────────────────────────────────────────┤
│ canvist core (Rust)                                         │
│ document model · editing engine · extensions · text layout  │
│ collaboration (Yjs) · HTML, Markdown, and JSON              │
└─────────────────────────────────────────────────────────────┘
```

The core is the same code on every platform. A host is a thin layer that turns the platform's input into editor commands, and paints what the core lays out. That's why a document behaves the same in Flutter, on your server, and in the browser.

## From keystroke to pixels

1. **You press a key.** The host receives it through the platform's own text input: `EditContext` in the browser, `UITextInput` on iOS, `InputConnection` on Android. That's what makes autocorrect, dictation, and IME composition work.
2. **The engine decides what it means.** Keyboard shortcuts and input rules from your extensions run first. `**` might close a bold run; `## ` might turn the line into a heading.
3. **The document changes in small steps.** Every change is a step that can be undone exactly, and that maps one-to-one onto a Yjs operation for collaboration.
4. **Only the edited paragraph is laid out again.** Layout breaks lines with the same Unicode rules browsers use, and reuses every line the edit didn't touch.
5. **The host paints a display list.** Layout produces a list of positioned text runs, rectangles, and decorations. The browser paints it on a canvas; native platforms paint it with their own graphics APIs.

A keystroke costs microseconds in the core, whatever the size of the document. See [Performance](../guides/performance.md) for the measured numbers.

## Why paint to a canvas?

`contenteditable` and platform text views behave differently on every browser and operating system, and they fight you when you want custom rendering. Painting every pixel yourself means:

- the same layout and line breaks everywhere;
- remote carets, comments, and highlights that sit exactly where they should;
- custom rendering, such as pages, without hacks.

The cost is that the host must rebuild what the platform used to give for free: text input, selection handles, accessibility, and find-in-page. Each host does that work, and keeps an accessibility tree in sync with the document so screen readers can read and edit it.

## Text measurement

Layout needs to know how wide each word is. The host measures text with the platform's own fonts (the browser's `measureText`, CoreText on iOS), and the core does everything else. On a server, the core measures with its own font engine, so [server-rendered HTML and SVG](../quick-start/ssr.md) wrap at the same points as the live editor.

## Collaboration

When a document is shared, every step is mirrored into a Yjs document, and remote Yjs updates become steps. canvist speaks the standard Yjs sync protocol, so it works with y-websocket, Hocuspocus, and [canvist Cloud](../cloud/readme.md). See [Collaboration](../guides/collaboration.md).
