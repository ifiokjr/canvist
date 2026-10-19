/**
 * End to end: prepare a real release with monochange from a copy of this
 * repository and prove the website's changelog only ever receives the `user`
 * stream. Developer notes for the app and the SDK must land in their own
 * changelogs and never in `website/src/content/changelog/`.
 *
 * Run from the repository root:
 *
 * ```sh
 * deno test --allow-read --allow-write --allow-run=git,monochange tools/release/
 * ```
 */
import { assert, assertEquals } from "jsr:@std/assert@^1";
import { dirname, join } from "jsr:@std/path@^1";
import {
	CHANGELOG_DIRECTORY,
	loadReleases,
} from "../../website/src/lib/changelog.ts";

const REPO_ROOT = new URL("../../", import.meta.url).pathname;

const USER_NOTE = "Pricing loads before you finish scrolling";
const APP_DEVELOPER_NOTE = "Rotate the Railway deployment token";
const SDK_DEVELOPER_NOTE = "Add Transaction::split_block to canvist_model";

async function run(
	command: string,
	args: string[],
	cwd: string,
): Promise<string> {
	const output = await new Deno.Command(command, {
		args,
		cwd,
		stdout: "piped",
		stderr: "piped",
	})
		.output();
	const decoder = new TextDecoder();
	if (!output.success) {
		throw new Error(
			`${command} ${args.join(" ")} failed:\n${decoder.decode(output.stderr)}`,
		);
	}
	return decoder.decode(output.stdout);
}

/** Copy the working tree (tracked and unignored files) into `target`. */
async function copyWorkingTree(target: string): Promise<void> {
	const listing = await run("git", [
		"ls-files",
		"-z",
		"--cached",
		"--others",
		"--exclude-standard",
	], REPO_ROOT);
	for (const path of listing.split("\0").filter(Boolean)) {
		const source = join(REPO_ROOT, path);
		// Deleted but still tracked: nothing to copy.
		if (!(await Deno.stat(source).then((info) => info.isFile, () => false))) {
			continue;
		}
		await Deno.mkdir(join(target, dirname(path)), { recursive: true });
		await Deno.copyFile(source, join(target, path));
	}
}

async function writeChangesets(workspace: string): Promise<void> {
	const changesets = join(workspace, ".changeset");
	for await (const entry of Deno.readDir(changesets)) {
		if (entry.name.endsWith(".md")) {
			await Deno.remove(join(changesets, entry.name));
		}
	}
	await Deno.writeTextFile(
		join(changesets, "user-pricing-speed.md"),
		`---\napp: user_fix\n---\n\n# ${USER_NOTE}\n\n## User impact\n\nThe pricing calculator is ready as soon as the page appears.\n`,
	);
	await Deno.writeTextFile(
		join(changesets, "app-deploy-token.md"),
		`---\napp: fix\n---\n\n# ${APP_DEVELOPER_NOTE}\n\nThe deploy job reads the new project token.\n`,
	);
	await Deno.writeTextFile(
		join(changesets, "sdk-split-block.md"),
		`---\ncanvist_model: feat\n---\n\n# ${SDK_DEVELOPER_NOTE}\n`,
	);
}

async function readAll(directory: string): Promise<string> {
	let text = "";
	for await (const entry of Deno.readDir(directory)) {
		text += await Deno.readTextFile(join(directory, entry.name));
	}
	return text;
}

Deno.test("only user-stream notes reach the website's changelog", async () => {
	const workspace = await Deno.makeTempDir({ prefix: "canvist-release-" });
	try {
		await copyWorkingTree(workspace);
		await writeChangesets(workspace);
		// A fresh repository: monochange reads changeset provenance from git.
		await run("git", ["init", "--quiet"], workspace);
		await run("monochange", ["prepare", "--quiet"], workspace);

		const websiteChangelog = join(workspace, CHANGELOG_DIRECTORY);
		const published = await readAll(websiteChangelog);
		assert(published.includes(USER_NOTE), "the user note is on the website");
		assert(
			!published.includes(APP_DEVELOPER_NOTE),
			"the app's developer note stays off the website",
		);
		assert(
			!published.includes(SDK_DEVELOPER_NOTE),
			"the SDK's developer note stays off the website",
		);

		// The page's loader accepts what monochange wrote.
		const releases = await loadReleases(websiteChangelog);
		const entries = releases.flatMap((release) =>
			release.sections.flatMap((section) => section.entries)
		);
		assertEquals(entries.map((entry) => [entry.type, entry.summary]), [[
			"user_fix",
			USER_NOTE,
		]]);

		// Developer notes are kept, just somewhere the site never reads.
		const appDeveloper = await Deno.readTextFile(
			join(workspace, "website/developer-changelog.md"),
		);
		assert(appDeveloper.includes(APP_DEVELOPER_NOTE));
		assert(!appDeveloper.includes(USER_NOTE));
		const sdk = await Deno.readTextFile(join(workspace, "changelog.md"));
		assert(sdk.includes(SDK_DEVELOPER_NOTE));
	} finally {
		await Deno.remove(workspace, { recursive: true });
	}
});
