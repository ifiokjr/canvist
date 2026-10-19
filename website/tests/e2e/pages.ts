/** Every route the site builds, for tests that sweep the whole site. */
export const PAGES = [
	{ path: "/", title: /canvist: the editor that owns every pixel/ },
	{ path: "/pricing", title: /Pricing · canvist/ },
	{ path: "/services", title: /Services · canvist/ },
	{ path: "/services/sync", title: /canvist Sync · canvist/ },
	{ path: "/services/convert", title: /canvist Convert · canvist/ },
	{ path: "/services/history", title: /canvist History · canvist/ },
	{ path: "/docs", title: /Introduction · canvist/ },
	{ path: "/docs/concepts/how-it-works", title: /How canvist works · canvist/ },
	{ path: "/docs/crates/canvist_core", title: /canvist_core · canvist/ },
] as const;

/** The landing page's sections, in deck order. */
export const SECTIONS = [
	"top",
	"every-pixel",
	"everywhere",
	"together",
	"typed",
	"speed",
	"free",
	"services",
	"start",
] as const;

export const WIDTHS = [320, 768, 1280, 1920] as const;
