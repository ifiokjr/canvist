import { defineConfig, devices } from "@playwright/test";

/**
 * Browser tests: pixel parity between the canvas and server-rendered HTML,
 * and the editor's behaviour, in Chromium, Firefox, and WebKit.
 * `CANVIST_BROWSERS` (space separated) narrows the run.
 */
const browsers = (process.env.CANVIST_BROWSERS ?? "chromium firefox webkit")
	.split(/\s+/);
const all = [
	{ name: "chromium", use: { ...devices["Desktop Chrome"] } },
	{ name: "firefox", use: { ...devices["Desktop Firefox"] } },
	{ name: "webkit", use: { ...devices["Desktop Safari"] } },
];

/** Whether this run is on a CI server, which sets `CI` in the environment. */
const onContinuousIntegration = process.env.CI !== undefined;

/** The port y-websocket's server listens on. */
const COLLAB_PORT = 5181;

/** How long one test may take, in milliseconds: font loading and three-way pixel comparisons are slow. */
const TEST_TIMEOUT_MS = 60_000;

export default defineConfig({
	testDir: "tests/browser",
	timeout: TEST_TIMEOUT_MS,
	fullyParallel: true,
	forbidOnly: onContinuousIntegration,
	reporter: onContinuousIntegration ? [["list"], ["github"]] : "list",
	use: { baseURL: "http://localhost:5179" },
	projects: all.filter((project) => browsers.includes(project.name)),
	webServer: [
		{
			command: "npx vite --config tests/vite.config.ts",
			url: "http://localhost:5179/parity.html",
			reuseExistingServer: !onContinuousIntegration,
		},
		{
			// y-websocket's own server, for the collaboration tests.
			command: "node node_modules/y-websocket/bin/server.cjs",
			env: { HOST: "127.0.0.1", PORT: String(COLLAB_PORT) },
			port: COLLAB_PORT,
			reuseExistingServer: !onContinuousIntegration,
		},
	],
});
