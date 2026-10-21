/**
 * Type-level tests for the media commands, checked by `tsc --noEmit` and
 * never run. Every `@ts-expect-error` must flag a real error, or the check
 * fails.
 *
 * @module
 */
import {
	audio,
	createEngine,
	file,
	image,
	twitch,
	youtube,
} from "../src/full.ts";

export async function mediaSourcesAreRequiredStrings(): Promise<void> {
	const engine = await createEngine([image, youtube, twitch, audio, file]);
	const doc = engine.create();

	doc.commands.setImage({ src: "cat.png", alt: "A cat" });
	doc.commands.setYoutubeVideo({ src: "https://youtu.be/x" });
	doc.commands.setTwitchVideo({ src: "https://twitch.tv/x" });
	doc.commands.setAudio({ src: "a.mp3" });
	doc.commands.insertFile({ url: "r.pdf" });

	// @ts-expect-error a source is required
	doc.commands.setImage({ alt: "A cat" });
	// @ts-expect-error a source can't be null, though the stored attribute defaults to it
	doc.commands.setImage({ src: null });
	// @ts-expect-error a video's source can't be null
	doc.commands.setYoutubeVideo({ src: null });
	// @ts-expect-error a file's URL can't be null
	doc.commands.insertFile({ url: null });
}
