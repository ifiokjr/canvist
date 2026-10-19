/**
 * The footage `npm run prepare:footage` writes to `public/`, parsed into
 * typed values. Every shape is checked when it loads, so a change in the
 * Rust recorder fails the render loudly instead of drawing nonsense.
 */

import { useEffect, useState } from "react";
import {
	cancelRender,
	continueRender,
	delayRender,
	staticFile,
} from "remotion";

/** `[x, y, width, height]` in page pixels. */
export type Box = readonly [number, number, number, number];

/** One top-level block, painted by `to_svg`: id, SVG index, and box. */
export interface Layer {
	readonly id: string;
	readonly svg: number;
	readonly box: Box;
}

export interface Peer {
	readonly name: string;
	readonly color: string;
	readonly caret: Box;
}

/** One captured state of a document. */
export interface Frame {
	readonly blocks: readonly Layer[];
	readonly selection: {
		readonly svg: number;
		readonly x: number;
		readonly y: number;
	} | null;
	readonly caret: Box | null;
	readonly peers: readonly Peer[];
	/** Microseconds from the input to a finished display list, if timed. */
	readonly micros: number | null;
	/** The input behind this frame, such as `a` or `Enter`. */
	readonly note: string | null;
	readonly extra: Record<string, unknown>;
	readonly height: number;
}

export interface Reel {
	readonly meta: Record<string, unknown>;
	readonly width: number;
	readonly svgs: readonly string[];
	readonly frames: readonly Frame[];
}

export interface Budget {
	readonly metric: string;
	readonly budget: string;
	readonly today: string;
}

export interface Typed {
	readonly snippet: string;
	readonly commands: number;
	readonly extensions: number;
	readonly diagnostics: readonly {
		readonly line: number;
		readonly column: number;
		readonly code: number;
		readonly message: string;
	}[];
}

export interface Manifest {
	readonly target: string;
	readonly profile: string;
}

export interface Brand {
	readonly mark: {
		readonly viewBox: string;
		readonly ink: string;
		readonly paint: string;
	};
	readonly lockup: {
		readonly viewBox: string;
		readonly ink: string;
		readonly paint: string;
	};
}

/** Which page shape the footage was laid out for. */
export type Profile = "wide" | "narrow";

export interface Footage {
	readonly typing: Reel;
	readonly format: Reel;
	readonly drag: Reel;
	readonly collab: Reel;
	readonly devices: readonly Reel[];
	readonly sweep: Reel;
	readonly budgets: readonly Budget[];
	readonly typed: Typed;
	readonly manifest: Manifest;
	readonly brand: Brand;
}

class ShapeError extends Error {
	constructor(path: string, expected: string) {
		super(`footage: ${path} is not ${expected}; rerun npm run prepare:footage`);
	}
}

type Json = unknown;

function record(value: Json, path: string): Record<string, Json> {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		throw new ShapeError(path, "an object");
	}

	return value as Record<string, Json>;
}

function array(value: Json, path: string): readonly Json[] {
	if (!Array.isArray(value)) {
		throw new ShapeError(path, "an array");
	}

	return value;
}

function number(value: Json, path: string): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		throw new ShapeError(path, "a number");
	}

	return value;
}

function string(value: Json, path: string): string {
	if (typeof value !== "string") {
		throw new ShapeError(path, "a string");
	}

	return value;
}

function box(value: Json, path: string): Box {
	const [x, y, width, height] = array(value, path).map((item, index) =>
		number(item, `${path}[${index}]`)
	);

	if (
		x === undefined || y === undefined || width === undefined ||
		height === undefined
	) {
		throw new ShapeError(path, "[x, y, width, height]");
	}

	return [x, y, width, height];
}

function frame(value: Json, path: string): Frame {
	const object = record(value, path);
	const selection = object.selection === null
		? null
		: array(object.selection, `${path}.selection`);

	return {
		blocks: array(object.blocks, `${path}.blocks`).map((layer, index) => {
			const at = `${path}.blocks[${index}]`;
			const [id, svg, ...rest] = array(layer, at);
			return {
				id: string(id, `${at}[0]`),
				svg: number(svg, `${at}[1]`),
				box: box(rest, at),
			};
		}),
		selection: selection === null ? null : {
			svg: number(selection[0], path),
			x: number(selection[1], path),
			y: number(selection[2], path),
		},
		caret: object.caret === null ? null : box(object.caret, `${path}.caret`),
		peers: array(object.peers, `${path}.peers`).map((peer, index) => {
			const at = `${path}.peers[${index}]`;
			const fields = record(peer, at);
			return {
				name: string(fields.name, `${at}.name`),
				color: string(fields.color, `${at}.color`),
				caret: box(fields.caret, `${at}.caret`),
			};
		}),
		micros: object.micros === null
			? null
			: number(object.micros, `${path}.micros`),
		note: object.note === null ? null : string(object.note, `${path}.note`),
		extra: object.extra === null ? {} : record(object.extra, `${path}.extra`),
		height: number(object.height, `${path}.height`),
	};
}

function reel(value: Json, path: string): Reel {
	const object = record(value, path);

	return {
		meta: record(object.meta, `${path}.meta`),
		width: number(object.width, `${path}.width`),
		svgs: array(object.svgs, `${path}.svgs`).map((svg, index) =>
			string(svg, `${path}.svgs[${index}]`)
		),
		frames: array(object.frames, `${path}.frames`).map((item, index) =>
			frame(item, `${path}.frames[${index}]`)
		),
	};
}

function brandArt(value: Json, path: string): Brand["mark"] {
	const object = record(value, path);
	return {
		viewBox: string(object.viewBox, `${path}.viewBox`),
		ink: string(object.ink, `${path}.ink`),
		paint: string(object.paint, `${path}.paint`),
	};
}

async function json(path: string): Promise<Json> {
	const response = await fetch(staticFile(path));

	if (!response.ok) {
		throw new Error(`footage: ${path} is missing; run npm run prepare:footage`);
	}

	return response.json();
}

async function load(profile: Profile): Promise<Footage> {
	const [
		typing,
		format,
		drag,
		collab,
		platforms,
		budgets,
		typed,
		manifest,
		brand,
	] = await Promise.all([
		json(`footage/${profile}/typing.json`),
		json(`footage/${profile}/format.json`),
		json(`footage/${profile}/drag.json`),
		json(`footage/${profile}/collab.json`),
		json("footage/platforms.json"),
		json("footage/budgets.json"),
		json("footage/typed.json"),
		json("footage/manifest.json"),
		json("brand/brand.json"),
	]);
	const platformsObject = record(platforms, "platforms");
	const typedObject = record(typed, "typed");
	const manifestObject = record(manifest, "manifest");
	const brandObject = record(brand, "brand");

	return {
		typing: reel(typing, "typing"),
		format: reel(format, "format"),
		drag: reel(drag, "drag"),
		collab: reel(collab, "collab"),
		devices: array(platformsObject.devices, "platforms.devices").map((
			item,
			index,
		) => reel(item, `devices[${index}]`)),
		sweep: reel(platformsObject.sweep, "platforms.sweep"),
		budgets: array(budgets, "budgets").map((row, index) => {
			const fields = record(row, `budgets[${index}]`);
			return {
				metric: string(fields.metric, "metric"),
				budget: string(fields.budget, "budget"),
				today: string(fields.today, "today"),
			};
		}),
		typed: {
			snippet: string(typedObject.snippet, "typed.snippet"),
			commands: number(typedObject.commands, "typed.commands"),
			extensions: number(typedObject.extensions, "typed.extensions"),
			diagnostics: array(typedObject.diagnostics, "typed.diagnostics").map(
				(item, index) => {
					const fields = record(item, `typed.diagnostics[${index}]`);
					return {
						line: number(fields.line, "line"),
						column: number(fields.column, "column"),
						code: number(fields.code, "code"),
						message: string(fields.message, "message"),
					};
				},
			),
		},
		manifest: {
			target: string(manifestObject.target, "manifest.target"),
			profile: string(manifestObject.profile, "manifest.profile"),
		},
		brand: {
			mark: brandArt(brandObject.mark, "brand.mark"),
			lockup: brandArt(brandObject.lockup, "brand.lockup"),
		},
	};
}

const cache = new Map<Profile, Promise<Footage>>();

/** The footage for `profile`, holding the render until it has loaded. */
export function useFootage(profile: Profile): Footage | null {
	const [footage, setFootage] = useState<Footage | null>(null);
	const [handle] = useState(() => delayRender(`loading ${profile} footage`));

	useEffect(() => {
		let promise = cache.get(profile);

		if (!promise) {
			promise = load(profile);
			cache.set(profile, promise);
		}

		promise.then((loaded) => {
			setFootage(loaded);
			continueRender(handle);
		}).catch((error: unknown) => cancelRender(error));
	}, [profile, handle]);

	return footage;
}

/** A field of a frame's `extra` object, if it is a string. */
export function extraString(
	frame: Frame | undefined,
	key: string,
): string | undefined {
	const value = frame?.extra[key];
	return typeof value === "string" ? value : undefined;
}

/** A field of a frame's `extra` object, if it is a number. */
export function extraNumber(
	frame: Frame | undefined,
	key: string,
): number | undefined {
	const value = frame?.extra[key];
	return typeof value === "number" ? value : undefined;
}
