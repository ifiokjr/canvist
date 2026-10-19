/**
 * The accessibility mirror: the document as semantic HTML, visually hidden
 * beside the canvas, so screen readers and find-in-page tools can read
 * what the canvas paints.
 *
 * It is the same HTML the server renders, refreshed when the browser is
 * idle after a change rather than on every keystroke.
 *
 * @module
 */

/** How long to wait for an idle moment before refreshing anyway. */
const IDLE_TIMEOUT_MS = 500;

/** A visually hidden copy of the document for assistive technology. */
export class AccessibilityMirror {
	readonly #element: HTMLDivElement;
	readonly #render: () => string;
	#pending = 0;

	/**
	 * A mirror inside `parent`, labelled `label`, whose content `render`
	 * returns as HTML.
	 */
	constructor(parent: HTMLElement, label: string, render: () => string) {
		this.#render = render;
		this.#element = parent.ownerDocument.createElement("div");
		this.#element.className = "cv-mirror";
		this.#element.setAttribute("role", "document");
		this.#element.setAttribute("aria-label", label);
		Object.assign(this.#element.style, {
			position: "absolute",
			width: "1px",
			height: "1px",
			margin: "-1px",
			padding: "0",
			border: "0",
			overflow: "hidden",
			clipPath: "inset(50%)",
			whiteSpace: "nowrap",
		});
		parent.append(this.#element);
		// Serializing a long document costs milliseconds: not before first paint.
		this.schedule();
	}

	/** The document changed: refresh when the browser is next idle. */
	schedule(): void {
		if (this.#pending !== 0) {
			return;
		}

		this.#pending = typeof requestIdleCallback === "function"
			? requestIdleCallback(() => this.refresh(), { timeout: IDLE_TIMEOUT_MS })
			: window.setTimeout(() => this.refresh(), IDLE_TIMEOUT_MS);
	}

	/** Refresh now. */
	refresh(): void {
		this.#cancel();
		this.#element.innerHTML = this.#render();
	}

	dispose(): void {
		this.#cancel();
		this.#element.remove();
	}

	#cancel(): void {
		if (this.#pending === 0) {
			return;
		}

		if (typeof cancelIdleCallback === "function") {
			cancelIdleCallback(this.#pending);
		} else {
			clearTimeout(this.#pending);
		}

		this.#pending = 0;
	}
}
