/**
 * The placeholder: dimmed text shown where the first line starts while the
 * document is empty.
 *
 * It is a DOM element over the canvas, set in the body text's font and
 * line height, so it sits exactly where typed text will appear. Assistive
 * technology reads it from the text field's `aria-placeholder` instead.
 *
 * @module
 */

/**
 * How strongly the placeholder shows the body text's colour: enough for
 * 4.5:1 contrast on the StarterKit's light and dark pages, as WCAG asks of
 * placeholder text.
 */
const PLACEHOLDER_OPACITY = 0.62;

/** Placeholder text over an empty document. */
export class Placeholder {
	readonly #element: HTMLDivElement;

	/**
	 * A placeholder inside `parent` in the body text's CSS `font` and
	 * `color`.
	 */
	constructor(parent: HTMLElement, font: string, color: string) {
		this.#element = parent.ownerDocument.createElement("div");
		this.#element.className = "cv-placeholder";
		this.#element.setAttribute("aria-hidden", "true");
		Object.assign(this.#element.style, {
			position: "absolute",
			display: "none",
			margin: "0",
			font,
			color,
			opacity: String(PLACEHOLDER_OPACITY),
			whiteSpace: "pre",
			overflow: "hidden",
			pointerEvents: "none",
			userSelect: "none",
		});
		parent.append(this.#element);
	}

	/** The text to show. An empty string shows nothing. */
	set text(text: string) {
		this.#element.textContent = text;
	}

	/**
	 * Show the placeholder on the line `line` covers, in document
	 * coordinates, or hide it for `null`.
	 */
	show(line: DOMRectReadOnly | null): void {
		const style = this.#element.style;

		if (line === null || this.#element.textContent === "") {
			style.display = "none";
			return;
		}

		// The caret is centred on where text starts.
		const left = line.x + line.width / 2;
		Object.assign(style, {
			display: "block",
			left: `${left}px`,
			top: `${line.y}px`,
			right: "0",
			lineHeight: `${line.height}px`,
		});
	}

	dispose(): void {
		this.#element.remove();
	}
}
