/**
 * The end card: free forever, and the hosted services that pay for it, from
 * `docs/src/v2/business.md`.
 */

import { spring, useCurrentFrame } from "remotion";

import {
	DISPLAY,
	LINE,
	MONO,
	PAINT_ON_DARK,
	PAPER,
	PAPER_MUTED,
} from "../brand";
import { TypedText } from "../motion/Caret";
import { caretOn, FPS, WET } from "../motion/time";
import { Scene, type SceneProps } from "./common";

/** When the lockup lands and the headline's caret moves into it. */
const LOCKUP = 96;

const SERVICES = [
	"canvist Sync",
	"canvist Convert",
	"canvist History",
] as const;

export function End({ footage, stage }: SceneProps) {
	const frame = useCurrentFrame();
	const wide = stage.width > stage.height;
	const { viewBox, ink, paint } = footage.brand.lockup;
	const [, , boxWidth = 240, boxHeight = 64] = viewBox.split(" ").map(Number);
	const lockupWidth = wide ? 420 : 460;
	const left = wide ? 140 : 80;
	const headline = wide ? 150 : 128;

	return (
		<Scene>
			<div
				style={{
					position: "absolute",
					left,
					top: wide ? 150 : stage.height > stage.width ? 420 : 110,
					width: stage.width - left * 2,
				}}
			>
				<TypedText
					text="Free forever."
					start={6}
					rate={22}
					size={headline}
					weight={800}
					keepCaret={frame < LOCKUP}
				/>
				<TypedText
					text="Unlicense. The editor, the server, every binding."
					start={34}
					rate={50}
					size={wide ? 40 : 36}
					weight={500}
					color={PAPER_MUTED}
					caretColor="transparent"
					style={{ marginTop: 26 }}
				/>
				<div
					style={{
						marginTop: wide ? 76 : 64,
						fontFamily: MONO,
						fontSize: 22,
						color: PAINT_ON_DARK,
						letterSpacing: "0.08em",
						textTransform: "uppercase",
					}}
				>
					{frame >= 64 ? "We charge only for running it" : " "}
				</div>
				<div
					style={{ display: "flex", flexWrap: "wrap", gap: 18, marginTop: 20 }}
				>
					{SERVICES.map((service, index) => {
						const appear = 70 + index * 6;

						if (frame < appear) {
							return null;
						}

						const settled = spring({
							frame: frame - appear,
							fps: FPS,
							config: WET,
						});
						return (
							<div
								key={service}
								style={{
									fontFamily: DISPLAY,
									fontWeight: 700,
									fontSize: wide ? 34 : 30,
									color: PAPER,
									padding: "14px 26px",
									border: `2px solid ${LINE}`,
									borderRadius: 18,
									transform: `translateY(${(1 - settled) * 24}px)`,
								}}
							>
								{service}
							</div>
						);
					})}
				</div>
			</div>
			<div
				style={{
					position: "absolute",
					left,
					bottom: wide ? 110 : 120,
					display: "flex",
					alignItems: "flex-end",
					justifyContent: "space-between",
					width: stage.width - left * 2,
					flexWrap: "wrap",
					gap: 24,
				}}
			>
				{frame >= LOCKUP && (
					<svg
						width={lockupWidth}
						height={(lockupWidth * boxHeight) / boxWidth}
						viewBox={viewBox}
					>
						<path d={ink} fill={PAPER} />
						{caretOn(frame, LOCKUP) && <path d={paint} fill={PAINT_ON_DARK} />}
					</svg>
				)}
				<TypedText
					text="github.com/ifiokjr/canvist"
					start={104}
					rate={50}
					size={wide ? 30 : 28}
					weight={500}
					family={MONO}
					color={PAPER_MUTED}
					caretColor="transparent"
				/>
			</div>
		</Scene>
	);
}
