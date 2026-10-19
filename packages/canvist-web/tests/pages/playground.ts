import { createEditor } from "../../src/mod.ts";

const element = document.querySelector<HTMLElement>("#editor");

if (element === null) {
	throw new Error("missing #editor");
}

const editor = await createEditor(element, {
	content: {
		format: "markdown",
		input: [
			"# canvist on the web",
			"",
			"Every pixel here is painted on a **canvas**, measured by the browser, so it wraps exactly like the *server-rendered* HTML. Kerning: AVATAR, Wa, To, ff fi fl.",
			"",
			"- Bullets with `inline code`",
			"- [links](https://canvist.dev) and ~~strike~~",
			"",
			"> A quote, with a bar.",
			"",
			"```",
			"let width = measure(text, font);",
			"```",
			"",
			"עברית ועוד, العربية أيضاً, हिन्दी देवनागरी, 日本語のテキスト, emoji 🎨🖌️👩‍💻",
		].join("\n"),
	},
});
editor.setFocused(true);
await editor.ready();
const stats = document.querySelector("#stats");

if (stats !== null) {
	stats.textContent = JSON.stringify(
		{ frame: editor.lastFrame, stats: editor.stats() },
		null,
		2,
	);
}

Object.assign(globalThis, { editor });
