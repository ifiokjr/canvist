/**
 * The pixel parity matrix: documents that stress font rendering, each
 * rendered as the canvas editor, as server-rendered HTML, and as SVG.
 *
 * Fixtures are ProseMirror JSON so every mark is explicit. Web fonts live
 * in `tests/fonts/` as subsets of the characters below
 * (`node scripts/fonts.ts` rebuilds them), and the HTML and the canvas load
 * the same files.
 *
 * @module
 */
import type {
	BlockJson,
	DocJson,
	InlineJson,
	MarkJson,
	Registered,
	StarterKit,
} from "@canvist/headless";
import type { ThemeOptions } from "../../src/editor.ts";

type Kit = Registered<StarterKit>;
type Block = BlockJson<Kit>;
type Inline = InlineJson<Kit>;
type Mark = MarkJson<Kit>["type"] | MarkJson<Kit>;

/** A web font face a fixture loads. */
export interface WebFont {
	readonly family: string;
	/** The file in `tests/fonts/`. */
	readonly file: string;
	readonly weight?: string;
	readonly style?: string;
	readonly unicodeRange?: string;
}

/** One document in the matrix. */
export interface Fixture {
	readonly id: string;
	readonly title: string;
	readonly doc: DocJson<Kit>;
	readonly theme?: ThemeOptions;
	readonly fonts?: readonly WebFont[];
	/** The document width in CSS pixels. Default 480. */
	readonly width?: number;
}

export function text(value: string, ...marks: readonly Mark[]): Inline {
	return marks.length === 0 ? { type: "text", text: value } : {
		type: "text",
		text: value,
		marks: marks.map((mark): MarkJson<Kit> =>
			typeof mark === "string" ? { type: mark } as MarkJson<Kit> : mark
		),
	};
}

export function paragraph(...content: readonly (Inline | string)[]): Block {
	return {
		type: "paragraph",
		content: content.map((inline) =>
			typeof inline === "string" ? text(inline) : inline
		),
	};
}

function heading(level: 1 | 2 | 3, value: string): Block {
	return { type: "heading", attrs: { level }, content: [text(value)] };
}

function list(
	type: "bulletList" | "orderedList",
	...items: readonly Block[][]
): Block {
	return {
		type,
		content: items.map((content): Block => ({ type: "listItem", content })),
	};
}

function doc(...content: readonly Block[]): DocJson<Kit> {
	return { type: "doc", content };
}

const LATIN =
	"The quick brown fox jumps over the lazy dog. AVATAR, Wa, To, Ty, LT, " +
	"“quotes” and ‘apostrophes’, fi fl ffi ffl, 0123456789, ½ — en–dash… " +
	"(parentheses) [brackets] {braces} and a-very-long-hyphenated-compound " +
	"https://example.com/a/path/that/has/to/wrap?with=query&and=more";

/** Latin text with every inline mark, in the theme's body family. */
function latinDoc() {
	return doc(
		heading(1, "Kerning, ligatures, wrapping"),
		paragraph(LATIN),
		paragraph(
			text("Bold", "bold"),
			", ",
			text("italic", "italic"),
			", ",
			text("bold italic", "bold", "italic"),
			", ",
			text("underlined", "underline"),
			", ",
			text("struck", "strike"),
			", ",
			text("inline code()", "code"),
			", ",
			text("a link", { type: "link", attrs: { href: "https://canvist.dev" } }),
			", and ",
			text("more code with a background", "code"),
			" across the line.",
		),
		list("bulletList", [
			paragraph("A bullet with enough words to wrap onto a second line here."),
		], [
			paragraph("Second bullet"),
		]),
		list("orderedList", [paragraph("First")], [paragraph("Second, numbered")]),
		{
			type: "blockquote",
			content: [
				paragraph(
					"A quotation with a bar, wrapping once or twice inside its padding.",
				),
			],
		},
		{
			type: "codeBlock",
			content: [text('fn main() {\n\tprintln!("tabs\tand spaces");\n}')],
		},
	);
}

const SYSTEM_SANS = "ui-sans-serif, system-ui, sans-serif";
const SYSTEM_SERIF = "ui-serif, Georgia, serif";
const SYSTEM_MONO = "ui-monospace, Menlo, monospace";

const INTER: readonly WebFont[] = [
	{ family: "Inter", file: "inter-variable.ttf", weight: "100 900" },
	{
		family: "Inter",
		file: "inter-variable-italic.ttf",
		weight: "100 900",
		style: "italic",
	},
];

/** Noto Sans JP split in two by `unicode-range`, so only the subsets in use load. */
const NOTO_JP: readonly WebFont[] = [
	{
		family: "Noto Sans JP",
		file: "noto-sans-jp-kana.ttf",
		unicodeRange: "U+3000-30FF",
	},
	{
		family: "Noto Sans JP",
		file: "noto-sans-jp-kanji.ttf",
		unicodeRange: "U+4E00-9FFF",
	},
];

const NOTO_ARABIC: readonly WebFont[] = [{
	family: "Noto Sans Arabic",
	file: "noto-sans-arabic.ttf",
}];
const NOTO_HEBREW: readonly WebFont[] = [{
	family: "Noto Sans Hebrew",
	file: "noto-sans-hebrew.ttf",
}];
const NOTO_DEVANAGARI: readonly WebFont[] = [{
	family: "Noto Sans Devanagari",
	file: "noto-sans-devanagari.ttf",
}];
const NOTO_COLR: readonly WebFont[] = [{
	family: "Noto Color Emoji COLRv1",
	file: "noto-colrv1.ttf",
}];
const TWEMOJI_SVG: readonly WebFont[] = [{
	family: "Twemoji Mozilla",
	file: "twemoji-mozilla.ttf",
}];

/** The letter spacing of the letter-spacing fixture, in CSS px. */
const LETTER_SPACING = 1.5;
/** How many characters of {@link LATIN} the font size fixtures set, so the largest sizes stay short. */
const SIZE_SAMPLE_LENGTH = 120;

const EMOJI =
	"Emoji 🎨 🖌️ 👩‍💻 👍🏽 🇯🇵 🏳️‍🌈 ❤️ 1️⃣ ☕ — keycaps, flags, ZWJ, skin tones.";

export const FIXTURES: readonly Fixture[] = [
	{
		id: "system-sans",
		title: "System sans-serif",
		doc: latinDoc(),
		theme: { bodyFamily: SYSTEM_SANS },
	},
	{
		id: "system-serif",
		title: "System serif",
		doc: latinDoc(),
		theme: { bodyFamily: SYSTEM_SERIF },
	},
	{
		id: "system-mono",
		title: "System monospace",
		doc: latinDoc(),
		theme: { bodyFamily: SYSTEM_MONO, codeFamily: SYSTEM_MONO },
	},
	{
		id: "inter",
		title: "Inter, a variable web font",
		doc: latinDoc(),
		theme: { bodyFamily: `Inter, ${SYSTEM_SANS}` },
		fonts: INTER,
	},
	{
		id: "letter-spacing",
		title: "Letter spacing",
		doc: doc(paragraph(LATIN)),
		theme: {
			bodyFamily: `Inter, ${SYSTEM_SANS}`,
			letterSpacing: LETTER_SPACING,
		},
		fonts: INTER,
	},
	...[10, 13, 24, 40, 64].map((size) => ({
		id: `size-${size}`,
		title: `Inter at ${size} px`,
		doc: doc(paragraph(LATIN.slice(0, SIZE_SAMPLE_LENGTH))),
		theme: { bodyFamily: `Inter, ${SYSTEM_SANS}`, size },
		fonts: INTER,
	})),
	{
		id: "cjk",
		title: "Noto Sans JP, two unicode-range subsets",
		doc: doc(
			paragraph(
				"日本語のテキストは単語の間に空白がないので、どの文字の間でも折り返せます。句読点「、」と「。」は行頭に来ません。",
			),
			paragraph(
				"カタカナとひらがな、漢字が混ざった文章。English words mixed in 日本語。",
			),
		),
		theme: { bodyFamily: `"Noto Sans JP", ${SYSTEM_SANS}` },
		fonts: NOTO_JP,
	},
	{
		id: "rtl",
		title: "Arabic and Hebrew, right to left",
		doc: doc(
			paragraph(
				"العربية تكتب من اليمين إلى اليسار، والحروف تتصل ببعضها. الرقم 2024 يبقى من اليسار إلى اليمين.",
			),
			paragraph(
				"עברית נכתבת מימין לשמאל, עם מילים באנגלית like this באמצע המשפט.",
			),
			paragraph(
				"Mixed: English with עברית and العربية in one line, then more English.",
			),
		),
		theme: {
			bodyFamily: `"Noto Sans Arabic", "Noto Sans Hebrew", ${SYSTEM_SANS}`,
		},
		fonts: [...NOTO_ARABIC, ...NOTO_HEBREW],
	},
	{
		id: "devanagari",
		title: "Devanagari",
		doc: doc(
			paragraph(
				"हिन्दी देवनागरी लिपि में लिखी जाती है। संयुक्ताक्षर जैसे क्ष, त्र, ज्ञ और श्र सही दिखने चाहिए।",
			),
		),
		theme: { bodyFamily: `"Noto Sans Devanagari", ${SYSTEM_SANS}` },
		fonts: NOTO_DEVANAGARI,
	},
	{
		id: "emoji-system",
		title: "System emoji",
		doc: doc(paragraph(EMOJI)),
		theme: { bodyFamily: SYSTEM_SANS },
	},
	{
		id: "emoji-colrv1",
		title: "Noto Color Emoji (COLRv1)",
		doc: doc(paragraph(EMOJI)),
		theme: {
			bodyFamily:
				`ui-sans-serif, system-ui, "Noto Color Emoji COLRv1", sans-serif`,
		},
		fonts: NOTO_COLR,
	},
	{
		id: "emoji-svg",
		title: "Twemoji (OpenType SVG)",
		doc: doc(paragraph(EMOJI)),
		theme: {
			bodyFamily: `ui-sans-serif, system-ui, "Twemoji Mozilla", sans-serif`,
		},
		fonts: TWEMOJI_SVG,
	},
];

/** The fixture with `id`. */
export function fixture(id: string): Fixture {
	const found = FIXTURES.find((candidate) => candidate.id === id);

	if (found === undefined) {
		throw new Error(`no parity fixture ${id}`);
	}

	return found;
}

/** Every character a fixture's text uses, for subsetting its fonts. */
export function characters(fixture: Fixture): string {
	const seen = new Set<string>();

	for (const block of fixture.doc.content ?? []) {
		collectCharacters(block, seen);
	}

	return [...seen].join("");
}

/** Add every character of `node`'s text, and its descendants', to `seen`. */
function collectCharacters(node: Block | Inline, seen: Set<string>): void {
	if ("text" in node) {
		for (const character of node.text) {
			seen.add(character);
		}
	}

	if (!("content" in node)) {
		return;
	}

	for (const child of node.content ?? []) {
		collectCharacters(child, seen);
	}
}
