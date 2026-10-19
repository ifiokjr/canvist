# canvist Cloud Sync

In this guide you connect clients to canvist Cloud Sync, so people can edit documents together without you running a sync server. Sync speaks the standard y-websocket and Hocuspocus protocols, so the providers you already know work unchanged.

## 1. Mint a client token on your server

Your API key (`ck_…`) stays on your server. For each user, mint a short-lived client token that covers only the documents they may open:

```ts no_run
const SYNC_URL = "https://SYNC_HOST";
const API_KEY = Deno.env.get("CANVIST_API_KEY")!;

/** A one-hour token letting `userId` edit every document under `team/`. */
async function clientToken(userId: string): Promise<string> {
	const response = await fetch(`${SYNC_URL}/v1/tokens`, {
		method: "POST",
		headers: {
			authorization: `Bearer ${API_KEY}`,
			"content-type": "application/json",
		},
		body: JSON.stringify({
			subject: userId,
			document: "team/*",
			scope: "write",
			ttl_seconds: 3600,
		}),
	});
	const { token } = await response.json();
	return token;
}
```

- `document` is one document name, or a prefix ending in `*`. `*` alone covers every document in the environment.
- `scope` is `write` or `read`. Updates from read-only sessions are dropped, and the client stays connected.
- Tokens last from 1 second to 24 hours. Mint a new one before it expires, and reconnect.

## 2. Connect with y-websocket or Hocuspocus

```ts no_run
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";

const token = await fetch("/api/canvist-token").then((response) =>
	response.text()
);
const ydoc = new Y.Doc();
const provider = new WebsocketProvider(
	"wss://SYNC_HOST/v1/sync",
	"team/notes",
	ydoc,
	{
		params: { token },
	},
);
```

```ts no_run
import { HocuspocusProvider } from "@hocuspocus/provider";
import * as Y from "yjs";

const token = await fetch("/api/canvist-token").then((response) =>
	response.text()
);
const provider = new HocuspocusProvider({
	url: "wss://SYNC_HOST/v1/hocuspocus",
	name: "team/notes",
	document: new Y.Doc(),
	token,
});
```

Hocuspocus can open several documents on one socket. Each one authenticates with its own token.

## What Sync does for you

- **Persistence.** Documents are stored and come back when anyone reconnects.
- **Presence.** Awareness states (carets, names, colours) are relayed. One person can't overwrite another's state.
- **Schema checks.** Updates that write outside canvist's document structure are refused. A refused update never disconnects anyone: the client keeps editing, and the change just doesn't reach others.
- **Webhooks.** A signed `document.updated` event after a document goes quiet. See [the API reference](./api.md#webhooks).
- **Your server can join in.** Read, write, and convert stored documents over HTTP with your API key. See [Documents](./api.md#documents).

## Limits

| Limit           | Default                                             |
| --------------- | --------------------------------------------------- |
| Document name   | 1 to 256 characters: letters, digits, and `-_.~:@/` |
| Message size    | 1 MiB                                               |
| Document size   | 16 MiB of Yjs state                                 |
| Presence states | 16 KiB each, eight per connection                   |

## When the free plan is full

Clients are never cut off. On a full free plan, a session on a document that isn't active yet this month **pauses**: the socket stays open, the person keeps editing locally, and nothing syncs. Every document already active this month keeps syncing. A paused session checks again every 30 seconds, and catches up on its own once you upgrade or the month turns.

y-websocket clients log a permission warning when they pause. Hocuspocus clients receive a stateless message, `{ "type": "canvist.paused", "reason": "document_limit" }`, which you can use to show a banner.

Paid plans never pause.

## Close codes

| Code   | Meaning                                       |
| ------ | --------------------------------------------- |
| `1000` | The document was deleted                      |
| `1007` | A malformed frame                             |
| `1011` | The document couldn't be loaded               |
| `1012` | The server is restarting. Reconnect           |
| `1013` | The client fell behind. Reconnect to catch up |

Bad credentials are refused before the WebSocket opens: `401` for a missing, malformed, or expired token, `403` for a valid token that doesn't cover the document.
