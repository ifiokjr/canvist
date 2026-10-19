/**
 * The parts of the EditContext API the editor uses.
 *
 * EditContext (Chromium 121+) lets a canvas receive text input, IME
 * composition, and the platform's text services as if it were a text field,
 * without a hidden element. TypeScript's DOM library doesn't declare it
 * yet, so these declarations cover what `input.ts` calls.
 *
 * @see https://w3c.github.io/edit-context/
 * @module
 */

/** Text input arriving at an {@link EditContext}. */
export interface TextUpdateEvent extends Event {
	/** The UTF-16 range of the context's text being replaced. */
	readonly updateRangeStart: number;
	readonly updateRangeEnd: number;
	/** The replacement. */
	readonly text: string;
	/** The selection afterwards. */
	readonly selectionStart: number;
	readonly selectionEnd: number;
}

/** The platform asks where the characters in a range are drawn. */
export interface CharacterBoundsUpdateEvent extends Event {
	readonly rangeStart: number;
	readonly rangeEnd: number;
}

interface EditContextEvents {
	textupdate: TextUpdateEvent;
	characterboundsupdate: CharacterBoundsUpdateEvent;
	compositionstart: CompositionEvent;
	compositionend: CompositionEvent;
}

/** A text model the platform's input methods edit. */
export interface EditContext extends EventTarget {
	readonly text: string;
	readonly selectionStart: number;
	readonly selectionEnd: number;
	updateText(start: number, end: number, text: string): void;
	updateSelection(start: number, end: number): void;
	updateControlBounds(bounds: DOMRect): void;
	updateSelectionBounds(bounds: DOMRect): void;
	updateCharacterBounds(start: number, bounds: DOMRect[]): void;
	addEventListener<K extends keyof EditContextEvents>(
		type: K,
		listener: (event: EditContextEvents[K]) => void,
		options?: AddEventListenerOptions,
	): void;
	addEventListener(
		type: string,
		listener: EventListenerOrEventListenerObject,
		options?: boolean | AddEventListenerOptions,
	): void;
}

interface EditContextInit {
	readonly text?: string;
	readonly selectionStart?: number;
	readonly selectionEnd?: number;
}

type EditContextConstructor = new (init?: EditContextInit) => EditContext;

/** An element that takes an {@link EditContext}. */
export type EditContextHost = HTMLElement & {
	editContext: EditContext | null;
};

/** The browser's EditContext constructor, where it has one. */
export function editContextApi(): EditContextConstructor | undefined {
	const scope = globalThis as typeof globalThis & {
		readonly EditContext?: EditContextConstructor;
	};

	return scope.EditContext;
}
