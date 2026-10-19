/**
 * The website's changelog: the `user` release stream that monochange writes
 * to `src/content/changelog/<version>.json`, one file per app release (see
 * `[changelog.outputs.website]` in `monochange.toml`).
 *
 * Only user-facing change types may reach the page. Parsing rejects any entry
 * from another stream or of another type, so a developer note that slipped
 * into these files fails the build instead of being published.
 */

import { existsSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

/** Where monochange writes the release files, relative to the repository root. */
export const CHANGELOG_DIRECTORY = "website/src/content/changelog";

/** The user-stream change types declared in `monochange.toml`. */
export const USER_CHANGE_TYPES = [
	"user_feature",
	"user_fix",
	"user_change",
	"pricing",
] as const;

export type UserChangeType = (typeof USER_CHANGE_TYPES)[number];

export interface ChangelogEntry {
	readonly type: UserChangeType;
	readonly summary: string;
	/** Markdown, without the changeset's "User impact" heading. */
	readonly details: string;
}

export interface ChangelogSection {
	readonly title: string;
	readonly entries: readonly ChangelogEntry[];
}

export interface ChangelogRelease {
	readonly version: string;
	/** `YYYY-MM-DD`, the day the release was prepared. */
	readonly date: string;
	readonly sections: readonly ChangelogSection[];
}

/** `changelog_version_title` on the `app` group renders `<version> (<date>)`. */
const TITLE = /^(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?) \((\d{4}-\d{2}-\d{2})\)$/;

/** monochange renders the changeset's `## User impact` two levels deeper. */
const USER_IMPACT_HEADING = /^#{1,6} User impact\s*\n+/;

export class ChangelogError extends Error {
	override name = "ChangelogError";
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isUserChangeType(value: unknown): value is UserChangeType {
	return USER_CHANGE_TYPES.some((type) => type === value);
}

function field(
	record: Record<string, unknown>,
	key: string,
	where: string,
): string {
	const value = record[key];
	if (typeof value !== "string") {
		throw new ChangelogError(`${where}: \`${key}\` must be a string`);
	}
	return value;
}

function list(
	record: Record<string, unknown>,
	key: string,
	where: string,
): unknown[] {
	const value = record[key];
	if (!Array.isArray(value)) {
		throw new ChangelogError(`${where}: \`${key}\` must be an array`);
	}
	return value;
}

function parseEntry(value: unknown, where: string): ChangelogEntry {
	if (!isRecord(value)) {
		throw new ChangelogError(`${where}: entries must be objects`);
	}
	const stream = field(value, "stream", where);
	if (stream !== "user") {
		throw new ChangelogError(
			`${where}: entry from the \`${stream}\` stream; only the \`user\` stream belongs on the website`,
		);
	}
	const type = value["change_type"];
	if (!isUserChangeType(type)) {
		throw new ChangelogError(
			`${where}: change type \`${
				String(type)
			}\` is not user-facing (expected one of ${USER_CHANGE_TYPES.join(", ")})`,
		);
	}
	const details = typeof value["details_markdown"] === "string"
		? value["details_markdown"]
		: "";
	return {
		type,
		summary: field(value, "summary", where),
		details: details.replace(USER_IMPACT_HEADING, "").trim(),
	};
}

/**
 * Parse one release file. `source` names the file in error messages.
 *
 * @throws {ChangelogError} when the file is not a user-stream release.
 */
export function parseRelease(value: unknown, source: string): ChangelogRelease {
	if (!isRecord(value)) {
		throw new ChangelogError(`${source}: expected a JSON object`);
	}
	const title = field(value, "title", source);
	const [, version, date] = TITLE.exec(title) ?? [];
	if (version === undefined || date === undefined) {
		throw new ChangelogError(
			`${source}: title \`${title}\` must read \`<version> (<YYYY-MM-DD>)\``,
		);
	}
	const sections = list(value, "sections", source).map((section, index) => {
		const where = `${source} section ${index + 1}`;
		if (!isRecord(section)) {
			throw new ChangelogError(`${where}: sections must be objects`);
		}
		return {
			title: field(section, "title", where),
			entries: list(section, "entries", where).map((entry, position) =>
				parseEntry(entry, `${where} entry ${position + 1}`)
			),
		};
	});
	return { version, date, sections };
}

/**
 * Read and validate every release file in `directory`, newest first. A missing
 * directory means nothing has been released yet.
 */
export async function loadReleases(
	directory: string,
): Promise<ChangelogRelease[]> {
	if (!existsSync(directory)) return [];
	const names = (await readdir(directory)).filter((name) =>
		name.endsWith(".json")
	);
	const releases = await Promise.all(
		names.map(async (name) =>
			parseRelease(
				JSON.parse(await readFile(join(directory, name), "utf8")),
				name,
			)
		),
	);
	return releases.sort((left, right) =>
		compareVersionsDescending(left.version, right.version)
	);
}

/** Compare two semver versions, newest first. */
export function compareVersionsDescending(left: string, right: string): number {
	const parts = (version: string) => {
		const [core = "", prerelease] = version.split("-", 2);
		return { numbers: core.split(".").map(Number), prerelease };
	};
	const a = parts(left);
	const b = parts(right);
	for (let index = 0; index < 3; index++) {
		const difference = (b.numbers[index] ?? 0) - (a.numbers[index] ?? 0);
		if (difference !== 0) return difference;
	}
	// A release outranks its prereleases.
	if (a.prerelease === b.prerelease) return 0;
	if (a.prerelease === undefined) return -1;
	if (b.prerelease === undefined) return 1;
	return b.prerelease.localeCompare(a.prerelease, "en", { numeric: true });
}
