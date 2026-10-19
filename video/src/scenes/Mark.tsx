/**
 * The Wet Caret paints itself in: the "c" is brushed on, the caret runs
 * down and its one drop falls, then the caret types the rest of the
 * wordmark and a tagline.
 */

import { interpolate, spring, useCurrentFrame } from "remotion";

import { PAINT_ON_DARK, PAPER, PAPER_MUTED } from "../brand";
import { TypedText } from "../motion/Caret";
import { caretOn, FPS, WET } from "../motion/time";
import { ERASE_BEFORE, Scene, type SceneProps } from "./common";

/**
 * Where each letter of the lockup ends, in master units, so the caret can
 * stop after it: c, a, n, v, i, s, t.
 */
const LETTER_ENDS = [39.9, 75.8, 111.4, 149, 164.4, 197.6, 224.6] as const;
/** The lockup caret's left edge in the master. */
const CARET_LEFT = 227.1;
/** Gap between a letter and the caret, in master units. */
const CARET_GAP = 2.6;

/**
 * The brush's path around the c's bowl in lockup units: from the upper
 * terminal, counter-clockwise over the top and round to the lower one.
 */
const BOWL = "M41 16 L33.5 22.5 A14 14 0 1 0 33.5 43.5 L42 50";

/** Where the caret's bar ends and its drop begins, in lockup units. */
const DRIP_FROM = 52.4;

const BRUSH = 26;
const RUN = { start: 24, length: 12 } as const;
const DROP = 36;
const TYPE = { start: 58, step: 3 } as const;
const TAGLINE = 84;

export function Mark({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const { viewBox, ink, paint } = footage.brand.lockup;
	const [, , boxWidth = 240, boxHeight = 64] = viewBox.split(" ").map(Number);
	const width = stage.width * (stage.width > stage.height ? 0.5 : 0.78);
	const scale = width / boxWidth;
	const height = boxHeight * scale;

	// The brush: a thick stroke around the c's bowl, from its upper
	// terminal counter-clockwise to its lower one.
	const brushed = interpolate(frame, [0, BRUSH], [0, 1], {
		extrapolateRight: "clamp",
		easing: (t) => 1 - (1 - t) ** 3,
	});

	// The caret runs down from its rounded top, then its drop falls once.
	const run = interpolate(frame, [RUN.start, RUN.start + RUN.length], [
		6.5,
		DRIP_FROM,
	], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
	const drop = frame < DROP
		? 0
		: spring({ frame: frame - DROP, fps: FPS, config: WET });

	const letters = Math.max(
		1,
		Math.min(
			LETTER_ENDS.length,
			1 + Math.floor((frame - TYPE.start) / TYPE.step),
		),
	);
	const typedEnd = LETTER_ENDS[letters - 1] ?? LETTER_ENDS[0];
	const finished = letters === LETTER_ENDS.length;
	const caretX = finished ? 0 : typedEnd + CARET_GAP - CARET_LEFT;
	const lastMoved = frame < TYPE.start
		? DROP
		: TYPE.start + (letters - 1) * TYPE.step;
	const showCaret = frame < RUN.start + RUN.length + 20 ||
		caretOn(frame, lastMoved);
	const erase = duration - ERASE_BEFORE;

	return (
		<Scene>
			<div
				style={{
					position: "absolute",
					left: (stage.width - width) / 2,
					top: stage.height * 0.5 - height * 0.85,
				}}
			>
				<svg
					width={width}
					height={height}
					viewBox={viewBox}
					style={{ overflow: "visible" }}
				>
					<defs>
						<mask id="brush" maskUnits="userSpaceOnUse">
							<path
								d={BOWL}
								fill="none"
								stroke="white"
								strokeWidth={17}
								pathLength={1}
								strokeDasharray={`${brushed} 1`}
							/>
							<rect
								x={LETTER_ENDS[0]}
								y={0}
								width={boxWidth}
								height={boxHeight}
								fill="white"
							/>
						</mask>
						<clipPath id="typed">
							<rect x={0} y={0} width={typedEnd} height={boxHeight} />
						</clipPath>
						<clipPath id="run">
							<rect x={0} y={0} width={boxWidth + 20} height={run} />
						</clipPath>
						<clipPath id="drip">
							<rect
								x={0}
								y={DRIP_FROM}
								width={boxWidth + 20}
								height={boxHeight}
							/>
						</clipPath>
					</defs>
					<path
						d={ink}
						fill={PAPER}
						mask={brushed < 1 ? "url(#brush)" : undefined}
						clipPath="url(#typed)"
					/>
					{showCaret && (
						<g transform={`translate(${caretX} 0)`}>
							<path d={paint} fill={PAINT_ON_DARK} clipPath="url(#run)" />
							{frame >= DROP && (
								<g transform={`translate(0 ${(1 - drop) * -6})`}>
									<path d={paint} fill={PAINT_ON_DARK} clipPath="url(#drip)" />
								</g>
							)}
						</g>
					)}
				</svg>
			</div>
			<div
				style={{
					position: "absolute",
					left: 0,
					right: 0,
					top: stage.height * 0.5 + height * 0.45,
					display: "flex",
					justifyContent: "center",
				}}
			>
				<TypedText
					text="The rich-text editor that paints every pixel."
					start={TAGLINE}
					rate={40}
					size={Math.round(stage.headline * 0.62)}
					weight={500}
					color={PAPER_MUTED}
					erase={erase}
				/>
			</div>
		</Scene>
	);
}
