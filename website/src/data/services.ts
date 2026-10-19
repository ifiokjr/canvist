/**
 * canvist's hosted services, from docs/src/v2/business.md. The software for
 * each is free and self-hostable; the paid product is running it.
 */
export type ServiceId = "sync" | "convert" | "history";

export interface Service {
	id: ServiceId;
	name: string;
	tagline: string;
	summary: string;
	status: "building" | "next";
	statusText: string;
	/** What the hosted service does, as short concrete lines. */
	does: string[];
	/** What you would run yourself instead. */
	selfHost: string;
	/** A short example of using it. */
	example: { label: string; code: string };
}

export const services: Service[] = [
	{
		id: "sync",
		name: "Sync",
		tagline: "Real-time collaboration without running a server.",
		summary:
			"A managed yrs relay with persistence, auth, presence and webhooks. It speaks the y-websocket and Hocuspocus protocols, so any Yjs client can join.",
		status: "building",
		statusText: "Building",
		does: [
			"Relays Yjs updates between every peer in a document",
			"Persists each document between sessions",
			"Carries presence: carets, selections and who is here",
			"Authenticates connections and calls your webhooks on change",
		],
		selfHost:
			"Run the exact relay binary canvist runs, on your own infrastructure, with the same protocols.",
		example: {
			label: "Connect an editor (provisional API)",
			code: `const sync = await connectSync({
  url: "wss://sync.canvist.example/v1",
  document: "notes/launch-plan",
  token: await getToken(),
});

editor.collaborate(sync);`,
		},
	},
	{
		id: "convert",
		name: "Convert",
		tagline: "Every document format, one HTTP call away.",
		summary:
			"A REST API that converts between canvist JSON, Markdown, HTML and plain text. DOCX and PDF come later.",
		status: "building",
		statusText: "Building",
		does: [
			"Converts JSON, Markdown, HTML and plain text in any direction",
			"DOCX and PDF are planned",
		],
		selfHost: "Run the exact conversion binary canvist runs, next to your app.",
		example: {
			label: "Markdown to canvist JSON (provisional API)",
			code: `curl https://convert.canvist.example/v1/convert \\
  -H "Authorization: Bearer $CANVIST_KEY" \\
  -H "Content-Type: text/markdown" \\
  -H "Accept: application/vnd.canvist+json" \\
  --data-binary @launch-plan.md`,
		},
	},
	{
		id: "history",
		name: "History",
		tagline: "Every version of every document, kept.",
		summary:
			"Snapshots, named versions, diffs and restore for documents on Sync. Scrub back through a document's life and bring any moment back.",
		status: "next",
		statusText: "Next",
		does: [
			"Takes snapshots of each document",
			"Names the versions you want to keep",
			"Diffs any two versions",
			"Restores any version",
		],
		selfHost:
			"Run the exact binary canvist runs and keep every version on your own storage.",
		example: {
			label: "Name and restore a version (provisional API)",
			code: `const v = await history.name("notes/launch-plan", "Sent to legal");

// Later:
await history.restore("notes/launch-plan", v.id);`,
		},
	},
];

export function serviceById(id: ServiceId): Service {
	const service = services.find((entry) => entry.id === id);
	if (!service) throw new Error(`Unknown service "${id}"`);
	return service;
}
