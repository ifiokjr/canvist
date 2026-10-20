import react from "@vitejs/plugin-react";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

/**
 * One page per example. `server.ts` runs this config in middleware mode and
 * renders the server-rendered pages itself, so a static build leaves them
 * out.
 */
export default defineConfig({
	plugins: [react(), vue()],
	server: {
		port: 5180,
		strictPort: true,
		// The packages are linked from this repository's `packages/`; an app
		// that installs them from npm doesn't need this.
		fs: { allow: [fileURLToPath(new URL("../..", import.meta.url))] },
	},
	// The linked packages have their own copies of React and Vue for their
	// builds; the app must use one.
	resolve: { dedupe: ["react", "react-dom", "vue"] },
	// Server rendering loads the linked bindings through Vite, so they share
	// that one React too.
	ssr: { noExternal: ["@canvist/react"] },
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
			],
		},
	},
});
