import { assertEquals, assertThrows } from "@std/assert";
import {
	CHANGELOG_DIRECTORY,
	ChangelogError,
	compareVersionsDescending,
	loadReleases,
	parseRelease,
	USER_CHANGE_TYPES,
} from "../../src/lib/changelog.ts";

const REPO_ROOT = new URL("../../../", import.meta.url).pathname;

/** The shape of `fixtures/changelog-release.json`. */
interface FixtureRelease {
	title: string;
	sections: { title: string; entries: Record<string, unknown>[] }[];
}

/** A release exactly as `monochange preview` renders the `website` output. */
const fixture = (): FixtureRelease =>
	JSON.parse(
		Deno.readTextFileSync(
			new URL("./fixtures/changelog-release.json", import.meta.url),
		),
	);

/** The fixture with its only entry changed, as monochange would write it. */
function withEntry(changes: Record<string, unknown>): FixtureRelease {
	const release = fixture();
	const entry = release.sections[0]?.entries[0];
	if (entry === undefined) throw new Error("the fixture has no entry");
	Object.assign(entry, changes);
	return release;
}

Deno.test("a monochange release file parses into version, date, and entries", () => {
	const release = parseRelease(fixture(), "0.1.1.json");
	assertEquals(release.version, "0.1.1");
	assertEquals(release.date, "2026-10-19");
	assertEquals(release.sections.map((section) => section.title), ["New"]);
	const [entry] = release.sections[0]?.entries ?? [];
	assertEquals(entry?.type, "user_feature");
	assertEquals(entry?.summary, "The canvist website is live");
	// The changeset's "User impact" heading is the lint's marker, not content.
	assertEquals(
		entry?.details.startsWith("You can now see what canvist is"),
		true,
	);
});

Deno.test("a developer-stream entry never reaches the site", () => {
	const error = assertThrows(
		() =>
			parseRelease(
				withEntry({ stream: "default", change_type: "feat" }),
				"0.1.2.json",
			),
		ChangelogError,
	);
	assertEquals(
		error.message.includes("only the `user` stream belongs on the website"),
		true,
	);
});

Deno.test("a developer change type is rejected even when labelled as the user stream", () => {
	for (const type of ["feat", "fix", "breaking", "docs", "none"]) {
		assertThrows(
			() => parseRelease(withEntry({ change_type: type }), "0.1.2.json"),
			ChangelogError,
			"is not user-facing",
		);
	}
});

Deno.test("a title without a version and date fails the build", () => {
	const release = fixture();
	release.title =
		"app [0.1.1](https://github.com/canvist/canvist/releases/tag/app/v0.1.1)";
	assertThrows(
		() => parseRelease(release, "0.1.1.json"),
		ChangelogError,
		"must read",
	);
});

Deno.test("the site's user types are exactly monochange's user-stream types", () => {
	const config = Deno.readTextFileSync(`${REPO_ROOT}monochange.toml`);
	const declared = [...config.matchAll(/^(\w+) = \{[^}\n]*stream = "user"/gm)]
		.map(([, type]) => type)
		.sort();
	assertEquals(declared, [...USER_CHANGE_TYPES].sort());
});

Deno.test("every committed release file is a valid user-stream release", async () => {
	const releases = await loadReleases(`${REPO_ROOT}${CHANGELOG_DIRECTORY}`);
	for (const release of releases) {
		for (const section of release.sections) {
			for (const entry of section.entries) {
				assertEquals(USER_CHANGE_TYPES.includes(entry.type), true);
			}
		}
	}
});

Deno.test("releases sort newest first, with a release above its prereleases", () => {
	const versions = [
		"0.2.0-rc.1",
		"0.1.10",
		"0.2.0",
		"0.1.9",
		"1.0.0",
		"0.2.0-rc.2",
	];
	assertEquals(versions.sort(compareVersionsDescending), [
		"1.0.0",
		"0.2.0",
		"0.2.0-rc.2",
		"0.2.0-rc.1",
		"0.1.10",
		"0.1.9",
	]);
});
