/**
 * The launch film: nine scenes, each swept in by a caret.
 */

import type { ComponentType, ReactNode } from "react";
import {
	AbsoluteFill,
	Easing,
	interpolate,
	Sequence,
	useCurrentFrame,
} from "remotion";

import { INK, PAINT_ON_DARK } from "./brand";
import { useFootage } from "./data";
import { type Format, type Stage, STAGES } from "./layout";
import { Bench } from "./scenes/Bench";
import { Collab } from "./scenes/Collab";
import type { SceneProps } from "./scenes/common";
import { DragScene } from "./scenes/Drag";
import { End } from "./scenes/End";
import { Format as FormatScene } from "./scenes/Format";
import { Headless } from "./scenes/Headless";
import { Mark } from "./scenes/Mark";
import { Everywhere, Reflow } from "./scenes/Platforms";
import { Typing } from "./scenes/Typing";

/** How long the caret takes to sweep one scene over the last. */
const WIPE = 20;

interface SceneSpec {
	readonly id: string;
	readonly duration: number;
	readonly Component: ComponentType<SceneProps & { format: Format }>;
}

const SCENES: readonly SceneSpec[] = [
	{ id: "mark", duration: 180, Component: Mark },
	{ id: "typing", duration: 300, Component: Typing },
	{ id: "format", duration: 320, Component: FormatScene },
	{ id: "drag", duration: 180, Component: DragScene },
	{ id: "collab", duration: 270, Component: Collab },
	{ id: "reflow", duration: 180, Component: Reflow },
	{ id: "everywhere", duration: 170, Component: Everywhere },
	{ id: "headless", duration: 190, Component: Headless },
	{ id: "bench", duration: 170, Component: Bench },
	{ id: "end", duration: 190, Component: End },
];

/** The film's length in frames. */
export const FILM_FRAMES =
	SCENES.reduce((total, scene) => total + scene.duration, 0) -
	WIPE * (SCENES.length - 1);

export function Film({ format }: { format: Format }) {
	const stage = STAGES[format];
	const footage = useFootage(stage.profile);

	if (!footage) {
		return <AbsoluteFill style={{ background: INK }} />;
	}

	let from = 0;

	return (
		<AbsoluteFill style={{ background: INK }}>
			{SCENES.map(({ id, duration, Component }, index) => {
				const start = from;
				from += duration - WIPE;

				return (
					<Sequence key={id} name={id} from={start} durationInFrames={duration}>
						<CaretSweep stage={stage} enabled={index > 0}>
							<Component
								footage={footage}
								stage={stage}
								duration={duration}
								format={format}
							/>
						</CaretSweep>
					</Sequence>
				);
			})}
		</AbsoluteFill>
	);
}

/**
 * Reveals a scene behind a caret sweeping left to right, as if the new
 * scene were being typed over the old one.
 */
function CaretSweep(
	{ stage, enabled, children }: {
		stage: Stage;
		enabled: boolean;
		children: ReactNode;
	},
) {
	const frame = useCurrentFrame();

	if (!enabled || frame >= WIPE) {
		return <AbsoluteFill>{children}</AbsoluteFill>;
	}

	const progress = interpolate(frame, [0, WIPE], [0, 1], {
		extrapolateRight: "clamp",
		easing: Easing.bezier(0.65, 0, 0.2, 1),
	});
	const edge = progress * (stage.width + 60);
	const width = Math.round(stage.width / 80);

	return (
		<AbsoluteFill>
			<AbsoluteFill
				style={{ clipPath: `inset(0 ${stage.width - edge}px 0 0)` }}
			>
				{children}
			</AbsoluteFill>
			<div
				style={{
					position: "absolute",
					top: -width,
					left: edge - width,
					width,
					height: stage.height + width * 2,
					background: PAINT_ON_DARK,
					borderRadius: width / 2,
				}}
			/>
		</AbsoluteFill>
	);
}
