/**
 * Loads and checks `speed.json` and `size.json`, so a typo in a threshold or
 * a budget fails loudly instead of disabling a check.
 *
 * @module
 */
import { readFileSync } from "node:fs";

import { type Artifact, METRICS, type SizeConfig } from "./size.ts";
import type { SpeedConfig, Suite } from "./speed.ts";

type Json = Record<string, unknown>;

function isRecord(value: unknown): value is Json {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
	return Array.isArray(value) &&
		value.every((item) => typeof item === "string");
}

function isNumberRecord(value: unknown): value is Record<string, number> {
	return isRecord(value) &&
		Object.values(value).every((item) => typeof item === "number");
}

function ensure(condition: boolean, file: string, message: string): void {
	if (!condition) throw new Error(`${file}: ${message}`);
}

function readJson(url: URL): Json {
	const value: unknown = JSON.parse(readFileSync(url, "utf8"));

	ensure(isRecord(value), url.pathname, "expected an object");

	return value as Json;
}

function isSuite(value: unknown): value is Suite {
	return isRecord(value) && typeof value["package"] === "string" &&
		typeof value["bench"] === "string" &&
		(value["features"] === undefined || isStringArray(value["features"])) &&
		(value["metric"] === undefined || value["metric"] === "instructions");
}

/** `speed.json`, checked. */
export function loadSpeedConfig(url: URL): SpeedConfig {
	const json = readJson(url);
	const file = url.pathname;

	ensure(
		Number.isInteger(json["rounds"]) && Number(json["rounds"]) >= 2,
		file,
		"`rounds` must be an integer of at least 2",
	);
	ensure(
		Number.isInteger(json["confirmRounds"]),
		file,
		"`confirmRounds` must be an integer",
	);
	ensure(
		typeof json["threshold"] === "number",
		file,
		"`threshold` must be a number",
	);
	ensure(
		isNumberRecord(json["thresholds"]),
		file,
		"`thresholds` must map ids to numbers",
	);
	ensure(
		isStringArray(json["globalPaths"]),
		file,
		"`globalPaths` must be strings",
	);
	ensure(
		Array.isArray(json["suites"]) && json["suites"].every(isSuite),
		file,
		"every suite needs a `package` and a `bench`",
	);

	return json as unknown as SpeedConfig;
}

function isSizes(value: unknown): boolean {
	return isNumberRecord(value) &&
		Object.keys(value).every((key) =>
			(METRICS as readonly string[]).includes(key)
		);
}

function isArtifact(value: unknown): value is Artifact {
	return isRecord(value) && typeof value["name"] === "string" &&
		typeof value["group"] === "string" && isStringArray(value["files"]) &&
		(value["budget"] === undefined || isSizes(value["budget"])) &&
		(value["metrics"] === undefined || isStringArray(value["metrics"]));
}

/** `size.json`, checked, including that every artifact's group exists. */
export function loadSizeConfig(url: URL): SizeConfig {
	const json = readJson(url);
	const file = url.pathname;
	const groups = isRecord(json["groups"]) ? json["groups"] : {};
	const artifacts = Array.isArray(json["artifacts"]) ? json["artifacts"] : [];

	ensure(
		typeof json["growthPercent"] === "number",
		file,
		"`growthPercent` must be a number",
	);
	ensure(
		typeof json["growthBytes"] === "number",
		file,
		"`growthBytes` must be a number",
	);
	ensure(
		typeof json["approvalLabel"] === "string",
		file,
		"`approvalLabel` must be a string",
	);
	ensure(
		isStringArray(json["globalPaths"]),
		file,
		"`globalPaths` must be strings",
	);
	ensure(
		artifacts.length > 0 && artifacts.every(isArtifact),
		file,
		"every artifact needs a `name`, a `group`, and `files`",
	);

	for (const artifact of artifacts as Artifact[]) {
		ensure(
			artifact.group in groups,
			file,
			`${artifact.name}: no group \`${artifact.group}\``,
		);
	}

	for (const [name, group] of Object.entries(groups)) {
		const needs = isRecord(group) && isStringArray(group["needs"])
			? group["needs"]
			: [];
		const unknown = needs.filter((need) => !(need in groups));

		ensure(
			unknown.length === 0,
			file,
			`${name} needs unknown groups: ${unknown.join(", ")}`,
		);
	}

	return json as unknown as SizeConfig;
}
