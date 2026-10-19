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
	{ path: "/changelog", title: /Changelog · canvist/ },
] as const;

/** The landing page's beats, in order. */
export const SECTIONS = [
	"top",
	"every-pixel",
	"everywhere",
	"together",
	"free",
	"start",
] as const;

export const WIDTHS = [320, 768, 1280, 1920] as const;

/** Common screens, including short laptops and a landscape phone. */
export const VIEWPORT_MATRIX = [
	{ name: "laptop-720", width: 1280, height: 720 },
	{ name: "laptop-768", width: 1366, height: 768 },
	{ name: "desktop", width: 1440, height: 900 },
	{ name: "full-hd", width: 1920, height: 1080 },
	{ name: "qhd", width: 2560, height: 1440 },
	{ name: "tablet", width: 768, height: 1024 },
	{ name: "phone", width: 390, height: 844 },
	{ name: "small-phone", width: 360, height: 640 },
	{ name: "phone-landscape", width: 844, height: 390 },
] as const;
