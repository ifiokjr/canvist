/**
 * An editor under a virtual screen reader, for the screen reader smoke
 * tests: `?markdown=`, `?editable=false`, and `?placeholder=` configure the
 * editor, and `window.speak(steps)` moves the virtual cursor that many
 * times and returns every phrase it spoke.
 */
import { virtual } from "@guidepup/virtual-screen-reader";
import { createEditor } from "../../src/mod.ts";

const params = new URLSearchParams(location.search);
const editor = await createEditor(
	document.querySelector<HTMLElement>("#host")!,
	{
		content: { format: "markdown", input: params.get("markdown") ?? "" },
		label: "Notes",
		editable: params.get("editable") !== "false",
		placeholder: params.get("placeholder") ?? "",
	},
);
await editor.ready();
// The mirror refreshes when the browser is idle; read it as it is now.
editor.render();

/** Move the virtual cursor `steps` times and return what was spoken. */
async function speak(steps: number): Promise<string[]> {
	await virtual.start({ container: document.body });

	for (let step = 0; step < steps; step += 1) {
		await virtual.next();
	}

	const phrases = await virtual.spokenPhraseLog();
	await virtual.stop();

	return phrases;
}

Object.assign(globalThis, { editor, speak });
