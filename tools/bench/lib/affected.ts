/**
 * Which benchmarks and artifacts a change can affect.
 *
 * A crate's benchmarks and build outputs depend on its own files and on every
 * workspace crate it reaches through path dependencies, of any kind. A change
 * to any other crate, to the docs, or to an unrelated package can't change
 * them, so CI skips them. A change to a global path, such as `Cargo.lock` or
 * the toolchain, affects everything.
 *
 * @module
 */
import { execFileSync } from "node:child_process";
import { relative } from "node:path";

/** A workspace member and the workspace members it depends on by path. */
export interface WorkspaceCrate {
	readonly name: string;
	/** Relative to the workspace root, without a trailing slash. */
	readonly dir: string;
	readonly dependencies: readonly string[];
	/** The names of its `[[bench]]` targets. */
	readonly benches: readonly string[];
}

/** What can change one benchmark suite or artifact group. */
export interface Inputs {
	readonly crates?: readonly string[];
	readonly paths?: readonly string[];
}

interface MetadataPackage {
	name: string;
	manifest_path: string;
	dependencies: { name: string; path?: string }[];
	targets: { name: string; kind: string[] }[];
}

/**
 * The workspace's crates from `cargo metadata --no-deps`, which needs no
 * network and no build.
 */
export function readWorkspace(root: string): WorkspaceCrate[] {
	const output = execFileSync("cargo", [
		"metadata",
		"--no-deps",
		"--format-version",
		"1",
	], { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
	const metadata = JSON.parse(output) as { packages: MetadataPackage[] };

	return metadata.packages.map((pkg) => ({
		name: pkg.name,
		dir: relative(root, pkg.manifest_path.replace(/\/Cargo\.toml$/, "")),
		dependencies: pkg.dependencies
			.filter((dependency) => dependency.path !== undefined)
			.map((dependency) => dependency.name),
		benches: pkg.targets
			.filter((target) => target.kind.includes("bench"))
			.map((target) => target.name),
	}));
}

/** `crates` and every workspace crate they reach through path dependencies. */
export function closure(
	workspace: readonly WorkspaceCrate[],
	crates: readonly string[],
): Set<string> {
	const byName = new Map(workspace.map((crate) => [crate.name, crate]));
	const reached = new Set<string>();
	const pending = [...crates];

	while (pending.length > 0) {
		const name = pending.pop()!;

		if (reached.has(name)) continue;

		reached.add(name);
		pending.push(...(byName.get(name)?.dependencies ?? []));
	}

	return reached;
}

/** Whether `path` is `prefix` or inside it. */
export function within(path: string, prefix: string): boolean {
	const directory = prefix.endsWith("/") ? prefix : `${prefix}/`;

	return path === prefix || path.startsWith(directory);
}

/** The crate whose directory holds `path`, the innermost when nested. */
export function crateOf(
	workspace: readonly WorkspaceCrate[],
	path: string,
): string | undefined {
	const owners = workspace
		.filter((crate) => crate.dir !== "" && within(path, crate.dir))
		.sort((left, right) => right.dir.length - left.dir.length);

	return owners[0]?.name;
}

/** Whether any of `changed` can affect something built from `inputs`. */
export function isAffected(
	workspace: readonly WorkspaceCrate[],
	changed: readonly string[],
	inputs: Inputs,
): boolean {
	const crates = closure(workspace, inputs.crates ?? []);
	const paths = inputs.paths ?? [];

	return changed.some((path) => {
		const owner = crateOf(workspace, path);

		if (owner !== undefined && crates.has(owner)) return true;

		return paths.some((prefix) => within(path, prefix));
	});
}

/** The files that differ between two commits, relative to the root. */
export function changedFiles(
	root: string,
	base: string,
	head: string,
): string[] {
	const output = execFileSync("git", [
		"diff",
		"--name-only",
		"--no-renames",
		base,
		head,
	], {
		cwd: root,
		encoding: "utf8",
	});

	return output.split("\n").filter((line) => line !== "");
}
