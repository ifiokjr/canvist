/**
 * Dragging the code block to the top. Every slot it passes is a real
 * `MoveBlocks` commit, relaid out, so the other blocks make room exactly
 * where layout puts them.
 */

import { useMemo } from "react";
import { Easing, interpolate, spring, useCurrentFrame } from "remotion";

import { INK, PAINT, PAPER } from "../brand";
import type { Box, Reel } from "../data";
import { Page, usePageHeight } from "../motion/Page";
import { FPS, WET } from "../motion/time";
import { Caption, Scene, type SceneProps, Sheet } from "./common";

const ENTER = { start: 22, end: 44 } as const;
const GRAB = 50;
const DROP = 118;
const LIFT = 1.025;

interface Drag {
	readonly id: string;
	/** The dragged block's box in each captured frame. */
	readonly slots: readonly Box[];
}

function dragOf(reel: Reel): Drag {
	const id = reel.meta.dragging;

	if (typeof id !== "string") {
		throw new Error(
			"footage: drag.meta.dragging is missing; rerun npm run prepare:footage",
		);
	}

	const slots = reel.frames.map((frame) => {
		const layer = frame.blocks.find((block) => block.id === id);

		if (!layer) {
			throw new Error(
				`footage: the dragged block ${id} is missing from a frame`,
			);
		}

		return layer.box;
	});

	return { id, slots };
}

const centre = (box: Box) => box[1] + box[3] / 2;

export function DragScene({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const reel = footage.drag;
	const drag = useMemo(() => dragOf(reel), [reel]);
	const first = drag.slots[0] ?? [0, 0, 0, 0];
	const last = drag.slots[drag.slots.length - 1] ?? first;

	// The pointer grabs the block by its left edge and carries it up.
	const grabOffset = first[3] / 2;
	const pointerY = (at: number) =>
		interpolate(at, [GRAB, DROP], [centre(first), centre(last)], {
			extrapolateLeft: "clamp",
			extrapolateRight: "clamp",
			easing: Easing.inOut(Easing.cubic),
		});

	// Each captured frame shows from the moment the pointer is nearest its slot.
	const starts = useMemo(() => {
		const result = drag.slots.map(() => Infinity);
		result[0] = 0;

		for (let at = 0; at <= duration; at += 1) {
			const y = pointerY(at);
			let nearest = 0;

			drag.slots.forEach((slot, index) => {
				if (
					Math.abs(centre(slot) - y) <
						Math.abs(centre(drag.slots[nearest] ?? slot) - y)
				) {
					nearest = index;
				}
			});

			if (result[nearest] === Infinity) {
				result[nearest] = at;
			}
		}

		return result.map((
			value,
			index,
		) => (value === Infinity ? (result[index - 1] ?? 0) : value));
	}, [drag, duration]);

	const pageHeight = usePageHeight(reel, starts, frame);
	const held = frame >= GRAB && frame < DROP;
	const landed = frame >= DROP
		? spring({ frame: frame - DROP, fps: FPS, config: WET })
		: 0;
	const lift = held
		? spring({ frame: frame - GRAB, fps: FPS, config: WET, from: 1, to: LIFT })
		: interpolate(landed, [0, 1], [LIFT, 1]);
	const blockY = frame < GRAB
		? first[1]
		: frame < DROP
		? pointerY(frame) - grabOffset
		: interpolate(landed, [0, 1], [pointerY(DROP) - grabOffset, last[1]]);

	const index = starts.filter((at) => at <= frame).length - 1;
	const svgIndex =
		reel.frames[index]?.blocks.find((block) => block.id === drag.id)?.svg ?? 0;
	const pointer = pointerPosition(frame, first, blockY + grabOffset);

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="03 · move"
				headline="Drag a block. The rest make room."
			/>
			<Sheet stage={stage} page={pageHeight}>
				<Page
					reel={reel}
					starts={starts}
					frame={frame}
					caret={false}
					lift={drag.id}
				>
					<div
						style={{
							position: "absolute",
							left: first[0],
							top: blockY,
							width: first[2],
							height: first[3],
							transform: `scale(${lift})`,
							transformOrigin: "0% 50%",
							outline: frame >= GRAB && landed < 0.6
								? `3px solid ${PAINT}`
								: "none",
							outlineOffset: 6,
							borderRadius: 12,
						}}
						dangerouslySetInnerHTML={{ __html: reel.svgs[svgIndex] ?? "" }}
					/>
					{frame >= ENTER.start && frame < DROP + 12 && (
						<Grip y={blockY + grabOffset} />
					)}
					<Pointer x={pointer.x} y={pointer.y} pressed={held} />
				</Page>
			</Sheet>
		</Scene>
	);
}

function pointerPosition(
	frame: number,
	first: Box,
	heldY: number,
): { x: number; y: number } {
	const restX = first[0] + first[2] * 0.62;
	const restY = first[1] + first[3] + 160;
	const grabX = -22;

	if (frame < ENTER.end) {
		const t = interpolate(frame, [ENTER.start, ENTER.end], [0, 1], {
			extrapolateLeft: "clamp",
			extrapolateRight: "clamp",
			easing: Easing.out(Easing.cubic),
		});
		return {
			x: interpolate(t, [0, 1], [restX, grabX]),
			y: interpolate(t, [0, 1], [restY, heldY]),
		};
	}

	if (frame < DROP + 10) {
		return { x: grabX, y: heldY };
	}

	const t = interpolate(frame, [DROP + 10, DROP + 40], [0, 1], {
		extrapolateRight: "clamp",
		easing: Easing.inOut(Easing.cubic),
	});
	return {
		x: interpolate(t, [0, 1], [grabX, restX]),
		y: interpolate(t, [0, 1], [heldY, heldY + 120]),
	};
}

/** The six-dot handle a host draws beside the hovered block. */
function Grip({ y }: { y: number }) {
	return (
		<svg
			width={14}
			height={22}
			viewBox="0 0 14 22"
			style={{ position: "absolute", left: -32, top: y - 11 }}
		>
			{[0, 1, 2].flatMap((row) =>
				[0, 1].map((column) => (
					<circle
						key={`${row}${column}`}
						cx={3 + column * 8}
						cy={3 + row * 8}
						r={2.4}
						fill={PAINT}
					/>
				))
			)}
		</svg>
	);
}

function Pointer(
	{ x, y, pressed }: { x: number; y: number; pressed: boolean },
) {
	return (
		<svg
			width={34}
			height={40}
			viewBox="0 0 17 20"
			style={{
				position: "absolute",
				left: x - 4,
				top: y - 2,
				transform: `scale(${pressed ? 0.9 : 1})`,
				transformOrigin: "0 0",
			}}
		>
			<path
				d="M1 1 L1 16 L5 12.5 L8 19 L10.6 17.8 L7.8 11.5 L13 11.5 Z"
				fill={INK}
				stroke={PAPER}
				strokeWidth={1.2}
				strokeLinejoin="round"
			/>
		</svg>
	);
}
