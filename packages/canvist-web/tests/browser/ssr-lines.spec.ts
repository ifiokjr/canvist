/**
 * Where Chromium breaks the lines of plain paragraphs set in Inter, kept in
 * `crates/canvist_layout/tests/fixtures/browser-lines.json`. A Rust test
 * lays the same text out with `parley` and the same font file, so server
 * rendering is held to the browser's line breaks.
 *
 * Set `CANVIST_UPDATE_GOLDENS=1` to record the file again; otherwise the
 * test fails when Chromium's breaks drift from it.
 *
 * @module
 */
import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { type Fixture, FIXTURES } from "../parity/fixtures.ts";

const GOLDEN = new URL(
	"../../../../crates/canvist_layout/tests/fixtures/browser-lines.json",
	import.meta.url,
);
const DEFAULT_WIDTH = 480;
const DEFAULT_SIZE = 16;
/** Fixtures set in Inter, the font `parley` can load on the server. */
const INTER_FIXTURES = [
	"inter",
	"size-10",
	"size-13",
	"size-24",
	"size-40",
	"size-64",
];

/** One paragraph's text and its lines, as UTF-16 `[start, end)` ranges. */
interface Lines {
	readonly text: string;
	readonly lines: readonly (readonly [number, number])[];
}

/** The texts of the fixture's top-level paragraphs that hold one unmarked text run. */
function plainParagraphs(fixture: Fixture): string[] {
	return (fixture.doc.content ?? []).flatMap((block) => {
		const content = "content" in block ? block.content ?? [] : [];
		const [only] = content;
		const plain = block.type === "paragraph" && content.length === 1 &&
			only !== undefined &&
			"text" in only && only.marks === undefined;

		return plain ? [only.text] : [];
	});
}

test.skip(
	({ browserName }) => browserName !== "chromium",
	"the golden records Chromium's line breaks",
);

test("Chromium's line breaks for plain Inter paragraphs", async ({ page }) => {
	const cases = [];

	for (const id of INTER_FIXTURES) {
		const fixture = FIXTURES.find((candidate) => candidate.id === id);

		if (fixture === undefined) {
			throw new Error(`no fixture ${id}`);
		}

		await page.goto(`/parity.html?fixture=${id}`);
		await page.waitForFunction(() =>
			(globalThis as { parity?: { ready: boolean } }).parity?.ready === true
		);
		const texts = plainParagraphs(fixture);
		const lines: Lines[] = await page.evaluate((wanted) => {
			const editor = (globalThis as unknown as {
				editor: {
					geometry(): { blocks: { text?: string; stops?: number[][] }[] };
				};
			}).editor;

			return editor.geometry().blocks
				.filter((block) =>
					block.text !== undefined && wanted.includes(block.text)
				)
				.map((block) => {
					const text = block.text ?? "";
					const starts: number[] = [];
					let top = Number.NaN;

					for (const [offset = 0, , , lineTop = 0] of block.stops ?? []) {
						if (lineTop !== top && offset < text.length) {
							starts.push(offset);
							top = lineTop;
						}
					}

					const ranges = starts.map((
						start,
						index,
					): [number, number] => [start, starts[index + 1] ?? text.length]);
					return { text, lines: ranges };
				});
		}, texts);

		cases.push({
			fixture: id,
			width: fixture.width ?? DEFAULT_WIDTH,
			bodyFamily: fixture.theme?.bodyFamily ?? "",
			size: fixture.theme?.size ?? DEFAULT_SIZE,
			letterSpacing: fixture.theme?.letterSpacing ?? 0,
			texts: lines,
		});
	}

	const recorded = `${
		JSON.stringify(
			{ browser: "chromium", breaks: "chromium", cases },
			null,
			"\t",
		)
	}\n`;

	if (process.env.CANVIST_UPDATE_GOLDENS !== undefined) {
		await writeFile(GOLDEN, recorded);
		return;
	}

	expect(recorded).toBe(await readFile(GOLDEN, "utf8"));
});
