/**
 * Fails when an npm package's tarball is missing a file its manifest points
 * at: every `exports` target, plus `main`, `module`, and `types`.
 *
 * `npm publish --dry-run` never checks this, so a package whose build step was
 * skipped publishes as `package.json` and a readme. Run it after building:
 *
 *   node tools/release/check_npm_pack.ts packages/canvist-headless packages/canvist-web
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

interface PackEntry {
	name: string;
	files: { path: string }[];
}

/** Every string in a manifest's `exports` map, whatever its nesting. */
function exportTargets(value: unknown): string[] {
	if (typeof value === "string") return [value];
	if (Array.isArray(value)) return value.flatMap(exportTargets);
	if (typeof value === "object" && value !== null) {
		return Object.values(value).flatMap(exportTargets);
	}
	return [];
}

function requiredFiles(manifest: Record<string, unknown>): string[] {
	const entries = [manifest["main"], manifest["module"], manifest["types"]]
		.filter((entry): entry is string => typeof entry === "string");
	const targets = [...entries, ...exportTargets(manifest["exports"])]
		// A `*` subpath pattern names many files, which the pack check can't
		// enumerate; the build's own tests cover those.
		.filter((target) => !target.includes("*"))
		.map((target) => target.replace(/^\.\//, ""));
	return [...new Set(["package.json", ...targets])].sort();
}

function missingFiles(directory: string): { name: string; missing: string[] } {
	const manifest = JSON.parse(
		readFileSync(join(directory, "package.json"), "utf8"),
	) as Record<string, unknown>;
	const output = execFileSync("npm", [
		"pack",
		"--dry-run",
		"--json",
		"--ignore-scripts",
	], {
		cwd: directory,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
	});
	const [entry] = JSON.parse(output) as PackEntry[];
	if (entry === undefined) {
		throw new Error(`npm pack reported nothing for ${directory}`);
	}
	const packed = new Set(entry.files.map((file) => file.path));
	return {
		name: entry.name,
		missing: requiredFiles(manifest).filter((file) => !packed.has(file)),
	};
}

const directories = process.argv.slice(2);
if (directories.length === 0) {
	console.error(
		"usage: node tools/release/check_npm_pack.ts <package directory>...",
	);
	process.exit(2);
}

let failed = false;
for (const directory of directories) {
	const { name, missing } = missingFiles(directory);
	if (missing.length === 0) {
		console.log(`✔ ${name}: every exported file is in the tarball`);
		continue;
	}
	failed = true;
	console.error(`✖ ${name} would publish without: ${missing.join(", ")}`);
}
process.exit(failed ? 1 : 0);
