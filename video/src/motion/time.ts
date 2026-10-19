/**
 * Timing for footage: when each captured frame of a reel appears in the
 * film, and the "wet" springs that carry things to their new positions.
 *
 * The Wet Caret motion language:
 *
 * - **Typed, never faded.** Text arrives a character at a time and leaves
 *   by being deleted. Nothing in the film tweens its opacity.
 * - **The caret leads.** Scenes change behind a sweeping caret, and every
 *   caption is written by one.
 * - **Wet, then dry.** Anything that moves lands with a little overshoot,
 *   like paint settling, then stays put.
 * - **Real geometry.** Blocks and carets move between the rectangles
 *   layout computed; the film only chooses when.
 */

import { spring } from "remotion";

import type { Frame } from "../data";

export const FPS = 30;

/** Paint settling: quick, with a small overshoot. */
export const WET = { damping: 13, mass: 0.7, stiffness: 170 } as const;
/** A caret gliding: stiff, barely overshooting. */
export const GLIDE = { damping: 26, mass: 0.5, stiffness: 420 } as const;

/**
 * The film frame at which each captured frame appears, given how many
 * beats each one deserves, scaled so the last appears at `span`.
 */
export function schedule(
	frames: readonly Frame[],
	beats: (frame: Frame, index: number) => number,
	span: number,
): number[] {
	const raw: number[] = [];
	let at = 0;

	frames.forEach((frame, index) => {
		raw.push(at);
		at += beats(frame, index);
	});

	const last = raw[raw.length - 1] ?? 0;
	const scale = last === 0 ? 1 : span / last;
	return raw.map((value) => value * scale);
}

/** The index of the captured frame showing at film frame `frame`. */
export function indexAt(starts: readonly number[], frame: number): number {
	let low = 0;
	let high = starts.length - 1;

	while (low < high) {
		const middle = Math.ceil((low + high) / 2);

		if ((starts[middle] ?? Infinity) <= frame) {
			low = middle;
		} else {
			high = middle - 1;
		}
	}

	return low;
}

export interface Change {
	/** The film frame the value changed at. */
	readonly at: number;
	readonly value: number;
}

/**
 * A value that springs to each new target from wherever the previous
 * spring had carried it, evaluated at `frame`.
 */
export function settle(
	changes: readonly Change[],
	frame: number,
	config: { damping: number; mass: number; stiffness: number } = WET,
): number {
	const first = changes[0];

	if (!first) {
		return 0;
	}

	let from = first.value;
	let to = first.value;
	let since = first.at;

	for (const change of changes.slice(1)) {
		if (change.at > frame) {
			break;
		}

		from = from +
			(to - from) * spring({ frame: change.at - since, fps: FPS, config });
		to = change.value;
		since = change.at;
	}

	return from +
		(to - from) * spring({ frame: frame - since, fps: FPS, config });
}

/**
 * Whether a caret is showing: solid for half a second after it last moved,
 * then blinking on a one-second cycle in hard steps, as the brand asks.
 */
export function caretOn(frame: number, lastMoved: number): boolean {
	const idle = frame - lastMoved;

	if (idle < FPS / 2) {
		return true;
	}

	return Math.floor((idle - FPS / 2) / (FPS / 2)) % 2 === 1;
}
