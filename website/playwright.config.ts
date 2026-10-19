import { defineConfig, devices } from "@playwright/test";
import process from "node:process";

const PORT = 4600;
const CI = Boolean(process.env["CI"]);

/**
 * End-to-end tests against the production build (`deno task build` first).
 *
 * Visual snapshots are rendered on macOS only: font rasterisation differs
 * between operating systems, so the baselines are platform-specific and CI
 * runs them on a macOS runner.
 */
export default defineConfig({
	testDir: "tests/e2e",
	fullyParallel: true,
	forbidOnly: CI,
	retries: CI ? 1 : 0,
	reporter: CI ? [["github"], ["html", { open: "never" }]] : [["list"]],
	snapshotPathTemplate:
		"{testDir}/__snapshots__/{testFilePath}/{arg}-{projectName}-{platform}{ext}",
	expect: {
		toHaveScreenshot: {
			maxDiffPixelRatio: 0.01,
			animations: "disabled",
			caret: "hide",
		},
	},
	use: {
		baseURL: `http://127.0.0.1:${PORT}`,
		trace: "retain-on-failure",
	},
	projects: [
		{ name: "chromium", use: { ...devices["Desktop Chrome"] } },
	],
	webServer: {
		command: `deno run --allow-net --allow-read tests/serve.ts ${PORT}`,
		url: `http://127.0.0.1:${PORT}/`,
		reuseExistingServer: !CI,
		timeout: 30_000,
	},
});
