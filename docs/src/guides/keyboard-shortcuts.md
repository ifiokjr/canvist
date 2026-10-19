# Keyboard shortcuts

Use this page to find the keys and typing shortcuts each extension adds, send keys to the engine yourself, and add your own.

## How keys are written

canvist writes keys the way ProseMirror and TipTap do: modifiers, then the key, joined by `-`.

- `Mod` is Cmd on macOS and iOS, and Ctrl everywhere else. Use it for shortcuts that should feel native on both.
- The other modifiers are `Shift`, `Alt` (Option on a Mac), and `Ctrl`.
- Keys use their `KeyboardEvent.key` names: `Enter`, `Backspace`, `Tab`, `ArrowLeft`, `b`.

So `Mod-Shift-z` is Cmd+Shift+Z on a Mac, and Ctrl+Shift+Z on Windows.

## Shortcuts by extension

| Extension       | Keys                                     | Typing shortcuts       |
| --------------- | ---------------------------------------- | ---------------------- |
| Paragraph       | `Mod-Alt-0`                              |                        |
| Heading         | `Mod-Alt-1` to `Mod-Alt-6`               | `#` to `######`        |
| Bold            | `Mod-b`                                  | `**text**`, `__text__` |
| Italic          | `Mod-i`                                  | `*text*`, `_text_`     |
| Underline       | `Mod-u`                                  |                        |
| Strike          | `Mod-Shift-s`                            | `~~text~~`             |
| Code            | `Mod-e`                                  | `` `text` ``           |
| Link            |                                          | A URL, then a space    |
| Blockquote      | `Mod-Shift-b`                            | `>`                    |
| Code block      | `Mod-Alt-c`                              | `` ``` `` then space   |
| Bullet list     | `Mod-Shift-8`                            | `-`, `*`, `+`          |
| Ordered list    | `Mod-Shift-7`                            | `1.`                   |
| Task list       | `Mod-Shift-9`                            | `[ ]`, `[x]`           |
| List items      | `Enter`, `Tab`, `Shift-Tab`, `Backspace` |                        |
| Hard break      | `Shift-Enter`, `Mod-Enter`               |                        |
| Horizontal rule |                                          | `---`                  |
| History         | `Mod-z`, `Mod-Shift-z`, `Mod-y`          |                        |

Block typing shortcuts fire when you type a space after them at the start of a line: `##` then space makes a heading. Mark shortcuts fire when you type the closing characters.

Every engine also has the standard editing keys: arrows with Shift, Alt, and Mod to move and select by character, word, and line; `Mod-a`; `Backspace` and `Delete` by character, word (`Alt`), and line (`Mod`); and the Emacs keys macOS users expect (`Ctrl-a`, `Ctrl-e`, `Ctrl-k`, `Ctrl-h`, `Ctrl-d`).

Pressing Backspace right after a typing shortcut undoes it, and puts back what you typed.

## Send keys yourself

The editor hosts send keys for you. On a server or in a test, send them with `press` and type with `typeText`, which runs typing shortcuts as it goes:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.create();

doc.typeText("Plan");
doc.press("Mod-Alt-1");
doc.press("Enter");
doc.typeText("- one");
doc.press("Enter");
doc.typeText("two");
doc.press("Tab");

console.log(doc.export("markdown"));
// # Plan
//
// - one
//   - two
```

`press` returns `false` when no shortcut handled the key, so a host can let the platform handle it instead. An unknown key name fails with the code `invalidKey`.

## Show shortcuts in your UI

Put the shortcut in each button's tooltip, rendered for the user's platform:

```ts
/** `Mod-Shift-z` as the user's platform shows it: `⌘⇧Z` on a Mac, `Ctrl+Shift+Z` elsewhere. */
function shortcutLabel(key: string, mac: boolean): string {
	const parts = key.split("-");
	const name = parts.pop()!.toUpperCase();
	const symbols: Record<string, [string, string]> = {
		Mod: ["⌘", "Ctrl"],
		Shift: ["⇧", "Shift"],
		Alt: ["⌥", "Alt"],
		Ctrl: ["⌃", "Ctrl"],
	};
	const modifiers = parts.map((part) => symbols[part]?.[mac ? 0 : 1] ?? part);
	return mac ? [...modifiers, name].join("") : [...modifiers, name].join("+");
}

console.log(shortcutLabel("Mod-Shift-z", true));
// ⌘⇧Z
console.log(shortcutLabel("Mod-Shift-z", false));
// Ctrl+Shift+Z
```

## Add your own shortcut

Shortcuts belong to extensions. To add one, write an extension in Rust with a `keymap`. [Build your own editor](../extensions/build-your-own-editor.md#add-your-own-extension) has a complete example that binds `Mod-Alt-s`.
