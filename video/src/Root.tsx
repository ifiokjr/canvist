/**
 * The film's compositions: the 16:9 launch cut (rendered at 1080p and, with
 * `--scale 2`, 4K) and the 9:16 and 1:1 social cuts.
 */

import { Composition } from "remotion";

import { Film, FILM_FRAMES } from "./Film";
import { STAGES } from "./layout";
import { FPS } from "./motion/time";

export function Root() {
	return (
		<>
			<Composition
				id="Launch"
				component={Film}
				durationInFrames={FILM_FRAMES}
				fps={FPS}
				width={STAGES.landscape.width}
				height={STAGES.landscape.height}
				defaultProps={{ format: "landscape" }}
			/>
			<Composition
				id="LaunchVertical"
				component={Film}
				durationInFrames={FILM_FRAMES}
				fps={FPS}
				width={STAGES.portrait.width}
				height={STAGES.portrait.height}
				defaultProps={{ format: "portrait" }}
			/>
			<Composition
				id="LaunchSquare"
				component={Film}
				durationInFrames={FILM_FRAMES}
				fps={FPS}
				width={STAGES.square.width}
				height={STAGES.square.height}
				defaultProps={{ format: "square" }}
			/>
		</>
	);
}
