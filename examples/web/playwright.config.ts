import { defineConfig, devices } from "@playwright/test";

/** `CANVIST_BROWSERS` (space separated) narrows the run. */
const browsers = (process.env.CANVIST_BROWSERS ?? "chromium firefox webkit")
	.split(/\s+/);
const onContinuousIntegration = process.env.CI !== undefined;

export default defineConfig({
	testDir: "tests",
	fullyParallel: true,
	forbidOnly: onContinuousIntegration,
	reporter: onContinuousIntegration ? [["list"], ["github"]] : "list",
	use: { baseURL: "http://localhost:5180" },
	projects: [
		{ name: "chromium", use: { ...devices["Desktop Chrome"] } },
		{ name: "firefox", use: { ...devices["Desktop Firefox"] } },
		{ name: "webkit", use: { ...devices["Desktop Safari"] } },
	].filter((project) => browsers.includes(project.name)),
	webServer: {
		command: "npx vite",
		url: "http://localhost:5180/",
		reuseExistingServer: !onContinuousIntegration,
	},
});
