/**
 * A document typed one key at a time, with what each key cost: the time
 * from the key event to a finished display list, measured while recording.
 */

import { useMemo } from "react";
import { useCurrentFrame } from "remotion";

import { LINE, MONO, PAINT_ON_DARK, PAPER, PAPER_MUTED } from "../brand";
import type { Frame } from "../data";
import { Page, usePageHeight } from "../motion/Page";
import { indexAt, schedule } from "../motion/time";
import { Caption, Scene, type SceneProps, Sheet } from "./common";

/** One 60 Hz frame, in microseconds. */
const FRAME_MICROS = 1_000_000 / 60;
const LEAD_IN = 34;
const HOLD = 64;

/** How long each captured key shows, in relative beats. */
export function typingBeats(frame: Frame, index: number): number {
	if (index === 0) {
		return 6;
	}

	if (frame.extra.kind === "InputRule") {
		return 7;
	}

	if (frame.note === "Enter") {
		return 6;
	}

	return frame.note === " " ? 2.2 : 1.5;
}

export function Typing({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const reel = footage.typing;
	const starts = useMemo(
		() =>
			schedule(reel.frames, typingBeats, duration - LEAD_IN - HOLD).map((at) =>
				at + LEAD_IN
			),
		[reel, duration],
	);
	const pageHeight = usePageHeight(reel, starts, frame);
	const index = indexAt(starts, frame);
	const timed = reel.frames.slice(1, index + 1).flatMap((
		item,
	) => (item.micros === null ? [] : [item.micros]));
	const latest = frame < LEAD_IN ? null : (timed[timed.length - 1] ?? null);
	const sorted = [...timed].sort((a, b) => a - b);
	const median = sorted[Math.floor(sorted.length / 2)] ?? null;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="01 · type"
				headline="Every key, painted in microseconds."
			>
				{stage.caption.height > 300 && (
					<Meter
						latest={latest}
						median={median}
						history={timed}
						target={footage.manifest.target}
						wide={stage.width > stage.height}
					/>
				)}
			</Caption>
			{stage.caption.height <= 300 && (
				<div
					style={{
						position: "absolute",
						left: stage.caption.x,
						bottom: 70,
						width: stage.caption.width,
					}}
				>
					<Meter
						latest={latest}
						median={median}
						history={timed}
						target={footage.manifest.target}
						wide={false}
					/>
				</div>
			)}
			<Sheet stage={stage} page={pageHeight}>
				<Page reel={reel} starts={starts} frame={frame} />
			</Sheet>
		</Scene>
	);
}

function Meter({
	latest,
	median,
	history,
	target,
	wide,
}: {
	latest: number | null;
	median: number | null;
	history: readonly number[];
	target: string;
	wide: boolean;
}) {
	const bars = history.slice(-48);
	const ceiling = 60;

	return (
		<div style={{ marginTop: wide ? 64 : 28, fontFamily: MONO, color: PAPER }}>
			<div style={{ display: "flex", alignItems: "baseline", gap: 14 }}>
				<span
					style={{
						fontSize: wide ? 104 : 72,
						fontWeight: 500,
						letterSpacing: "-0.04em",
						fontVariantNumeric: "tabular-nums",
					}}
				>
					{latest === null ? "—" : latest.toFixed(1)}
				</span>
				<span style={{ fontSize: 34, color: PAINT_ON_DARK }}>µs</span>
			</div>
			<div
				style={{
					fontSize: 20,
					color: PAPER_MUTED,
					marginTop: 6,
					letterSpacing: "0.02em",
				}}
			>
				key event → relayout → display list
			</div>
			<div
				style={{
					display: "flex",
					alignItems: "flex-end",
					gap: 3,
					height: 70,
					marginTop: 26,
					borderBottom: `2px solid ${LINE}`,
				}}
			>
				{bars.map((micros, index) => (
					<div
						key={index}
						style={{
							width: 6,
							height: Math.max(2, (Math.min(micros, ceiling) / ceiling) * 70),
							background: index === bars.length - 1 ? PAINT_ON_DARK : LINE,
						}}
					/>
				))}
			</div>
			<div
				style={{
					fontSize: 20,
					color: PAPER_MUTED,
					marginTop: 18,
					lineHeight: 1.5,
				}}
			>
				{median === null
					? " "
					: `median ${median.toFixed(1)} µs · ${
						((median / FRAME_MICROS) * 100).toFixed(2)
					}% of a 60 Hz frame`}
				<br />
				measured while recording
				<br />
				{target}
			</div>
		</div>
	);
}
