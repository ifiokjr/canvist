/**
 * Render the React page to HTML on the server. `server.ts` loads this
 * module through Vite on each request.
 */
import { StrictMode } from "react";
import { renderToString } from "react-dom/server";
import { App, type PageProps } from "./App.tsx";

export function render(props: PageProps): string {
	return renderToString(
		<StrictMode>
			<App {...props} />
		</StrictMode>,
	);
}
