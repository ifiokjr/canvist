/**
 * Markdown shortcuts becoming marks and blocks as they are typed. Each
 * input rule that fires is logged beside the page.
 */

import { useMemo } from "react";
import { useCurrentFrame } from "remotion";

import { MONO, PAINT_ON_DARK, PAPER, PAPER_MUTED } from "../brand";
import { TypedText } from "../motion/Caret";
import { Page, usePageHeight } from "../motion/Page";
import { schedule } from "../motion/time";
import { Caption, ERASE_BEFORE, Scene, type SceneProps, Sheet } from "./common";
import { typingBeats } from "./Typing";

const LEAD_IN = 26;
const HOLD = 46;

interface Rule {
	readonly typed: string;
	readonly became: string;
}

function rules(meta: Record<string, unknown>): Rule[] {
	const list = meta.rules;

	if (!Array.isArray(list)) {
		throw new Error(
			"footage: format.meta.rules is missing; rerun npm run prepare:footage",
		);
	}

	return list.map((rule: unknown) => {
		if (
			typeof rule !== "object" || rule === null || !("typed" in rule) ||
			!("became" in rule)
		) {
			throw new Error("footage: a formatting rule is malformed");
		}

		const { typed, became } = rule;

		if (typeof typed !== "string" || typeof became !== "string") {
			throw new Error("footage: a formatting rule is malformed");
		}

		return { typed, became };
	});
}

export function Format({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const reel = footage.format;
	const starts = useMemo(
		() =>
			schedule(reel.frames, typingBeats, duration - LEAD_IN - HOLD).map((at) =>
				at + LEAD_IN
			),
		[reel, duration],
	);
	const pageHeight = usePageHeight(reel, starts, frame);
	const all = useMemo(() => rules(reel.meta), [reel]);
	const fired = useMemo(
		() =>
			reel.frames.flatMap((
				item,
				index,
			) => (item.extra.kind === "InputRule" ? [starts[index] ?? 0] : [])),
		[reel, starts],
	);
	const wide = stage.width > stage.height;
	const erase = duration - ERASE_BEFORE;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="02 · structure"
				headline="Markdown in. Blocks out."
			>
				{stage.caption.height > 300 && (
					<div
						style={{
							marginTop: wide ? 56 : 28,
							display: "grid",
							gridTemplateColumns: wide ? "1fr" : "1fr 1fr",
							gap: wide ? 10 : "8px 32px",
						}}
					>
						{all.map((rule, position) => {
							const at = fired[position];

							if (at === undefined || frame < at) {
								return null;
							}

							const newest = fired.filter((value) =>
										value <= frame
									).length - 1 === position;
							return (
								<div
									key={rule.became}
									style={{
										display: "flex",
										gap: 16,
										alignItems: "baseline",
										fontFamily: MONO,
										fontSize: 24,
									}}
								>
									<TypedText
										text={rule.typed.trimEnd()}
										start={at}
										rate={60}
										size={24}
										weight={500}
										family={MONO}
										color={newest ? PAINT_ON_DARK : PAPER_MUTED}
										caretColor="transparent"
										erase={erase}
										style={{ minWidth: 140 }}
									/>
									<TypedText
										text={`→ ${rule.became}`}
										start={at + 3}
										rate={60}
										size={24}
										weight={500}
										family={MONO}
										color={newest ? PAPER : PAPER_MUTED}
										caretColor="transparent"
										erase={erase}
									/>
								</div>
							);
						})}
					</div>
				)}
			</Caption>
			<Sheet stage={stage} page={pageHeight}>
				<Page reel={reel} starts={starts} frame={frame} />
			</Sheet>
		</Scene>
	);
}
