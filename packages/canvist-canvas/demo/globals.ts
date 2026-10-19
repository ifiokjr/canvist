/**
 * The demo exposes its canvas as `globalThis.__canvas`, so browser tests
 * can drive and measure it.
 *
 * @module
 */
import type { CanvasHandle } from "../src/mod.ts";

declare global {
	var __canvas: CanvasHandle | undefined;
}
