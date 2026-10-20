/**
 * canvist canvas scenes on a server: load scene JSON or Yjs state, run
 * canvas commands and inspector edits, and export SVG and PNG with the
 * renderer the canvas draws with.
 *
 * Scenes live in their own WebAssembly module, so the document entry point
 * stays small.
 *
 * ```ts
 * import { createSceneEngine } from "@canvist/headless/scene";
 *
 * const engine = await createSceneEngine();
 * using doc = engine.fromJson({
 * 	type: "canvist-scene",
 * 	version: 1,
 * 	nodes: [{
 * 		type: "page",
 * 		children: [{
 * 			type: "rectangle",
 * 			props: { size: [120, 80], fills: [{ type: "solid", color: "#3b2cf5" }] },
 * 		}],
 * 	}],
 * });
 * const png = doc.exportPng({ scale: 2 });
 * ```
 *
 * @module
 */
import init, {
	WasmSceneDoc,
	WasmSceneEngine,
} from "../wasm-scene/canvist_headless_scene.js";
import { bundled, type WasmSource } from "./load.ts";
import type { ExportOptions, NodeId, SceneJson } from "./scene-types.ts";

export { type CanvistError, isCanvistError } from "./engine.ts";
export type { WasmSource } from "./load.ts";
export type * from "./scene-types.ts";

const BUNDLED = new URL(
	"../wasm-scene/canvist_headless_scene_bg.wasm",
	import.meta.url,
);

let loading: Promise<void> | undefined;

/**
 * Load the scene WebAssembly module once. Later calls return the first
 * call's promise. Without a source, Node, Deno, and Bun read the bundled
 * file; runtimes without a file system pass the module they imported.
 */
export function loadScene(source?: WasmSource): Promise<void> {
	loading ??= (async () => {
		await init({ module_or_path: source ?? await bundled(BUNDLED) });
	})();
	return loading;
}

/** How to create a scene engine. */
export interface SceneEngineOptions {
	/** Allocate node ids, and act as Yjs client, as this id. Defaults to 1. */
	readonly client?: number;
	/** Where the scene module comes from; see {@link loadScene}. */
	readonly wasm?: WasmSource;
}

/** Load the scene module and create an engine with every built-in canvas extension. */
export async function createSceneEngine(
	options: SceneEngineOptions = {},
): Promise<SceneEngine> {
	await loadScene(options.wasm);
	return new SceneEngine(options.client ?? 1);
}

/** A set of canvas extensions that creates scenes. Create one with {@link createSceneEngine}. */
export class SceneEngine {
	readonly #wasm: WasmSceneEngine;
	readonly #client: number;

	/** @internal Use {@link createSceneEngine}, which loads the module first. */
	constructor(client: number) {
		this.#client = client;
		this.#wasm = new WasmSceneEngine(client);
	}

	/** Tools, commands, node types, and inspector sections, as Rust reports them. */
	metadata(): unknown {
		return JSON.parse(this.#wasm.metadata());
	}

	/** A scene with one empty page. */
	create(): SceneDoc {
		return new SceneDoc(this.#wasm.create(), this.#client);
	}

	/** A scene from scene JSON, nested or flat. */
	fromJson(json: SceneJson): SceneDoc {
		return new SceneDoc(
			this.#wasm.fromJson(JSON.stringify(json)),
			this.#client,
		);
	}

	/** A scene joined from a peer's v1 Yjs state. It is shared, so it produces updates. */
	fromYjs(state: Uint8Array): SceneDoc {
		return new SceneDoc(this.#wasm.fromYjs(state), this.#client);
	}

	/** Release the engine's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}

/** One scene. Every edit is one undo step. */
export class SceneDoc {
	readonly #wasm: WasmSceneDoc;
	readonly #client: number;

	/** @internal Scenes come from a {@link SceneEngine}. */
	constructor(wasm: WasmSceneDoc, client: number) {
		this.#wasm = wasm;
		this.#client = client;
	}

	/** The scene in the canonical flat form. */
	toJson(): SceneJson {
		return JSON.parse(this.#wasm.toJson());
	}

	/** Run canvas command `name` (see {@link SceneEngine.metadata}) against the selection. */
	run(name: string, args: Readonly<Record<string, unknown>> = {}): void {
		this.#wasm.run(name, JSON.stringify(args));
	}

	/** Select nodes by id, for commands and inspector fields. */
	select(ids: readonly NodeId[]): void {
		this.#wasm.select([...ids]);
	}

	/** The selected nodes' ids. */
	selection(): NodeId[] {
		return this.#wasm.selection();
	}

	/** The inspector for the selection: sections of fields and their values. */
	inspect(): unknown {
		return JSON.parse(this.#wasm.inspect());
	}

	/** Set inspector field `key` on every selected node. */
	setField(key: string, value: unknown): void {
		this.#wasm.setField(key, JSON.stringify(value));
	}

	/** Undo the latest change; returns whether there was one. */
	undo(): boolean {
		return this.#wasm.undo();
	}

	/**
	 * Provide an image asset's pixels, `width` by `height` RGBA8. Exports
	 * draw an image without pixels as a grey placeholder; SVG exports
	 * reference provided images by asset id.
	 */
	addImage(
		asset: string,
		width: number,
		height: number,
		rgba: Uint8Array,
	): void {
		this.#wasm.addImage(asset, width, height, rgba);
	}

	/** The scene, or the nodes `options` names, as an SVG document. */
	exportSvg(options: ExportOptions = {}): string {
		return this.#wasm.exportSvg(JSON.stringify(options));
	}

	/** The scene, or the nodes `options` names, as a PNG image. */
	exportPng(options: ExportOptions = {}): Uint8Array {
		return this.#wasm.exportPng(JSON.stringify(options));
	}

	/** Share the scene as Yjs client `clientId` (the engine's client by default). */
	share(clientId: number = this.#client): void {
		this.#wasm.share(clientId);
	}

	/** The whole shared scene as one v1 Yjs update. */
	encodeState(): Uint8Array {
		return this.#wasm.encodeState();
	}

	/** Merge a peer's v1 update; returns whether the scene changed. */
	applyUpdate(update: Uint8Array): boolean {
		return this.#wasm.applyUpdate(update);
	}

	/** The v1 updates this scene produced since the last call. */
	takeUpdates(): Uint8Array[] {
		return this.#wasm.takeUpdates();
	}

	/** Release the scene's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}
