const path = require("node:path");
const { getDefaultConfig, mergeConfig } = require("@react-native/metro-config");

// `@canvist/react-native` and `@canvist/headless` are linked from
// `packages/`. Metro watches them, and resolves every import from this app's
// `node_modules`, so the packages' own development copies of React and React
// Native never load beside the app's.
const packages = path.resolve(__dirname, "../../packages");

/** @type {import("@react-native/metro-config").MetroConfig} */
const config = {
	watchFolders: [
		path.join(packages, "canvist-react-native"),
		path.join(packages, "canvist-headless"),
	],
	resolver: {
		nodeModulesPaths: [path.join(__dirname, "node_modules")],
		disableHierarchicalLookup: true,
	},
};

module.exports = mergeConfig(getDefaultConfig(__dirname), config);
