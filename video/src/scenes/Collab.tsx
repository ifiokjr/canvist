/**
 * Three peers writing over Yjs, seen from Ada's page. Every caret is a Yjs
 * relative position resolved by `Presence` in her document; they write
 * apart, then meet on the last line.
 */

import { useMemo } from "react";
import { useCurrentFrame } from "remotion";

import { DISPLAY, MONO, PAPER, PAPER_MUTED } from "../brand";
import type { Frame, Reel } from "../data";
import { TypedText } from "../motion/Caret";
import { Page, usePageHeight } from "../motion/Page";
import { type Change, GLIDE, indexAt, schedule, settle } from "../motion/time";
import { Caption, ERASE_BEFORE, Scene, type SceneProps, Sheet } from "./common";

const LEAD_IN = 30;
/** How long a collaborator's name shows after their caret moves. */
const FLAG_FRAMES = 24;
const HOLD = 54;

function beats(frame: Frame, index: number): number {
	if (index === 0) {
		return 8;
	}

	return frame.extra.phase === "converge" ? 16 : 1.6;
}

/** Each peer's caret positions over time, by name. */
function peerTracks(reel: Reel, starts: readonly number[]) {
	const tracks = new Map<
		string,
		{ color: string; x: Change[]; y: Change[]; height: number }
	>();

	reel.frames.forEach((item, index) => {
		const at = starts[index] ?? 0;

		for (const peer of item.peers) {
			const [x, y, , height] = peer.caret;
			const track = tracks.get(peer.name) ??
				{ color: peer.color, x: [], y: [], height };

			if (
				track.x[track.x.length - 1]?.value !== x ||
				track.y[track.y.length - 1]?.value !== y
			) {
				track.x.push({ at, value: x });
				track.y.push({ at, value: y });
			}

			track.height = height;
			tracks.set(peer.name, track);
		}
	});

	return tracks;
}

function metaNumber(reel: Reel, key: string): number {
	const value = reel.meta[key];

	if (typeof value !== "number") {
		throw new Error(
			`footage: collab.meta.${key} is missing; rerun npm run prepare:footage`,
		);
	}

	return value;
}

export function Collab({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const reel = footage.collab;
	const starts = useMemo(
		() =>
			schedule(reel.frames, beats, duration - LEAD_IN - HOLD).map((at) =>
				at + LEAD_IN
			),
		[reel, duration],
	);
	const pageHeight = usePageHeight(reel, starts, frame);
	const tracks = useMemo(() => peerTracks(reel, starts), [reel, starts]);
	const index = indexAt(starts, frame);
	const converged = reel.meta.converged === true;
	const done = index === reel.frames.length - 1;
	const erase = duration - ERASE_BEFORE;
	const facts = [
		`${metaNumber(reel, "updates")} Yjs updates, ${
			metaNumber(reel, "bytesPerUpdate")
		} B each`,
		`${
			metaNumber(reel, "remoteMedianMicros").toFixed(1)
		} µs to apply one and relay out`,
		converged ? "3 replicas, identical" : "replicas differ",
	];
	const doneAt = starts[starts.length - 1] ?? 0;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="04 · together"
				headline="Three carets. One document."
			>
				{stage.caption.height > 300 && (
					<div
						style={{
							marginTop: 48,
							display: "flex",
							flexDirection: "column",
							gap: 12,
						}}
					>
						{facts.map((fact, position) => (
							<TypedText
								key={fact}
								text={fact}
								start={position === 2 ? doneAt + 4 : 40 + position * 14}
								rate={50}
								size={24}
								weight={500}
								family={MONO}
								color={position === 2 && done ? PAPER : PAPER_MUTED}
								caretColor="transparent"
								erase={erase}
							/>
						))}
					</div>
				)}
			</Caption>
			<Sheet stage={stage} page={pageHeight}>
				<Page reel={reel} starts={starts} frame={frame} caret={false}>
					{[...tracks.entries()].map(([name, track]) => {
						const moved = track.x.filter((change) =>
							change.at <= frame
						).pop()?.at ?? 0;

						return (
							<PeerCaret
								key={name}
								name={name}
								color={track.color}
								x={settle(track.x, frame, GLIDE)}
								y={settle(track.y, frame, GLIDE)}
								height={track.height}
								flag={frame < LEAD_IN + 30 || frame - moved < FLAG_FRAMES}
							/>
						);
					})}
				</Page>
			</Sheet>
		</Scene>
	);
}

function PeerCaret(
	{ name, color, x, y, height, flag }: {
		name: string;
		color: string;
		x: number;
		y: number;
		height: number;
		flag: boolean;
	},
) {
	return (
		<div
			style={{
				position: "absolute",
				left: x - 1.5,
				top: y,
				width: 4,
				height,
				background: color,
				borderRadius: 2,
			}}
		>
			{!flag && (
				<div
					style={{
						position: "absolute",
						left: -3,
						top: -4,
						width: 10,
						height: 10,
						borderRadius: 5,
						background: color,
					}}
				/>
			)}
			{flag && (
				<div
					style={{
						position: "absolute",
						left: 0,
						bottom: height + 2,
						padding: "3px 10px 4px",
						background: color,
						color: PAPER,
						fontFamily: DISPLAY,
						fontWeight: 700,
						fontSize: 18,
						borderRadius: "8px 8px 8px 2px",
						whiteSpace: "nowrap",
					}}
				>
					{name}
				</div>
			)}
		</div>
	);
}
