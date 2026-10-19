import react from "@vitejs/plugin-react";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

/** One page per example. */
export default defineConfig({
	plugins: [react(), vue()],
	server: {
		port: 5180,
		strictPort: true,
		// The packages are linked from this repository's `packages/`; an app
		// that installs them from npm doesn't need this.
		fs: { allow: [fileURLToPath(new URL("../..", import.meta.url))] },
	},
	// canvist finds its WebAssembly next to its own modules; pre-bundling
	// would move the modules away from it.
	optimizeDeps: {
		exclude: [
			"@canvist/web",
			"@canvist/headless",
			"@canvist/react",
			"@canvist/vue",
		],
	},
	build: {
		rollupOptions: {
			input: [
				"index.html",
				"vanilla.html",
				"react.html",
				"vue.html",
				"ssr.html",
			],
		},
	},
});
