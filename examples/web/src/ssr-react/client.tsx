/**
 * Hydrate the server-rendered React page with the props the server
 * rendered it from.
 */
import { StrictMode } from "react";
import { hydrateRoot } from "react-dom/client";
import { App, type PageProps } from "./App.tsx";

const props = JSON.parse(
	document.querySelector("#props")?.textContent ?? "{}",
) as PageProps;

hydrateRoot(
	document.querySelector("#app")!,
	<StrictMode>
		<App {...props} />
	</StrictMode>,
);
