/**
 * Numbers from the performance budget table in
 * `docs/src/v2/architecture.md`, where each budget is a release-mode test.
 * Each bar is the whole budget; the paint inside it is today's number.
 */

import { useMemo } from "react";
import { spring, useCurrentFrame } from "remotion";

import { LINE, MONO, PAINT_ON_DARK, PAPER, PAPER_MUTED } from "../brand";
import type { Budget } from "../data";
import { TypedText } from "../motion/Caret";
import { FPS, WET } from "../motion/time";
import { Caption, ERASE_BEFORE, Scene, type SceneProps } from "./common";

/** The rows shown, by the start of their metric in the table, and how to label them. */
const ROWS: readonly (readonly [string, string])[] = [
	["Apply one keystroke (model), 100k blocks", "Keystroke, 100k blocks"],
	["Relayout one edited paragraph", "Relayout a paragraph"],
	["Display list for one screen", "Display list, one screen"],
	["Scroll one screen into unmeasured blocks", "Scroll into new text"],
	[
		"Apply a collaborator's keystroke, 100k blocks",
		"Remote keystroke, 100k blocks",
	],
	["Join a 10k-paragraph document from Yjs state", "Join 10k paragraphs"],
];

const UNITS: Record<string, number> = { "µs": 1, ms: 1000 };

/** A duration such as `< 2 µs` or `0.29 µs`, in microseconds. */
function micros(text: string): number {
	const match = text.match(/([\d.]+)\s*(µs|ms)/);
	const unit = match?.[2];

	if (!match || !unit) {
		throw new Error(`footage: cannot read a duration from "${text}"`);
	}

	return Number(match[1]) * (UNITS[unit] ?? 1);
}

interface Row {
	readonly label: string;
	readonly today: string;
	readonly budget: string;
	readonly ratio: number;
}

function pick(budgets: readonly Budget[]): Row[] {
	return ROWS.map(([metric, label]) => {
		const row = budgets.find((budget) => budget.metric.startsWith(metric));

		if (!row) {
			throw new Error(`footage: the budget table has no row for "${metric}"`);
		}

		return {
			label,
			today: row.today,
			budget: row.budget.replace(/^<\s*/, ""),
			ratio: micros(row.today) / micros(row.budget),
		};
	});
}

export function Bench({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const rows = useMemo(() => pick(footage.budgets), [footage]);
	const wide = stage.width > stage.height;
	const region = stage.sheet;
	const erase = duration - ERASE_BEFORE;
	const rowHeight = wide ? 118 : stage.height > stage.width ? 150 : 112;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="08 · fast"
				headline="Every budget is a test."
			/>
			<div
				style={{
					position: "absolute",
					left: region.x,
					top: region.y + (wide ? 60 : 0),
					width: region.width,
				}}
			>
				{rows.map((row, index) => {
					const start = 16 + index * 8;
					const fill = frame < start + 6
						? 0
						: spring({ frame: frame - start - 6, fps: FPS, config: WET });

					return (
						<div
							key={row.label}
							style={{ height: rowHeight, fontFamily: MONO }}
						>
							<div
								style={{
									display: "flex",
									justifyContent: "space-between",
									alignItems: "baseline",
									marginBottom: 14,
								}}
							>
								<TypedText
									text={row.label}
									start={start}
									rate={70}
									size={wide ? 28 : 26}
									weight={500}
									family={MONO}
									color={PAPER}
									caretColor="transparent"
									erase={erase}
								/>
								<TypedText
									text={`${row.today} of ${row.budget}`}
									start={start + 4}
									rate={70}
									size={wide ? 26 : 22}
									weight={500}
									family={MONO}
									color={PAINT_ON_DARK}
									caretColor="transparent"
									erase={erase}
								/>
							</div>
							{frame >= start && frame < duration - 6 && (
								<div
									style={{
										height: 18,
										borderRadius: 9,
										border: `2px solid ${LINE}`,
										position: "relative",
										overflow: "hidden",
									}}
								>
									<div
										style={{
											position: "absolute",
											left: 0,
											top: 0,
											bottom: 0,
											width: `${Math.max(row.ratio * 100, 0.8) * fill}%`,
											background: PAINT_ON_DARK,
											borderRadius: 9,
										}}
									/>
								</div>
							)}
						</div>
					);
				})}
				<TypedText
					text="docs/src/v2/architecture.md · tests/perf_budgets.rs in every crate"
					start={70}
					rate={70}
					size={20}
					weight={400}
					family={MONO}
					color={PAPER_MUTED}
					caretColor="transparent"
					erase={erase}
					style={{ marginTop: 8 }}
				/>
			</div>
		</Scene>
	);
}
