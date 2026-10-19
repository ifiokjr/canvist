/// <reference types="astro/client" />

interface ImportMetaEnv {
	/** Absolute path of the repository root, with a trailing slash. */
	readonly REPO_ROOT: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
