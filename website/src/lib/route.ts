/**
 * The public path of a page. Pages build as `name.html` (`build.format:
 * "file"`) but are linked and served without the extension, so
 * `/index.html` is `/` and `/pricing.html` is `/pricing`.
 */
export function routePath(pathname: string): string {
	const path = pathname.replace(/\.html$/, "").replace(/\/index$/, "/");
	return path === "" ? "/" : path;
}
