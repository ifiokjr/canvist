import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

/** Builds `tests/bundle` as an app would ship it, into `report/bundle`. */
export default defineConfig({
	root: fileURLToPath(new URL(".", import.meta.url)),
	logLevel: "warn",
	build: {
		outDir: fileURLToPath(new URL("../../report/bundle", import.meta.url)),
		emptyOutDir: true,
		target: "es2022",
		// Inlining the module as base64 would hide its real size.
		assetsInlineLimit: 0,
	},
});
