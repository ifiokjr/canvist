/**
 * Print the gzipped size of each file in `report/bundle` (built by
 * `vite build --config tests/bundle/vite.config.ts`) and their total, as
 * a Markdown table. Exits with an error when the total is over `--budget`.
 *
 * @module
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { gzipSync } from "node:zlib";

const root = new URL("../report/bundle/", import.meta.url).pathname;
const budgetFlag = process.argv.indexOf("--budget");
const budget = budgetFlag === -1
	? Infinity
	: Number(process.argv[budgetFlag + 1]);

function files(directory: string): string[] {
	return readdirSync(directory).flatMap((name) => {
		const path = join(directory, name);
		return statSync(path).isDirectory() ? files(path) : [path];
	});
}

const shipped = files(root).filter((path) => /\.(js|wasm|css)$/.test(path));
const rows = shipped.map((path) => ({
	file: relative(root, path),
	bytes: gzipSync(readFileSync(path), { level: 9 }).length,
}));
const total = rows.reduce((sum, row) => sum + row.bytes, 0);

console.log("| File | Gzipped bytes |");
console.log("| ---- | ------------- |");

for (const row of rows.sort((a, b) => b.bytes - a.bytes)) {
	console.log(`| ${row.file} | ${row.bytes.toLocaleString("en")} |`);
}

console.log(
	`| **Total** (budget ${budget.toLocaleString("en")}) | **${
		total.toLocaleString("en")
	}** |`,
);

if (total > budget) {
	console.error(
		`@canvist/web ships ${total} gzipped bytes, over its ${budget} budget`,
	);
	process.exitCode = 1;
}
