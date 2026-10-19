/**
 * The Wet Caret and the captions it writes.
 */

import type { CSSProperties } from "react";
import { useCurrentFrame } from "remotion";

import { DISPLAY, PAINT_ON_DARK, PAPER } from "../brand";
import { caretOn } from "./time";

/** The caret from the mark master, in its own 64-unit view box. */
const CARET_PATH =
	"M46 10.3A3.5 3.5 0 0 1 53 10.3V47.8C53 50.1 54 51.5 54 54.3A2.85 2.85 0 1 1 48.3 54.3C48.3 52.4 49.6 51.9 49.6 50.4C49.6 49.1 48.9 48.7 47.9 48.3C46.7 47.8 46 47.1 46 45.8Z";
/** The caret's bounds in the master: x 46 to 54, y 6.8 to 57.15. */
const CARET_BOX = { x: 45.4, y: 6.8, width: 9.2, height: 50.4 } as const;

/** The Wet Caret alone, `height` pixels tall. */
export function WetCaret(
	{ height, color, style }: {
		height: number;
		color: string;
		style?: CSSProperties;
	},
) {
	const width = (height * CARET_BOX.width) / CARET_BOX.height;

	return (
		<svg
			width={width}
			height={height}
			viewBox={`${CARET_BOX.x} ${CARET_BOX.y} ${CARET_BOX.width} ${CARET_BOX.height}`}
			style={{ display: "block", overflow: "visible", ...style }}
		>
			<path d={CARET_PATH} fill={color} />
		</svg>
	);
}

export interface TypedTextProps {
	readonly text: string;
	/** The frame typing starts, relative to the scene. */
	readonly start: number;
	/** Characters per second. */
	readonly rate?: number;
	/** The frame the text starts being deleted, if it is. */
	readonly erase?: number;
	readonly size: number;
	readonly color?: string;
	readonly caretColor?: string;
	/** Keep the caret after typing finishes. */
	readonly keepCaret?: boolean;
	readonly weight?: number;
	readonly family?: string;
	readonly style?: CSSProperties;
}

/** How many characters of a caption are showing at `frame`. */
export function typedLength(
	text: string,
	frame: number,
	start: number,
	rate: number,
	erase?: number,
): number {
	const typed = Math.max(
		0,
		Math.min(text.length, Math.floor(((frame - start) * rate) / 30)),
	);

	if (erase === undefined || frame < erase) {
		return typed;
	}

	// Backspace held down: deletes accelerate.
	const held = frame - erase;
	return Math.max(0, typed - Math.floor(held * (1 + held / 6)));
}

/** A caption written by a caret, a character at a time. */
export function TypedText({
	text,
	start,
	rate = 28,
	erase,
	size,
	color = PAPER,
	caretColor = PAINT_ON_DARK,
	keepCaret = false,
	weight = 700,
	family = DISPLAY,
	style,
}: TypedTextProps) {
	const frame = useCurrentFrame();

	if (frame < start) {
		return null;
	}

	const shown = typedLength(text, frame, start, rate, erase);
	const finishedAt = start + Math.ceil((text.length * 30) / rate);
	const typing = shown < text.length || (erase !== undefined && frame >= erase);
	const lastMoved = typing ? frame : finishedAt;
	const visible = (typing || keepCaret || frame < finishedAt + 45) &&
		caretOn(frame, lastMoved);
	const caretHeight = size * 0.92;

	return (
		<div
			style={{
				fontFamily: family,
				fontWeight: weight,
				fontSize: size,
				lineHeight: 1.04,
				letterSpacing: "-0.02em",
				color,
				whiteSpace: "pre-wrap",
				...style,
			}}
		>
			{text.slice(0, shown)}
			<span
				style={{
					display: "inline-block",
					width: 0,
					height: caretHeight,
					verticalAlign: `-${size * 0.12}px`,
					overflow: "visible",
				}}
			>
				<WetCaret
					height={caretHeight}
					color={caretColor}
					style={{
						marginLeft: size * 0.04,
						visibility: visible ? "visible" : "hidden",
					}}
				/>
			</span>
		</div>
	);
}
