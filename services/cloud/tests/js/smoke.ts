/**
 * A smoke test for a deployed canvist Cloud.
 *
 *   CANVIST_ADMIN_TOKEN=… deno run --allow-net --allow-env smoke.ts \
 *     https://sync.example.com https://convert.example.com
 *
 * On Railway, `railway run --service sync --environment staging deno run …`
 * supplies the admin token without printing it. The test creates a
 * throwaway project, syncs two stock y-websocket clients and a Hocuspocus
 * client through the relay, reads the document back over REST, converts
 * it, and checks usage. It prints one line per check and exits non-zero on
 * the first failure.
 */

import { HocuspocusProvider } from "npm:@hocuspocus/provider@4.7.0";
import { WebsocketProvider } from "npm:y-websocket@3.0.0";
import * as Y from "npm:yjs@13.6.33";

const [syncBase, convertBase = syncBase] = Deno.args;
const admin = Deno.env.get("CANVIST_ADMIN_TOKEN");

if (!syncBase || !admin) {
	console.error(
		"usage: CANVIST_ADMIN_TOKEN=… smoke.ts <sync url> [convert url]",
	);
	Deno.exit(2);
}

const wsBase = syncBase.replace(/^http/, "ws");

function check(name: string, ok: boolean, detail = "") {
	console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? `: ${detail}` : ""}`);
	if (!ok) Deno.exit(1);
}

function until(test: () => boolean, timeoutMs = 15000): Promise<boolean> {
	return new Promise((resolve) => {
		const started = Date.now();
		const poll = () => {
			if (test()) return resolve(true);
			if (Date.now() - started > timeoutMs) return resolve(false);
			setTimeout(poll, 50);
		};
		poll();
	});
}

async function json(response: Response): Promise<Record<string, unknown>> {
	const body = await response.text();
	try {
		return JSON.parse(body);
	} catch {
		return { body };
	}
}

function texts(doc: Y.Doc): string[] {
	return doc
		.getArray<Y.Map<unknown>>("blocks")
		.toArray()
		.map((block) => String(block.get("text") ?? ""));
}

function append(doc: Y.Doc, text: string) {
	doc.transact(() => {
		const block = new Y.Map<unknown>();
		// Block ids are `client.seq` in base 36, as canvist writes them.
		const client = (doc.clientID % 1_000_000).toString(36);
		block.set("id", `${client}.${(texts(doc).length + 1).toString(36)}`);
		block.set("type", "paragraph");
		block.set("attrs", new Y.Map());
		block.set("text", new Y.Text(text));
		doc.getArray("blocks").push([block]);
	});
}

for (const base of new Set([syncBase, convertBase])) {
	const ready = await fetch(`${base}/readyz`);
	check(`${base}/readyz`, ready.ok, `${ready.status}`);
	await ready.body?.cancel();
}

const created = await json(
	await fetch(`${syncBase}/v1/admin/projects`, {
		method: "POST",
		headers: {
			authorization: `Bearer ${admin}`,
			"content-type": "application/json",
		},
		body: JSON.stringify({
			name: `smoke-${Date.now()}`,
			environments: ["production"],
		}),
	}),
);
const key = (created.keys as Record<string, string> | undefined)?.production;
check(
	"create a project",
	Boolean(key),
	String(created.project ?? JSON.stringify(created)),
);
const auth = {
	authorization: `Bearer ${key}`,
	"content-type": "application/json",
};

const minted = await json(
	await fetch(`${syncBase}/v1/tokens`, {
		method: "POST",
		headers: auth,
		body: JSON.stringify({
			subject: "smoke",
			document: "smoke/*",
			ttl_seconds: 600,
		}),
	}),
);
const token = String(minted.token ?? "");
check("mint a client token", token.length > 0);

const refused = await fetch(`${syncBase}/v1/documents`, {
	headers: { authorization: `Bearer ${token}` },
});
check(
	"client tokens cannot use the REST API",
	refused.status === 403,
	`${refused.status}`,
);
await refused.body?.cancel();

const alice = new Y.Doc();
const bob = new Y.Doc();
const providers = [alice, bob].map(
	(doc) =>
		new WebsocketProvider(`${wsBase}/v1/sync`, "smoke/notes", doc, {
			params: { token },
		}),
);
check(
	"two y-websocket clients sync",
	await until(() => providers.every((provider) => provider.synced)),
);

const started = performance.now();
append(alice, "from alice");
check(
	"an edit reaches the other client",
	await until(() => texts(bob).includes("from alice")),
);
console.log(`     round trip ${Math.round(performance.now() - started)} ms`);

const carol = new Y.Doc();
const hocuspocus = new HocuspocusProvider({
	url: `${wsBase}/v1/hocuspocus`,
	name: "smoke/notes",
	document: carol,
	token,
});
check("a Hocuspocus client syncs", await until(() => hocuspocus.synced));
append(carol, "from carol");
check(
	"all three converge",
	await until(() =>
		[alice, bob, carol].every((doc) => texts(doc).length === 2)
	),
);

const foreign = new Y.Doc();
foreign.getText("prosemirror").insert(0, "not canvist");
const rejected = await fetch(`${syncBase}/v1/documents/smoke/notes`, {
	method: "POST",
	headers: { authorization: `Bearer ${key}` },
	body: Y.encodeStateAsUpdate(foreign),
});
check(
	"updates outside the schema are refused",
	rejected.status === 422,
	`${rejected.status}`,
);
await rejected.body?.cancel();

await new Promise((resolve) => setTimeout(resolve, 500));
const exported = await json(
	await fetch(`${syncBase}/v1/documents/smoke/notes?format=markdown`, {
		headers: auth,
	}),
);
check(
	"the synced document exports as Markdown",
	exported.output === "from alice\n\nfrom carol" ||
		exported.output === "from carol\n\nfrom alice",
	JSON.stringify(exported.output),
);

const converted = await json(
	await fetch(`${convertBase}/v1/convert`, {
		method: "POST",
		headers: auth,
		body: JSON.stringify({
			from: "markdown",
			to: "html",
			input: "# Smoke\n\n**works**",
		}),
	}),
);
check(
	"convert Markdown to HTML",
	converted.output === "<h1>Smoke</h1><p><strong>works</strong></p>",
	JSON.stringify(converted.output),
);

const usage = await json(
	await fetch(`${syncBase}/v1/usage`, { headers: auth }),
);
const environments = usage.environments as
	| { conversions: number }[]
	| undefined;
check(
	"usage counts the document and the conversions",
	usage.active_documents === 1 && environments?.[0]?.conversions === 2,
	JSON.stringify(usage),
);

for (const provider of providers) provider.destroy();
hocuspocus.destroy();
console.log("smoke test passed");
Deno.exit(0);
