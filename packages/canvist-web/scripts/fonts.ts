/**
 * Build the web fonts the parity tests load: subsets of open-licence fonts
 * holding just the characters the fixtures use, as TrueType so the browser
 * and the server's parley measurer read the same files.
 *
 *   node scripts/fonts.ts
 *
 * Sources are downloaded once into `node_modules/.cache/canvist-fonts/`.
 * Re-run after changing a fixture's text; the output in `tests/fonts/` is
 * committed.
 *
 * @module
 */
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import subsetFont from "subset-font";
import { characters, FIXTURES } from "../tests/parity/fixtures.ts";

const GOOGLE = "https://github.com/google/fonts/raw/main/ofl";

/** Where each source font comes from. `tests/fonts/NOTICE.md` lists each one's copyright and license. */
const SOURCES = {
	inter: `${GOOGLE}/inter/Inter%5Bopsz,wght%5D.ttf`,
	interItalic: `${GOOGLE}/inter/Inter-Italic%5Bopsz,wght%5D.ttf`,
	notoJp: `${GOOGLE}/notosansjp/NotoSansJP%5Bwght%5D.ttf`,
	notoArabic: `${GOOGLE}/notosansarabic/NotoSansArabic%5Bwdth,wght%5D.ttf`,
	notoHebrew: `${GOOGLE}/notosanshebrew/NotoSansHebrew%5Bwdth,wght%5D.ttf`,
	notoDevanagari:
		`${GOOGLE}/notosansdevanagari/NotoSansDevanagari%5Bwdth,wght%5D.ttf`,
	notoColr:
		"https://github.com/googlefonts/noto-emoji/raw/main/2D/fonts/Noto-COLRv1.ttf",
	twemoji:
		"https://github.com/mozilla/twemoji-colr/releases/download/v0.7.0/Twemoji.Mozilla.ttf",
} as const;

/** Each output file: its source, and a filter on the fixtures' characters. */
const OUTPUTS: readonly {
	readonly file: string;
	readonly source: keyof typeof SOURCES;
	readonly keep?: (codePoint: number) => boolean;
}[] = [
	{ file: "inter-variable.ttf", source: "inter" },
	{ file: "inter-variable-italic.ttf", source: "interItalic" },
	{
		file: "noto-sans-jp-kana.ttf",
		source: "notoJp",
		keep: (c) => c >= 0x3000 && c <= 0x30ff,
	},
	{
		file: "noto-sans-jp-kanji.ttf",
		source: "notoJp",
		keep: (c) => c >= 0x4e00 && c <= 0x9fff,
	},
	{ file: "noto-sans-arabic.ttf", source: "notoArabic" },
	{ file: "noto-sans-hebrew.ttf", source: "notoHebrew" },
	{ file: "noto-sans-devanagari.ttf", source: "notoDevanagari" },
	{ file: "noto-colrv1.ttf", source: "notoColr" },
	{ file: "twemoji-mozilla.ttf", source: "twemoji" },
];

/** ASCII, plus the joiners shaping relies on, which every real font has. */
const ASCII =
	Array.from({ length: 95 }, (_, index) => String.fromCodePoint(32 + index))
		.join("") +
	"\u200c\u200d";
const cache = new URL("../node_modules/.cache/canvist-fonts/", import.meta.url);
const output = new URL("../tests/fonts/", import.meta.url);

async function source(name: keyof typeof SOURCES): Promise<Buffer> {
	const file = new URL(`${name}.ttf`, cache);

	if (!existsSync(file)) {
		const response = await fetch(SOURCES[name]);

		if (!response.ok) {
			throw new Error(`${SOURCES[name]}: HTTP ${response.status}`);
		}

		await writeFile(file, Buffer.from(await response.arrayBuffer()));
	}

	return readFile(file);
}

await mkdir(cache, { recursive: true });
await mkdir(output, { recursive: true });

for (const entry of OUTPUTS) {
	const users = FIXTURES.filter((fixture) =>
		(fixture.fonts ?? []).some((font) => font.file === entry.file)
	);
	const text = [...new Set(ASCII + users.map(characters).join(""))]
		.filter((character) => entry.keep?.(character.codePointAt(0) ?? 0) ?? true)
		.join("");
	const subset = await subsetFont(await source(entry.source), text, {
		targetFormat: "truetype",
		// Keep every variation axis and the layout tables shaping needs.
		preserveNameIds: [1, 2, 3, 4, 5, 6, 16, 17],
	});
	await writeFile(new URL(entry.file, output), subset);
	console.log(
		`${entry.file}: ${text.length} characters, ${subset.byteLength} bytes`,
	);
}

const license = await fetch(`${GOOGLE}/inter/OFL.txt`);
await writeFile(new URL("OFL.txt", output), await license.text());
