/**
 * Headless and typed: a snippet calls a command from an extension the
 * editor never registered, and the TypeScript compiler refuses it. The
 * declarations come from the StarterKit's `metadata_json()`; the error is
 * what `tsc` printed when `prepare` checked the snippet.
 */

import { useCurrentFrame } from "remotion";

import {
	INK_RAISED,
	LINE,
	MONO,
	PAINT_ON_DARK,
	PAPER,
	PAPER_MUTED,
} from "../brand";
import { typedLength, TypedText } from "../motion/Caret";
import { caretOn } from "../motion/time";
import { Caption, ERASE_BEFORE, Scene, type SceneProps } from "./common";

const ERROR = "#FF6B6B";
const KEYWORD = PAINT_ON_DARK;
const START = 24;
const RATE = 90;

function tint(line: string): { text: string; color: string }[] {
	return line.split(/(\b(?:import|from|const)\b|"[^"]*"|\b\d+\b)/).filter(
		Boolean,
	).map((text) => ({
		text,
		color: /^(import|from|const)$/.test(text)
			? KEYWORD
			: /^"/.test(text)
			? "#C8C2FF"
			: /^\d+$/.test(text)
			? "#FFC56B"
			: PAPER,
	}));
}

export function Headless({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const { snippet, diagnostics, commands, extensions } = footage.typed;
	const diagnostic = diagnostics[0];
	const shown = typedLength(snippet, frame, START, RATE);
	const typedAt = START + Math.ceil((snippet.length * 30) / RATE);
	const lines = snippet.slice(0, shown).trimEnd().split("\n");
	const wide = stage.width > stage.height;
	const size = wide ? 30 : 26;
	const region = stage.sheet;
	const erase = duration - ERASE_BEFORE;
	const flagged = diagnostic && frame >= typedAt + 8;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="07 · headless"
				headline="Typed by the extensions you register."
			>
				{stage.caption.height > 300 && (
					<TypedText
						text={`${commands} commands from ${extensions} extensions, generated from editor.metadata_json()`}
						start={50}
						rate={60}
						size={22}
						weight={500}
						family={MONO}
						color={PAPER_MUTED}
						caretColor="transparent"
						erase={erase}
						style={{ marginTop: 40, lineHeight: 1.5 }}
					/>
				)}
			</Caption>
			<div
				style={{
					position: "absolute",
					left: region.x,
					top: region.y + (wide ? 90 : 0),
					width: region.width,
					background: INK_RAISED,
					borderRadius: 28,
					padding: wide ? "40px 48px" : "32px 28px",
					boxSizing: "border-box",
					fontFamily: MONO,
					fontSize: size,
					lineHeight: 1.7,
					border: `2px solid ${LINE}`,
				}}
			>
				<div style={{ color: PAPER_MUTED, fontSize: 20, marginBottom: 20 }}>
					demo.ts
				</div>
				{lines.map((line, index) => {
					const number = index + 1;
					const isError = flagged && diagnostic.line === number;
					return (
						<div
							key={index}
							style={{
								display: "flex",
								whiteSpace: "pre",
								minHeight: size * 1.7,
							}}
						>
							<span style={{ width: size * 1.6, color: LINE, flexShrink: 0 }}>
								{number}
							</span>
							<span style={{ position: "relative" }}>
								{tint(line).map((part, position) => (
									<span key={position} style={{ color: part.color }}>
										{part.text}
									</span>
								))}
								{index === lines.length - 1 && shown < snippet.length &&
									caretOn(frame, frame) && (
									<span
										style={{
											display: "inline-block",
											width: 3,
											height: size,
											background: PAINT_ON_DARK,
											verticalAlign: "middle",
										}}
									/>
								)}
								{isError && (
									<Squiggle
										column={diagnostic.column}
										line={line}
										size={size}
										frame={frame - typedAt - 8}
									/>
								)}
							</span>
						</div>
					);
				})}
				{flagged && (
					<div
						style={{
							marginTop: 28,
							padding: "18px 22px",
							borderLeft: `5px solid ${ERROR}`,
							background: "#2B1F2A",
							borderRadius: 8,
							fontSize: wide ? 24 : 21,
							lineHeight: 1.45,
						}}
					>
						<TypedText
							text={`error TS${diagnostic.code}: ${diagnostic.message}`}
							start={typedAt + 14}
							rate={70}
							size={wide ? 24 : 21}
							weight={500}
							family={MONO}
							color={PAPER}
							caretColor="transparent"
							erase={erase}
							style={{ lineHeight: 1.45 }}
						/>
					</div>
				)}
			</div>
		</Scene>
	);
}

/** A wavy underline under the identifier starting at `column`. */
function Squiggle(
	{ column, line, size, frame }: {
		column: number;
		line: string;
		size: number;
		frame: number;
	},
) {
	const rest = line.slice(column - 1);
	const length = rest.match(/^[A-Za-z_$][\w$]*/)?.[0].length ?? rest.length;
	const advance = size * 0.6;
	const width = length * advance;
	const drawn = Math.min(1, frame / 8);
	const wave = 8;
	let d = `M0 3`;

	for (let x = 0; x < width; x += wave) {
		d += ` q${wave / 4} -3 ${wave / 2} 0 t${wave / 2} 0`;
	}

	return (
		<svg
			width={width * drawn}
			height={8}
			viewBox={`0 0 ${width * drawn} 8`}
			style={{
				position: "absolute",
				left: (column - 1) * advance,
				top: size * 1.32,
				overflow: "hidden",
			}}
		>
			<path d={d} fill="none" stroke={ERROR} strokeWidth={2.5} />
		</svg>
	);
}
