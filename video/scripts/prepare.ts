/**
 * Produces everything the film reads from `public/`, from the repository
 * itself, so a render always shows the product as it is today:
 *
 * - `footage/`: real canvist renders, from `cargo run -p canvist_footage`.
 * - `footage/budgets.json`: the performance budget table in
 *   `docs/src/guides/performance.md`.
 * - `footage/typed.json`: TypeScript declarations generated from the
 *   StarterKit's `metadata_json()`, and the compiler's real diagnostics for
 *   a snippet that calls an extension the editor never registered.
 * - `fonts/` and `brand/`: the bundled Atkinson Hyperlegible faces and the
 *   Wet Caret masters.
 *
 * Run with `npm run prepare:footage` (Node 22.18 or later runs TypeScript
 * directly).
 */

import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { declarations, SNIPPET } from "./typed.ts";

const VIDEO = join(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = join(VIDEO, "..");
const PUBLIC = join(VIDEO, "public");
const FOOTAGE = join(PUBLIC, "footage");

function run(command: string, args: readonly string[], cwd: string): string {
	const result = spawnSync(command, args, {
		cwd,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
	});

	if (result.error) {
		throw result.error;
	}

	// tsc exits with 1 or 2 when it reports diagnostics, which is what we want.
	if (result.status === null || result.status > 2) {
		throw new Error(
			`${command} ${args.join(" ")} exited with ${result.status}`,
		);
	}

	return result.stdout;
}

function footage(): void {
	const result = spawnSync(
		"cargo",
		[
			"run",
			"--release",
			"--locked",
			"-q",
			"-p",
			"canvist_footage",
			"--",
			relative(ROOT, FOOTAGE),
		],
		{ cwd: ROOT, stdio: "inherit" },
	);

	if (result.status !== 0) {
		throw new Error("cargo run -p canvist_footage failed");
	}
}

function assets(): void {
	mkdirSync(join(PUBLIC, "fonts"), { recursive: true });
	const faces = [
		"crates/canvist_layout/tests/fonts/AtkinsonHyperlegible-Regular.ttf",
		"tools/footage/fonts/AtkinsonHyperlegible-Bold.ttf",
		"tools/footage/fonts/AtkinsonHyperlegible-Italic.ttf",
		"tools/footage/fonts/AtkinsonHyperlegible-BoldItalic.ttf",
	];

	for (const face of faces) {
		copyFileSync(
			join(ROOT, face),
			join(PUBLIC, "fonts", face.split("/").pop() ?? face),
		);
	}

	mkdirSync(join(PUBLIC, "brand"), { recursive: true });
	const brand = Object.fromEntries(
		["mark", "lockup"].map((
			name,
		) => [name, paths(join(ROOT, `assets/brand/vector/${name}.svg`))]),
	);
	writeFileSync(join(PUBLIC, "brand/brand.json"), JSON.stringify(brand));
}

/** The view box and the ink and paint paths of a Wet Caret master. */
function paths(file: string): { viewBox: string; ink: string; paint: string } {
	const svg = readFileSync(file, "utf8");
	const viewBox = svg.match(/viewBox="([^"]+)"/)?.[1];
	const found = new Map(
		[...svg.matchAll(/<path fill="(#[0-9A-Fa-f]{6})" d="([^"]+)"/g)].map((
			match,
		) => [match[1], match[2]]),
	);
	const ink = found.get("#17141F");
	const paint = found.get("#3B2CF5");

	if (!viewBox || !ink || !paint) {
		throw new Error(
			`${file} is not a light Wet Caret master (ink #17141F, paint #3B2CF5)`,
		);
	}

	return { viewBox, ink, paint };
}

interface Budget {
	readonly metric: string;
	readonly budget: string;
	readonly today: string;
}

/** The rows of the performance budget table, footnote markers removed. */
function budgets(): Budget[] {
	const doc = readFileSync(
		join(ROOT, "docs/src/guides/performance.md"),
		"utf8",
	);
	const section = doc.split("## Performance budgets")[1]?.split("\n## ")[0];

	if (!section) {
		throw new Error(
			"docs/src/guides/performance.md has no performance budget table",
		);
	}

	return section
		.split("\n")
		.filter((line) => line.startsWith("|") && !line.includes("---"))
		.slice(1)
		.map((line) =>
			line.split("|").slice(1, -1).map((cell) =>
				cell.replace(/[¹²³]/g, "").trim()
			)
		)
		.map(([metric = "", budget = "", today = ""]) => ({
			metric,
			budget,
			today,
		}));
}

interface Diagnostic {
	readonly line: number;
	readonly column: number;
	readonly code: number;
	readonly message: string;
}

/** Type-check the snippet against the generated declarations. */
function typed(): {
	snippet: string;
	commands: number;
	extensions: number;
	diagnostics: Diagnostic[];
} {
	const metadata: unknown = JSON.parse(
		readFileSync(join(FOOTAGE, "metadata.json"), "utf8"),
	);
	const generated = declarations(metadata);
	const dir = join(VIDEO, ".typed");
	mkdirSync(dir, { recursive: true });
	writeFileSync(join(dir, "canvist.d.ts"), generated.source);
	writeFileSync(join(dir, "demo.ts"), SNIPPET);

	const output = run(
		join(VIDEO, "node_modules/.bin/tsc"),
		[
			"--ignoreConfig",
			"--noEmit",
			"--strict",
			"--pretty",
			"false",
			"--target",
			"es2022",
			"--module",
			"preserve",
			"demo.ts",
		],
		dir,
	);
	const diagnostics = [
		...output.matchAll(/^demo\.ts\((\d+),(\d+)\): error TS(\d+): (.+)$/gm),
	].map((match) => ({
		line: Number(match[1]),
		column: Number(match[2]),
		code: Number(match[3]),
		message: match[4] ?? "",
	}));

	if (diagnostics.length === 0) {
		throw new Error(
			`expected the snippet to fail type-checking, tsc said:\n${output}`,
		);
	}

	return {
		snippet: SNIPPET,
		commands: generated.commands,
		extensions: generated.extensions,
		diagnostics,
	};
}

const started = performance.now();
footage();
assets();
writeFileSync(join(FOOTAGE, "budgets.json"), JSON.stringify(budgets()));
writeFileSync(join(FOOTAGE, "typed.json"), JSON.stringify(typed()));
console.error(
	`prepared public/ in ${((performance.now() - started) / 1000).toFixed(1)} s`,
);
