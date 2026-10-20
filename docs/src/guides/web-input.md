# Text input on the web

In this guide you learn how the browser editor receives typing, input methods, the clipboard, and drops, how to switch input paths when a browser misbehaves, and how screen readers see a canvas.

## How text arrives

The editor paints on a canvas, which can't hold text. It receives text through one of two paths:

- **`EditContext`**, in Chromium-based browsers. The canvas takes focus and tells the platform the paragraph you are editing and where the caret is. Input methods, dictation, autocorrect, and the emoji picker then work as they do in a text field, and their windows open at the caret.
- **A hidden textarea**, everywhere else. A transparent textarea follows the caret and takes the focus. Typed text, compositions, and edits from on-screen keyboards are read from its events, and the textarea is emptied after each one.

Both paths run your key bindings first. A key without a binding becomes text. Every change is laid out and painted in the same task as the event that caused it.

`editor.inputMode` says which path is in use. To force the textarea, for example while a browser has an `EditContext` bug, pass `input: "textarea"`:

```ts no_run
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element, {
		input: "textarea",
		label: "Post body",
	});
	console.log(editor.inputMode); // "textarea"
}
```

## Keys follow the platform

`Mod` in key bindings is Cmd on macOS and iOS and Ctrl elsewhere, detected from the browser. Pass `platform: "apple"` or `platform: "other"` to override the detection. [Keyboard shortcuts](./keyboard-shortcuts.md) lists every binding.

## Composition, the clipboard, and drops

- **Input methods.** Preedit text from Japanese, Chinese, or Korean input methods, and from dead keys such as Option-E on a Mac, appears in the document as you compose and is replaced by the committed text. Key bindings wait until the composition ends.
- **The clipboard.** Copy and cut put HTML and plain text on the clipboard. Paste reads HTML when there is some, and plain text otherwise, so formatting survives between canvist editors, web pages, and word processors.
- **Pointer selection.** Click to place the caret, Shift-click to extend, double-click for a word, triple-click for the paragraph. Dragging after a double or triple click extends by words or paragraphs.
- **Drag and drop.** Drag the selection to move it, holding Option (Mac) or Ctrl (elsewhere) to copy it instead. Text and HTML dropped from other applications land where you drop them.

## Platform quirks it handles

These sequences are replayed in Chromium, Firefox, and WebKit on every change (`packages/canvist-web/tests/browser/input-edge.spec.ts`):

- **Safari's input methods.** Composed text is inserted once, whether Safari's final `insertFromComposition` arrives before or after `compositionend`, and the Enter that commits a composition (key code 229) doesn't split the paragraph.
- **Android keyboards.** Gboard and other soft keyboards send every key as `Unidentified` and say what they mean in `beforeinput`: typing, Backspace, Enter, undo, and redo all go through it, and words composed one at a time arrive whole.
- **Touch.** A tap places the caret and focuses the text field, so the on-screen keyboard opens.

## Screen readers

Beside the canvas, the editor keeps a visually hidden copy of the document as the same semantic HTML the server renders: headings, lists, links, and code. Screen readers read it, and so does find-in-page. It is refreshed when the browser is idle after a change, not on every keystroke. The focusable element has the `textbox` role and the name you pass as `label`, comes before the copy in reading order, and announces `aria-readonly` and the placeholder.

Every change runs an [axe](https://github.com/dequelabs/axe-core) audit (WCAG 2.2 AA) of editable, read-only, empty, and menu-showing editors, and smoke tests that read the editor with a virtual screen reader (`packages/canvist-web/tests/browser/a11y.spec.ts`). The placeholder has 4.5:1 contrast on the StarterKit's light and dark pages.

The copy does not yet mirror the selection, so a screen reader can read the document but doesn't announce the caret as it moves.

## Measure input latency

`editor.inputLatency()` summarizes the most recent 1,024 input events: the time from each event's timestamp until the editor had painted the change, in milliseconds. canvist's browser tests hold its 95th percentile under 8 ms while typing, and type 1,000 keys in a burst without dropping one. [Performance](./performance.md) has the numbers.
