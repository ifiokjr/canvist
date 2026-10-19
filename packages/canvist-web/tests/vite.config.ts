import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

/** Serves the browser test pages, compiling the package's TypeScript on the fly. */
export default defineConfig({
	root: fileURLToPath(new URL("./pages", import.meta.url)),
	publicDir: fileURLToPath(new URL("./fonts", import.meta.url)),
	server: {
		port: 5179,
		strictPort: true,
		fs: { allow: [fileURLToPath(new URL("../../..", import.meta.url))] },
	},
	logLevel: "warn",
});
