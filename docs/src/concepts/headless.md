# Headless: bring your own UI

Read this page to understand why canvist ships no toolbar, and how you build the UI you want on top of it.

## canvist draws the document, you draw the rest

canvist renders the document and handles typing, selection, and editing keys. It doesn't ship buttons, menus, popovers, or dialogs. Those belong to your app, in your design system and your framework.

So you never fight a built-in toolbar's styling, and you never hide features you didn't ask for. A note-taking app, a chat composer, and a CMS can share one engine and look nothing alike.

## Everything is reachable by calls

Every capability an extension adds is a call you can make:

| You want to…                               | Call                        |
| ------------------------------------------ | --------------------------- |
| Make a "Bold" button work                  | `doc.commands.toggleBold()` |
| Disable it where bold isn't allowed        | `doc.can.toggleBold()`      |
| Show it pressed when the selection is bold | `doc.query.isBold()`        |
| Show "Heading 2" in a dropdown             | `doc.query.headingLevel()`  |
| Prefill a link dialog                      | `doc.query.linkHref()`      |

Nothing happens by magic. If your UI shows something, it came from a query you made, so it's always in sync with the document.

## One engine, many surfaces

The same calls work wherever the engine runs:

- **In an editor:** your toolbar calls commands against the live selection.
- **On a server:** a job calls commands against a selection it sets, with no screen at all.
- **In tests:** you type, press keys, and assert on the exported document.
- **From an AI agent:** the agent calls commands by name, and the engine checks every call. See [Integrate canvist with an agent](../agents/readme.md).

## Next

- [Add a toolbar](../guides/toolbar.md): a complete toolbar built from these calls.
- [Keyboard shortcuts](../guides/keyboard-shortcuts.md): the keys each extension binds.
