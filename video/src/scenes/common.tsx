/**
 * Pieces every scene shares: the stage, the caption a caret writes, and the
 * Paper sheet documents sit on.
 */

import type { CSSProperties, ReactNode } from "react";
import { AbsoluteFill } from "remotion";

import { INK, MONO, PAINT_ON_DARK, PAPER } from "../brand";
import type { Footage } from "../data";
import type { Region, Stage } from "../layout";
import { TypedText } from "../motion/Caret";

export interface SceneProps {
	readonly footage: Footage;
	readonly stage: Stage;
	readonly duration: number;
}

/** How long before a scene ends its caption starts being deleted. */
export const ERASE_BEFORE = 22;

export function Scene(
	{ children, style }: { children: ReactNode; style?: CSSProperties },
) {
	return (
		<AbsoluteFill style={{ background: INK, overflow: "hidden", ...style }}>
			{children}
		</AbsoluteFill>
	);
}

export function at(region: Region): CSSProperties {
	return {
		position: "absolute",
		left: region.x,
		top: region.y,
		width: region.width,
	};
}

/** A small mono label, typed in. */
export function Kicker(
	{ text, start, erase }: { text: string; start: number; erase?: number },
) {
	return (
		<TypedText
			text={text}
			start={start}
			rate={40}
			size={22}
			weight={500}
			family={MONO}
			color={PAINT_ON_DARK}
			caretColor="transparent"
			{...(erase === undefined ? {} : { erase })}
			style={{
				letterSpacing: "0.08em",
				textTransform: "uppercase",
				marginBottom: 28,
			}}
		/>
	);
}

/** A scene's caption: kicker and headline, typed in and deleted at the end. */
export function Caption({
	stage,
	duration,
	kicker,
	headline,
	children,
}: {
	stage: Stage;
	duration: number;
	kicker: string;
	headline: string;
	children?: ReactNode;
}) {
	const erase = duration - ERASE_BEFORE;

	return (
		<div style={at(stage.caption)}>
			<Kicker text={kicker} start={4} erase={erase} />
			<TypedText
				text={headline}
				start={10}
				size={stage.headline}
				erase={erase}
			/>
			{children}
		</div>
	);
}

/** The shortest a sheet gets, so an empty page still reads as a page. */
const MIN_SHEET = 260;

/**
 * The Paper sheet in the stage's sheet region, with the page inset. Given
 * the page's height, the sheet grows with it.
 */
export function Sheet(
	{ stage, children, page }: {
		stage: Stage;
		children: ReactNode;
		page?: number;
	},
) {
	const height = page === undefined
		? stage.sheet.height
		: Math.min(stage.sheet.height, Math.max(MIN_SHEET, page + stage.inset * 2));

	return (
		<div
			style={{
				...at(stage.sheet),
				height,
				background: PAPER,
				borderRadius: 28,
				padding: stage.inset,
				boxSizing: "border-box",
				overflow: "hidden",
			}}
		>
			{children}
		</div>
	);
}
