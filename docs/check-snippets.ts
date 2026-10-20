/**
 * Checks every code sample in the docs against the real packages, so a page
 * can't drift from the API it teaches.
 *
 * - `rust`: compiled and run as doc tests. `docs/src/lib.rs` includes each
 *   page; this script fails if a page with Rust samples is missing there.
 * - `ts`: type-checked with `deno check`, then run with `deno run`. A sample
 *   whose info string has `no_run` (it needs a network peer, for example) is
 *   only type-checked.
 * - `dart`: analyzed with `dart analyze --fatal-infos` inside the `canvist`
 *   package, then run with `dart run` unless marked `no_run`. Samples that
 *   import Flutter are analyzed inside the Flutter example app instead.
 *
 * Output a sample prints is checked too: the comment lines straight after a
 * `console.log(...)` or `print(...)` line are the output it must produce.
 *
 * Every fence needs a language, because rustdoc compiles untagged fences as
 * Rust. Languages not listed above (`sh`, `json`, `html`, `text`, ...) are
 * shown, not checked.
 *
 * ```sh
 * deno run --allow-read --allow-write --allow-run --allow-env --allow-net docs/check-snippets.ts ts
 * deno run --allow-read --allow-write --allow-run --allow-env --allow-net docs/check-snippets.ts dart flutter
 * ```
 *
 * With no arguments it checks fences and the Rust page list only.
 */

import { dirname, fromFileUrl, join, relative } from "jsr:@std/path@^1";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const DOCS = join(ROOT, "docs/src");
const SOURCES = [DOCS, join(ROOT, "skills")];
const RUST_HARNESS = join(ROOT, "docs/src/lib.rs");

const CHECKED = new Set(["rust", "ts", "dart"]);
const SHOWN = new Set([
	"sh",
	"json",
	"html",
	"text",
	"toml",
	"yaml",
	"swift",
	"kotlin",
	"tsx",
	"vue",
	"markdown",
	"diff",
]);

interface Sample {
	file: string;
	line: number;
	lang: string;
	meta: string[];
	code: string;
}

type Target = "ts" | "dart" | "flutter";

async function* markdownFiles(dir: string): AsyncGenerator<string> {
	for await (const entry of Deno.readDir(dir)) {
		const path = join(dir, entry.name);
		if (entry.isDirectory) yield* markdownFiles(path);
		else if (entry.name.endsWith(".md")) yield path;
	}
}

/** Every fenced block in `markdown`, with its language and info-string flags. */
export function samples(file: string, markdown: string): Sample[] {
	const found: Sample[] = [];
	const lines = markdown.split("\n");
	for (let index = 0; index < lines.length; index++) {
		const open = /^(\s*)(`{3,})(.*)$/.exec(lines[index] ?? "");
		if (!open) continue;
		const [, indent = "", fence = "", info = ""] = open;
		const [lang = "", ...meta] = info.trim().split(/\s+/).filter(Boolean);
		const body: string[] = [];
		let end = index + 1;
		while (
			end < lines.length && !(lines[end] ?? "").startsWith(indent + fence)
		) {
			body.push((lines[end] ?? "").slice(indent.length));
			end++;
		}
		found.push({
			file,
			line: index + 1,
			lang,
			meta,
			code: body.join("\n") + "\n",
		});
		index = end;
	}
	return found;
}

/**
 * The output a sample promises: the comment lines right after each line that
 * prints. `// <h1>Hi</h1>` after `console.log(html)` expects `<h1>Hi</h1>`.
 */
export function expectedOutput(code: string): string[] | undefined {
	const lines = code.split("\n");
	const expected: string[] = [];
	let promised = false;
	for (let index = 0; index < lines.length; index++) {
		if (!/^\s*(console\.log|print)\(/.test(lines[index] ?? "")) continue;
		for (let next = index + 1; next < lines.length; next++) {
			const comment = /^\s*\/\/ ?(.*)$/.exec(lines[next] ?? "");
			if (!comment) break;
			expected.push(comment[1] ?? "");
			promised = true;
		}
	}
	return promised ? expected : undefined;
}

function where(sample: Sample): string {
	return `${relative(ROOT, sample.file)}:${sample.line}`;
}

async function run(
	command: string,
	args: string[],
	cwd: string,
): Promise<{ ok: boolean; stdout: string; stderr: string }> {
	const output = await new Deno.Command(command, {
		args,
		cwd,
		stdout: "piped",
		stderr: "piped",
	})
		.output();
	const decoder = new TextDecoder();
	return {
		ok: output.success,
		stdout: decoder.decode(output.stdout),
		stderr: decoder.decode(output.stderr),
	};
}

function fileName(sample: Sample, index: number, extension: string): string {
	const stem = relative(ROOT, sample.file).toLowerCase().replace(
		/[^a-z0-9]+/g,
		"_",
	);
	return `${stem}_${sample.line}_${index}.${extension}`;
}

class Failures {
	readonly messages: string[] = [];
	add(sample: Sample | undefined, message: string): void {
		this.messages.push(
			sample === undefined ? message : `${where(sample)}: ${message}`,
		);
	}
}

function checkOutput(sample: Sample, stdout: string, failures: Failures): void {
	const expected = expectedOutput(sample.code);
	if (expected === undefined) return;
	const actual = stdout.trimEnd().split("\n");
	if (actual.join("\n") !== expected.join("\n")) {
		failures.add(
			sample,
			`printed\n${actual.join("\n")}\nbut the comments promise\n${
				expected.join("\n")
			}`,
		);
	}
}

async function checkTypeScript(
	found: Sample[],
	failures: Failures,
): Promise<void> {
	const dir = join(ROOT, "target/doc-snippets/ts");
	await Deno.remove(dir, { recursive: true }).catch(() => {});
	await Deno.mkdir(dir, { recursive: true });
	const headless = join(ROOT, "packages/canvist-headless");
	const web = join(ROOT, "packages/canvist-web");
	await Deno.writeTextFile(
		join(dir, "deno.json"),
		JSON.stringify({
			imports: {
				"@canvist/headless": `${headless}/src/mod.ts`,
				"@canvist/headless/extensions":
					`${headless}/src/extensions.generated.ts`,
				"@canvist/headless/full": `${headless}/src/full.ts`,
				"@canvist/headless/ssr": `${headless}/src/ssr.ts`,
				"@canvist/web": `${web}/src/mod.ts`,
				"@canvist/web/fonts": `${web}/src/fonts.ts`,
				"@canvist/web/menus": `${web}/src/menus.ts`,
				// Workers import the module through their bundler; it is a WebAssembly.Module.
				"@canvist/headless/wasm/canvist_headless_wasm_bg.wasm":
					"./worker-wasm.ts",
				"yjs": "npm:yjs@^13.6.33",
				"y-websocket": "npm:y-websocket@^3.0.0",
				"y-protocols/": "npm:/y-protocols@^1.0.7/",
				"@hocuspocus/provider": "npm:@hocuspocus/provider@^3.4.0",
			},
			compilerOptions: { strict: true, lib: ["deno.window", "dom"] },
		}),
	);
	await Deno.writeTextFile(
		join(dir, "worker-wasm.ts"),
		"declare const wasm: WebAssembly.Module;\nexport default wasm;\n",
	);
	const files = found.map((sample, index) => {
		const name = fileName(sample, index, "ts");
		return { sample, name, path: join(dir, name) };
	});
	for (const { sample, path } of files) {
		await Deno.writeTextFile(path, sample.code);
	}
	if (files.length === 0) return;

	const check = await run("deno", [
		"check",
		"--quiet",
		...files.map((file) => file.name),
	], dir);
	if (!check.ok) failures.add(undefined, `deno check failed:\n${check.stderr}`);

	for (const { sample, name } of files) {
		if (sample.meta.includes("no_run")) continue;
		const result = await run("deno", [
			"run",
			"--quiet",
			"--allow-read",
			"--allow-env",
			name,
		], dir);
		if (!result.ok) failures.add(sample, `deno run failed:\n${result.stderr}`);
		else checkOutput(sample, result.stdout, failures);
	}
}

const DART = Deno.env.get("DART") ?? "dart";
const FLUTTER = Deno.env.get("FLUTTER") ?? "flutter";

async function checkDart(
	found: Sample[],
	failures: Failures,
	target: "dart" | "flutter",
): Promise<void> {
	const pkg = target === "dart"
		? join(ROOT, "platforms/dart")
		: join(ROOT, "platforms/flutter/example");
	const dir = join(pkg, "doc_snippets");
	await Deno.remove(dir, { recursive: true }).catch(() => {});
	if (found.length === 0) return;
	await Deno.mkdir(dir, { recursive: true });
	const files = found.map((sample, index) => ({
		sample,
		name: fileName(sample, index, "dart"),
	}));
	for (const { sample, name } of files) {
		await Deno.writeTextFile(join(dir, name), sample.code);
	}

	const tool = target === "dart" ? DART : FLUTTER;
	const analyze = await run(
		tool,
		["analyze", "--fatal-infos", "doc_snippets"],
		pkg,
	);
	if (!analyze.ok) {
		failures.add(
			undefined,
			`${tool} analyze failed:\n${analyze.stdout}${analyze.stderr}`,
		);
	}

	if (target === "flutter") return;
	for (const { sample, name } of files) {
		if (sample.meta.includes("no_run")) continue;
		const result = await run(DART, ["run", join("doc_snippets", name)], pkg);
		if (!result.ok) failures.add(sample, `dart run failed:\n${result.stderr}`);
		else checkOutput(sample, result.stdout, failures);
	}
}

/** Pages with Rust samples must be doc tests, or their samples go unchecked. */
async function checkRustHarness(
	found: Sample[],
	failures: Failures,
): Promise<void> {
	const harness = await Deno.readTextFile(RUST_HARNESS);
	const pages = new Set(found.map((sample) => relative(DOCS, sample.file)));
	for (const page of pages) {
		if (!harness.includes(`"${page}"`)) {
			failures.add(
				undefined,
				`docs/src/lib.rs does not include ${page}, so its Rust samples are untested`,
			);
		}
	}
}

function targetOf(sample: Sample): Target | undefined {
	if (sample.lang === "ts") return "ts";
	if (sample.lang !== "dart") return undefined;
	return /package:(flutter|canvist_flutter)\//.test(sample.code)
		? "flutter"
		: "dart";
}

if (import.meta.main) {
	const targets = new Set(Deno.args);
	const failures = new Failures();
	const all: Sample[] = [];
	for (const source of SOURCES) {
		for await (const file of markdownFiles(source)) {
			all.push(...samples(file, await Deno.readTextFile(file)));
		}
	}

	for (const sample of all) {
		if (sample.lang === "") {
			failures.add(
				sample,
				"a fence needs a language (rustdoc compiles untagged fences as Rust)",
			);
		} else if (!CHECKED.has(sample.lang) && !SHOWN.has(sample.lang)) {
			failures.add(
				sample,
				`unknown fence language \`${sample.lang}\`; add it to SHOWN or check it`,
			);
		}
	}
	await checkRustHarness(
		all.filter((sample) => sample.lang === "rust"),
		failures,
	);

	const byTarget = (target: Target) =>
		all.filter((sample) => targetOf(sample) === target);
	if (targets.has("ts")) await checkTypeScript(byTarget("ts"), failures);
	if (targets.has("dart")) await checkDart(byTarget("dart"), failures, "dart");
	if (targets.has("flutter")) {
		await checkDart(byTarget("flutter"), failures, "flutter");
	}

	const counts = ["rust", "ts", "dart"].map((lang) =>
		`${all.filter((sample) => sample.lang === lang).length} ${lang}`
	);
	if (failures.messages.length > 0) {
		console.error(failures.messages.join("\n\n"));
		console.error(
			`\n${failures.messages.length} problem(s) in the docs samples.`,
		);
		Deno.exit(1);
	}
	console.log(
		`Docs samples OK (${counts.join(", ")}; checked: ${
			[...targets].join(", ") || "fences only"
		}).`,
	);
}
