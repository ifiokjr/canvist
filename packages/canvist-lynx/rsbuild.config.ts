// The tests compile JSX as an app would: through the ReactLynx plugin, which
// `@lynx-js/react/testing-library`'s rstest config loads from this file.
import { pluginReactLynx } from "@lynx-js/react-rsbuild-plugin";
import { defineConfig } from "@rsbuild/core";

export default defineConfig({
	plugins: [pluginReactLynx()],
});
