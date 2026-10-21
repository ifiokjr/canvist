# Add a toolbar

In this guide you build a formatting toolbar: buttons that run commands, show when they're active, and disable themselves when they can't apply. canvist has no built-in toolbar, so it looks exactly like your app.

## The three calls behind every button

| Button behaviour             | Call                        |
| ---------------------------- | --------------------------- |
| Clicking runs the command    | `doc.commands.toggleBold()` |
| Pressed when active          | `doc.query.isBold()`        |
| Disabled when it can't apply | `doc.can.toggleBold()`      |

Refresh the toolbar after every change and every selection change. Queries and `can` checks cost about a microsecond each, so refreshing every button on every keystroke is fine.

## Describe the buttons once

Write the toolbar as data. The types come from your engine, so a button for an extension you didn't register won't compile:

```ts
import {
	bold,
	bulletList,
	createEngine,
	heading,
	italic,
} from "@canvist/headless";

const engine = await createEngine([bold, italic, heading, bulletList]);
type Editor = ReturnType<typeof engine.create>;

interface Button {
	label: string;
	run(doc: Editor): boolean;
	can(doc: Editor): boolean;
	active(doc: Editor): boolean;
}

const buttons: Button[] = [
	{
		label: "Bold",
		run: (doc) => doc.commands.toggleBold(),
		can: (doc) => doc.can.toggleBold(),
		active: (doc) => doc.query.isBold(),
	},
	{
		label: "Italic",
		run: (doc) => doc.commands.toggleItalic(),
		can: (doc) => doc.can.toggleItalic(),
		active: (doc) => doc.query.isItalic(),
	},
	{
		label: "Heading 2",
		run: (doc) => doc.commands.toggleHeading({ level: 2 }),
		can: (doc) => doc.can.toggleHeading({ level: 2 }),
		active: (doc) => doc.query.headingLevel() === 2,
	},
	{
		label: "Bullets",
		run: (doc) => doc.commands.toggleBulletList(),
		can: (doc) => doc.can.toggleBulletList(),
		active: (doc) => doc.query.isBulletList(),
	},
];

/** The toolbar as text: [x] for active, ( ) for disabled. */
function describe(doc: Editor): string {
	return buttons
		.map((button) => {
			const state = button.active(doc)
				? "[x]"
				: button.can(doc)
				? "[ ]"
				: "( )";
			return `${state} ${button.label}`;
		})
		.join("  ");
}

const doc = engine.parse("markdown", "## Plan\n\nShip **today**");
doc.select({ path: [1], offset: 6 });
console.log(describe(doc));
// [x] Bold  [ ] Italic  [ ] Heading 2  [ ] Bullets

doc.select({ path: [0], offset: 2 });
console.log(describe(doc));
// [ ] Bold  [ ] Italic  [x] Heading 2  [ ] Bullets
```

## Render it in your UI

The same descriptions drive real buttons. Here they are as plain DOM buttons over a `@canvist/web` editor, refreshed through `subscribe`, which calls back after every edit, selection change, and focus change:

```ts no_run
import { bold, createEditor, italic } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor")!;
const toolbar = document.querySelector<HTMLElement>("#toolbar")!;
const editor = await createEditor(element, {
	extensions: [bold, italic],
	content: {
		format: "markdown",
		input: "Select some text, then press a button.",
	},
});

const buttons = [
	{
		label: "B",
		run: () => editor.commands.toggleBold(),
		can: () => editor.can.toggleBold(),
		active: () => editor.query.isBold(),
	},
	{
		label: "I",
		run: () => editor.commands.toggleItalic(),
		can: () => editor.can.toggleItalic(),
		active: () => editor.query.isItalic(),
	},
];

const elements = buttons.map((button) => {
	const element = document.createElement("button");
	element.type = "button";
	element.textContent = button.label;
	element.addEventListener("click", () => {
		button.run();
		editor.focus();
	});
	return element;
});
toolbar.replaceChildren(...elements);

function refresh(): void {
	buttons.forEach((button, index) => {
		const element = elements[index]!;
		element.disabled = !editor.editable || !button.can();
		element.setAttribute("aria-pressed", String(button.active()));
	});
}

editor.subscribe(refresh);
refresh();
```

Use `aria-pressed` for toggle buttons, so screen readers announce their state.

## Menus over the selection

A toolbar can also float: `bubbleMenu` from `@canvist/web/menus` keeps an element over selected text, and `floatingMenu` beside an empty line, as TipTap's menus do. React and Vue have `BubbleMenu` and `FloatingMenu` components. See [the web quick start](../quick-start/web.md#6-menus-that-follow-the-selection).

## Dropdowns and dialogs

- **A block-type dropdown** shows `doc.query.headingLevel()`, or "Paragraph" when it's `null` and `doc.query.isParagraph()` is `true`.
- **A link dialog** prefills its field from `doc.query.linkHref()`, then runs `doc.commands.setLink({ href })` or `doc.commands.unsetLink()`.

## In Flutter

The pattern is the same in Dart. Keep the handles from `Engine.starterKit()`, and rebuild the toolbar after each change:

```dart
import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

class BoldButton extends StatelessWidget {
  const BoldButton({super.key, required this.doc, required this.bold, required this.onChanged});

  final Doc doc;
  final BoldHandle bold;
  final VoidCallback onChanged;

  @override
  Widget build(BuildContext context) {
    final active = doc.query(bold.isBold());
    return IconButton(
      icon: const Icon(Icons.format_bold),
      isSelected: active,
      onPressed: doc.can(bold.toggleBold())
          ? () {
              doc.run(bold.toggleBold());
              onChanged();
            }
          : null,
    );
  }
}
```

## Next

- [Keyboard shortcuts](./keyboard-shortcuts.md): show each button's shortcut in its tooltip.
- [Extension catalogue](../extensions/readme.md): every command and query you can bind.
