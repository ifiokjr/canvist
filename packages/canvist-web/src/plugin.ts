/**
 * Plugins: optional features with their own typed API on the editor.
 *
 * A plugin is a name and a `setup` function. Whatever `setup` returns
 * becomes `editor.plugins[name]`, typed from the plugin itself, so using a
 * plugin's API without registering it is a type error, and code for an
 * unregistered plugin is never loaded.
 *
 * @module
 */
import type { CanvasMeasurer } from "./measure.ts";

/** What the editor offers a plugin while it sets up. */
export interface PluginContext {
	/** The element the editor renders into. */
	readonly element: HTMLElement;
	/** The measurer, to hear which fonts and text each relayout measured. */
	readonly measurer: CanvasMeasurer;
	/** Aborted when the editor is destroyed; release listeners and timers with it. */
	readonly signal: AbortSignal;
	/** Faces of `families` finished loading: re-measure only what uses them. */
	fontsLoaded(families: readonly string[]): void;
	/** Lay out text in `families` without drawing it, as during a `font-display` block period. */
	hideFamilies(families: readonly string[]): void;
	/** Paint again on the next frame. */
	invalidate(): void;
	/** Delay the editor's `ready()` until `work` settles. */
	waitFor(work: Promise<unknown>): void;
}

/** A plugin named `Name` whose API is `Api`. */
export interface Plugin<Name extends string = string, Api = unknown> {
	readonly name: Name;
	setup(context: PluginContext): Api;
}

/** A list of plugins. */
export type Plugins = readonly Plugin[];

/** Each registered plugin's API, by name. */
export type PluginApis<P extends Plugins> = {
	readonly [Entry in P[number] as Entry["name"]]: ReturnType<Entry["setup"]>;
};
