/**
 * Write `src/emoji/gemoji.txt`, the emoji and shortcodes the `emoji`
 * extension knows, from GitHub's gemoji (MIT licensed).
 *
 * Each line is an emoji, a tab, and its shortcodes separated by spaces; the
 * first is its name. The file is compact because it is compiled into every
 * build that has the `emoji` feature, including WebAssembly ones.
 *
 * ```sh
 * deno run --allow-net=raw.githubusercontent.com --allow-write=src/emoji crates/canvist_extensions/scripts/emoji.ts
 * ```
 *
 * @module
 */

/** A pinned gemoji commit, so the table changes only when someone bumps it. */
const GEMOJI =
	"https://raw.githubusercontent.com/github/gemoji/fadaeaf1f1a9be82b321316a6c5502e43138b2f6/db/emoji.json";
const OUTPUT = new URL("../src/emoji/gemoji.txt", import.meta.url);

interface Entry {
	readonly emoji: string;
	readonly aliases: readonly string[];
}

const response = await fetch(GEMOJI);

if (!response.ok) {
	throw new Error(`fetching gemoji failed: ${response.status}`);
}

const entries = (await response.json()) as Entry[];
const lines = entries
	.filter((entry) => entry.aliases.length > 0)
	.map((entry) => `${entry.emoji}\t${entry.aliases.join(" ")}`);

await Deno.writeTextFile(OUTPUT, `${lines.join("\n")}\n`);
console.log(`wrote ${lines.length} emoji`);
