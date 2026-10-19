/**
 * The same document at every width. The sweep relays it out at 49 widths
 * between 1040 and 368 pixels; the lineup shows it at each platform's own
 * viewport and text size. Every screen is a real layout of that width.
 */

import { useMemo } from "react";
import { Easing, interpolate, spring, useCurrentFrame } from "remotion";

import { INK_RAISED, LINE, MONO, PAINT_ON_DARK, PAPER } from "../brand";
import type { Reel } from "../data";
import type { Format, Region } from "../layout";
import { TypedText } from "../motion/Caret";
import { Page } from "../motion/Page";
import { FPS, WET } from "../motion/time";
import {
	at,
	Caption,
	ERASE_BEFORE,
	Kicker,
	Scene,
	type SceneProps,
} from "./common";

/** Padding between a screen's edge and the page, in that screen's pixels. */
const SCREEN_PAD = 18;

function widthOf(reel: Reel, index: number): number {
	const value = reel.frames[index]?.extra.width;
	return typeof value === "number" ? value : reel.width;
}

/** One layout at one width, scaled to `scale`, clipped to `height`. */
function Screen(
	{ reel, index, scale, height }: {
		reel: Reel;
		index: number;
		scale: number;
		height: number;
	},
) {
	const width = widthOf(reel, index);
	const starts = useMemo(() => reel.frames.map((_, position) => position), [
		reel,
	]);

	return (
		<div
			style={{
				width: (width + SCREEN_PAD * 2) * scale,
				height,
				overflow: "hidden",
				background: PAPER,
			}}
		>
			<div
				style={{
					transform: `scale(${scale})`,
					transformOrigin: "0 0",
					padding: SCREEN_PAD,
					width,
				}}
			>
				<Page
					reel={{ ...reel, width }}
					starts={starts}
					frame={index}
					caret={false}
				/>
			</div>
		</div>
	);
}

export function Reflow({ footage, stage, duration }: SceneProps) {
	const frame = useCurrentFrame();
	const sweep = footage.sweep;
	const last = sweep.frames.length - 1;
	const progress = interpolate(frame, [30, duration - 50], [0, 1], {
		extrapolateLeft: "clamp",
		extrapolateRight: "clamp",
		easing: Easing.inOut(Easing.sin),
	});
	const index = Math.round(progress * last);
	const width = widthOf(sweep, index);
	const region = stage.sheet;
	const widest = widthOf(sweep, 0) + SCREEN_PAD * 2;
	const scale = Math.min(region.width / widest, 1.25);
	const micros = sweep.frames[index]?.micros ?? 0;

	return (
		<Scene>
			<Caption
				stage={stage}
				duration={duration}
				kicker="05 · reflow"
				headline="One layout engine. Any width."
			/>
			<div style={{ ...at(region), height: region.height }}>
				<div
					style={{
						fontFamily: MONO,
						fontSize: 22,
						color: PAINT_ON_DARK,
						marginBottom: 14,
						fontVariantNumeric: "tabular-nums",
					}}
				>
					{width.toFixed(0)} px · laid out in {(micros / 1000).toFixed(2)} ms
				</div>
				<div
					style={{
						borderRadius: 22,
						overflow: "hidden",
						display: "inline-block",
						border: `10px solid ${INK_RAISED}`,
					}}
				>
					<div
						style={{
							height: 34,
							background: INK_RAISED,
							display: "flex",
							gap: 9,
							alignItems: "center",
							paddingLeft: 6,
						}}
					>
						{[0, 1, 2].map((dot) => (
							<div
								key={dot}
								style={{
									width: 12,
									height: 12,
									borderRadius: 6,
									background: LINE,
								}}
							/>
						))}
					</div>
					<Screen
						reel={sweep}
						index={index}
						scale={scale}
						height={region.height - 110}
					/>
				</div>
			</div>
		</Scene>
	);
}

type Kind = "window" | "browser" | "tablet" | "phone";

interface Device {
	readonly platform: string;
	readonly label: string;
	readonly kind: Kind;
	/** The screen height in the platform's own pixels. */
	readonly screen: number;
}

const DEVICES: Record<string, Device> = {
	desktop: {
		platform: "desktop",
		label: "Desktop",
		kind: "window",
		screen: 600,
	},
	web: { platform: "web", label: "Web", kind: "browser", screen: 540 },
	flutter: {
		platform: "flutter",
		label: "Flutter",
		kind: "tablet",
		screen: 800,
	},
	ios: { platform: "ios", label: "iOS", kind: "phone", screen: 844 },
	android: {
		platform: "android",
		label: "Android",
		kind: "phone",
		screen: 892,
	},
	"react-native": {
		platform: "react-native",
		label: "React Native",
		kind: "phone",
		screen: 844,
	},
	lynx: { platform: "lynx", label: "Lynx", kind: "phone", screen: 812 },
};

const ROWS: Record<Format, readonly (readonly string[])[]> = {
	landscape: [["desktop", "web", "flutter"], [
		"ios",
		"android",
		"react-native",
		"lynx",
	]],
	portrait: [["desktop"], ["web", "flutter"], ["ios", "android"], [
		"react-native",
		"lynx",
	]],
	square: [["desktop", "web", "flutter"], [
		"ios",
		"android",
		"react-native",
		"lynx",
	]],
};

const BEZEL: Record<Kind, number> = {
	window: 40,
	browser: 52,
	tablet: 36,
	phone: 56,
};
const LABEL = 44;
const GAP = 36;

function platformOf(reel: Reel): string {
	const value = reel.meta.platform;

	if (typeof value !== "string") {
		throw new Error(
			"footage: a device reel has no platform; rerun npm run prepare:footage",
		);
	}

	return value;
}

export function Everywhere(
	{ footage, stage, duration, format }: SceneProps & { format: Format },
) {
	const frame = useCurrentFrame();
	const byPlatform = useMemo(
		() => new Map(footage.devices.map((reel) => [platformOf(reel), reel])),
		[footage],
	);
	const top = stage.width > stage.height
		? 200
		: stage.height > stage.width
		? 560
		: 170;
	const area: Region = {
		x: 60,
		y: top,
		width: stage.width - 120,
		height: stage.height - top - 50,
	};
	const rows = ROWS[format];
	const sized = rows.map((row) =>
		row.map((name) => {
			const reel = byPlatform.get(name);
			const device = DEVICES[name];

			if (!reel || !device) {
				throw new Error(`footage: no device reel for ${name}`);
			}

			const width = reel.width + SCREEN_PAD * 2;
			return {
				reel,
				device,
				aspect: width / (device.screen + BEZEL[device.kind]),
			};
		})
	);
	const heights = sized.map((row) =>
		(area.width - GAP * (row.length - 1)) /
		row.reduce((sum, item) => sum + item.aspect, 0)
	);
	const fit = Math.min(
		1,
		(area.height - LABEL * rows.length - GAP * (rows.length - 1)) /
			heights.reduce((a, b) => a + b, 0),
	);
	let order = 0;
	const erase = duration - ERASE_BEFORE;

	return (
		<Scene>
			<div
				style={{
					position: "absolute",
					left: 80,
					top: stage.width > stage.height
						? 70
						: stage.height > stage.width
						? 300
						: 52,
					width: stage.width - 160,
				}}
			>
				<Kicker text="06 · everywhere" start={4} erase={erase} />
				<TypedText
					text="One Rust core. Every screen."
					start={10}
					size={stage.headline * 0.86}
					erase={erase}
				/>
			</div>
			<div
				style={{
					position: "absolute",
					left: area.x,
					top: area.y,
					width: area.width,
					display: "flex",
					flexDirection: "column",
					gap: GAP,
					alignItems: "center",
				}}
			>
				{sized.map((row, rowIndex) => {
					const height = (heights[rowIndex] ?? 0) * fit;

					return (
						<div
							key={rowIndex}
							style={{ display: "flex", gap: GAP, alignItems: "flex-end" }}
						>
							{row.map(({ reel, device, aspect }) => {
								const appear = 20 + order * 7;
								order += 1;
								const settled = frame < appear
									? 0
									: spring({ frame: frame - appear, fps: FPS, config: WET });

								if (frame < appear) {
									return (
										<div
											key={device.platform}
											style={{ width: height * aspect, height: height + LABEL }}
										/>
									);
								}

								return (
									<div
										key={device.platform}
										style={{
											transform: `translateY(${(1 - settled) * 40}px) scale(${
												0.94 + settled * 0.06
											})`,
											transformOrigin: "50% 100%",
										}}
									>
										<DeviceFrame
											reel={reel}
											device={device}
											height={height}
											aspect={aspect}
										/>
										<div
											style={{
												height: LABEL,
												display: "flex",
												alignItems: "flex-end",
												justifyContent: "center",
											}}
										>
											<TypedText
												text={device.label}
												start={appear + 6}
												rate={36}
												size={24}
												weight={700}
												color={PAPER}
												caretColor="transparent"
												erase={erase}
											/>
										</div>
									</div>
								);
							})}
						</div>
					);
				})}
			</div>
		</Scene>
	);
}

function DeviceFrame(
	{ reel, device, height, aspect }: {
		reel: Reel;
		device: Device;
		height: number;
		aspect: number;
	},
) {
	const width = height * aspect;
	const scale = width / (reel.width + SCREEN_PAD * 2);
	const bezel = BEZEL[device.kind] * scale;
	const radius = device.kind === "phone"
		? 64 * scale
		: device.kind === "tablet"
		? 40 * scale
		: 18 * scale;

	return (
		<div
			style={{
				width,
				height,
				borderRadius: radius,
				background: INK_RAISED,
				overflow: "hidden",
				position: "relative",
			}}
		>
			{device.kind === "window" || device.kind === "browser"
				? (
					<div
						style={{
							height: bezel,
							display: "flex",
							alignItems: "center",
							gap: 8 * scale,
							paddingLeft: 18 * scale,
						}}
					>
						{[0, 1, 2].map((dot) => (
							<div
								key={dot}
								style={{
									width: 13 * scale,
									height: 13 * scale,
									borderRadius: 7 * scale,
									background: LINE,
								}}
							/>
						))}
						{device.kind === "browser" && (
							<div
								style={{
									marginLeft: 18 * scale,
									height: 26 * scale,
									width: width * 0.5,
									borderRadius: 13 * scale,
									background: LINE,
								}}
							/>
						)}
					</div>
				)
				: (
					<div
						style={{
							height: bezel,
							display: "flex",
							justifyContent: "center",
							alignItems: "center",
						}}
					>
						<div
							style={{
								width: device.kind === "phone" ? 110 * scale : 10 * scale,
								height: device.kind === "phone" ? 30 * scale : 10 * scale,
								borderRadius: 16 * scale,
								background: "#000",
							}}
						/>
					</div>
				)}
			<div
				style={{
					borderRadius: device.kind === "phone"
						? `0 0 ${radius}px ${radius}px`
						: 0,
					overflow: "hidden",
				}}
			>
				<Screen reel={reel} index={0} scale={scale} height={height - bezel} />
			</div>
			<div
				style={{
					position: "absolute",
					inset: 0,
					borderRadius: radius,
					boxShadow: `inset 0 0 0 ${Math.max(2, 6 * scale)}px ${INK_RAISED}`,
					pointerEvents: "none",
				}}
			/>
		</div>
	);
}
