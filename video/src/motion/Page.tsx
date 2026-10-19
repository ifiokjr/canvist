/**
 * A canvist document as the footage captured it: each top-level block is
 * the SVG `to_svg` painted for it, placed at the rectangle layout gave it.
 * When layout moves a block, the block springs there; when an input rule
 * changes it, it snaps in.
 */

import type { CSSProperties, ReactNode } from "react";
import { useMemo } from "react";
import { spring } from "remotion";

import { PAINT } from "../brand";
import type { Box, Frame, Reel } from "../data";
import { caretOn, type Change, FPS, GLIDE, indexAt, settle, WET } from "./time";

interface Track {
	readonly x: Change[];
	readonly y: Change[];
	/** Film frames at which the block's paint changed shape. */
	readonly snaps: number[];
}

/** Each block's movement history, up to and including captured frame `upTo`. */
function tracks(
	reel: Reel,
	starts: readonly number[],
	upTo: number,
): Map<string, Track> {
	const result = new Map<string, Track>();

	for (let index = 0; index <= upTo; index += 1) {
		const frame = reel.frames[index];
		const at = starts[index] ?? 0;

		if (!frame) {
			break;
		}

		const ruled = frame.extra.kind === "InputRule" ||
			frame.extra.kind === "Structure";

		for (const layer of frame.blocks) {
			const [x, y] = layer.box;
			const track = result.get(layer.id);

			if (!track) {
				result.set(layer.id, {
					x: [{ at, value: x }],
					y: [{ at, value: y }],
					snaps: index === 0 ? [] : [at],
				});
				continue;
			}

			const lastX = track.x[track.x.length - 1];
			const lastY = track.y[track.y.length - 1];

			if (lastX && Math.abs(lastX.value - x) > 0.5) {
				track.x.push({ at, value: x });
			}

			if (lastY && Math.abs(lastY.value - y) > 0.5) {
				track.y.push({ at, value: y });
			}

			const previous = reel.frames[index - 1]?.blocks.find((other) =>
				other.id === layer.id
			);

			if (
				ruled && previous && previous.svg !== layer.svg &&
				Math.abs(previous.box[3] - layer.box[3]) > 1
			) {
				track.snaps.push(at);
			}
		}
	}

	return result;
}

/** The film frame at which the caret last moved. */
function caretMoves(
	reel: Reel,
	starts: readonly number[],
	upTo: number,
): { x: Change[]; y: Change[]; h: Change[] } {
	const x: Change[] = [];
	const y: Change[] = [];
	const h: Change[] = [];

	for (let index = 0; index <= upTo; index += 1) {
		const caret = reel.frames[index]?.caret;
		const at = starts[index] ?? 0;

		if (!caret) {
			continue;
		}

		const [cx, cy, , ch] = caret;

		if (x[x.length - 1]?.value !== cx || y[y.length - 1]?.value !== cy) {
			x.push({ at, value: cx });
			y.push({ at, value: cy });
			h.push({ at, value: ch });
		}
	}

	return { x, y, h };
}

export interface PageProps {
	readonly reel: Reel;
	/** The film frame at which each captured frame appears. */
	readonly starts: readonly number[];
	/** The current film frame, relative to the same origin as `starts`. */
	readonly frame: number;
	/** Hide the local caret, as in the collaboration scene. */
	readonly caret?: boolean;
	/** A block to leave out, because the scene draws it elsewhere. */
	readonly lift?: string;
	/** Extra content drawn above the page, in page coordinates. */
	readonly children?: ReactNode;
	readonly style?: CSSProperties;
}

export function Page(
	{ reel, starts, frame, caret = true, lift, children, style }: PageProps,
) {
	const index = indexAt(starts, frame);
	const current: Frame | undefined = reel.frames[index];
	const history = useMemo(() => tracks(reel, starts, index), [
		reel,
		starts,
		index,
	]);
	const carets = useMemo(() => caretMoves(reel, starts, index), [
		reel,
		starts,
		index,
	]);

	if (!current) {
		return null;
	}

	const caretX = settle(carets.x, frame, GLIDE);
	const caretY = settle(carets.y, frame, GLIDE);
	const caretHeight = carets.h[carets.h.length - 1]?.value ?? 0;
	const lastMoved = carets.x[carets.x.length - 1]?.at ?? 0;

	return (
		<div style={{ position: "relative", width: reel.width, ...style }}>
			{current.blocks.map((layer) => {
				if (layer.id === lift) {
					return null;
				}

				const track = history.get(layer.id);
				return (
					<Block
						key={layer.id}
						svg={reel.svgs[layer.svg] ?? ""}
						box={layer.box}
						track={track}
						frame={frame}
					/>
				);
			})}
			{current.selection && (
				<div
					style={{
						position: "absolute",
						left: current.selection.x,
						top: current.selection.y,
					}}
					dangerouslySetInnerHTML={{
						__html: reel.svgs[current.selection.svg] ?? "",
					}}
				/>
			)}
			{caret && current.caret && caretOn(frame, lastMoved) && (
				<div
					style={{
						position: "absolute",
						left: caretX,
						top: caretY,
						width: Math.max(current.caret[2], 3),
						height: caretHeight,
						background: PAINT,
						borderRadius: 2,
					}}
				/>
			)}
			{children}
		</div>
	);
}

function Block(
	{ svg, box, track, frame }: {
		svg: string;
		box: Box;
		track: Track | undefined;
		frame: number;
	},
) {
	const [, , width, height] = box;
	const x = track ? settle(track.x, frame) : box[0];
	const y = track ? settle(track.y, frame) : box[1];
	const lastSnap = track?.snaps.filter((at) => at <= frame).pop();
	const snap = lastSnap === undefined ? 1 : spring({
		frame: frame - lastSnap,
		fps: FPS,
		config: WET,
		from: 0.94,
		to: 1,
	});

	return (
		<div
			style={{
				position: "absolute",
				left: x,
				top: y,
				width,
				height,
				transform: `scale(${snap})`,
				transformOrigin: "0% 50%",
			}}
			dangerouslySetInnerHTML={{ __html: svg }}
		/>
	);
}

/** The page's height at `frame`, springing as the document grows. */
export function usePageHeight(
	reel: Reel,
	starts: readonly number[],
	frame: number,
): number {
	const changes = useMemo(() => {
		const result: Change[] = [];

		reel.frames.forEach((item, index) => {
			if (result[result.length - 1]?.value !== item.height) {
				result.push({ at: starts[index] ?? 0, value: item.height });
			}
		});

		return result;
	}, [reel, starts]);

	return settle(changes, frame);
}

/** Where a block of the current frame is, springs included. */
export function useBlockBox(
	reel: Reel,
	starts: readonly number[],
	frame: number,
	id: string,
): Box | null {
	const index = indexAt(starts, frame);
	const history = useMemo(() => tracks(reel, starts, index), [
		reel,
		starts,
		index,
	]);
	const layer = reel.frames[index]?.blocks.find((block) => block.id === id);
	const track = history.get(id);

	if (!layer || !track) {
		return null;
	}

	return [
		settle(track.x, frame),
		settle(track.y, frame),
		layer.box[2],
		layer.box[3],
	];
}
