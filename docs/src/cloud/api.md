# canvist Cloud API

canvist Cloud runs the `canvist_cloud` server. Every route below works the same on a self-hosted instance ([self-hosting](./self-hosting.md)).

## Credentials

| Credential   | Looks like       | Who holds it                         | What it can do                                                    |
| ------------ | ---------------- | ------------------------------------ | ----------------------------------------------------------------- |
| API key      | `ck_…`           | Your servers only                    | Every REST route for one project environment; open any document   |
| Client token | a JWT (`eyJ…`)   | Browsers and apps, for an hour       | Open the documents its `document` claim covers, `read` or `write` |
| Admin token  | anything you set | The operator (`CANVIST_ADMIN_TOKEN`) | Create projects and keys; admin routes are off without it         |

Each project has environments (`production`, `staging`, `development` by default). Keys, documents, webhooks, and usage are per environment.

REST calls send the API key as `Authorization: Bearer ck_…`. Errors are JSON:

```json
{ "error": { "code": "invalid_credentials", "message": "invalid credentials" } }
```

## Sync

### y-websocket

```ts
import { WebsocketProvider } from "y-websocket";

const provider = new WebsocketProvider(
	"wss://SYNC_HOST/v1/sync",
	"team/notes",
	doc,
	{
		params: { token },
	},
);
```

`GET /v1/sync/{document}?token=…` upgrades to a y-websocket session. The token may also be sent as `Authorization: Bearer`. Bad credentials are refused before the upgrade with `401` (missing, malformed, forged, or expired) or `403` (valid, but not for this document); a bad document name is `400`.

### Hocuspocus

```ts
import { HocuspocusProvider } from "@hocuspocus/provider";

const provider = new HocuspocusProvider({
	url: "wss://SYNC_HOST/v1/hocuspocus",
	name: "team/notes",
	document: doc,
	token,
});
```

`GET /v1/hocuspocus` speaks the Hocuspocus protocol, including several documents on one socket. Each document authenticates with its own token; a refused one gets `permissionDenied` with the reason, and the socket stays open for the others. Read-only tokens authenticate with the `readonly` scope.

### What the relay enforces

- **Document names:** 1 to 256 bytes of ASCII letters, digits, and `-_.~:@/`.
- **The canvist schema at the root:** updates may only write the `blocks` root `Y.Array`, and each of its entries must be a shared type (a block `Y.Map`). Updates that write other roots or put bare values in `blocks` are refused. The relay does not check block types or attributes; every canvist client hides blocks that break its schema without deleting them (see the `canvist_collab` docs).
- **Sizes:** 1 MiB per message and 16 MiB per document by default (`CANVIST_MAX_MESSAGE_BYTES`, `CANVIST_MAX_DOCUMENT_BYTES`).
- **Read-only sessions:** updates from `read` tokens are dropped; the client keeps its local changes and stays connected.
- **Presence:** an awareness state is owned by the connection that introduced it, so one peer cannot overwrite another's caret. States are capped at 16 KiB, eight per connection, and withdrawn when the connection closes.
- **Refused updates never disconnect.** The client keeps working; the change simply does not reach others. Malformed frames close the socket with `1007`.

Close codes: `1012` the server is restarting (reconnect), `1013` the peer fell behind (reconnect to resync), `1000` the document was deleted, `1011` the document could not be loaded.

## Tokens

`POST /v1/tokens` (API key) mints a client token.

```json
{
	"subject": "user-42",
	"document": "team/*",
	"scope": "write",
	"ttl_seconds": 3600
}
```

- `subject`: your id for the user, 1 to 256 bytes. It names the editor in webhooks.
- `document`: a document name, or a prefix ending in `*` (`*` alone covers the environment).
- `scope`: `write` (default) or `read`.
- `ttl_seconds`: 1 to 86,400; default 3,600.

Returns `{ "token": "eyJ…", "expires_at": 1792400000 }`. Tokens are HS256 JWTs signed with the server's `CANVIST_TOKEN_SECRET`; mint a new one before `expires_at` and reconnect.

## Documents

| Route                         | Body / response                                                                                      |
| ----------------------------- | ---------------------------------------------------------------------------------------------------- |
| `GET /v1/documents`           | `{ "documents": [{ "name", "pending_updates" }] }`                                                   |
| `GET /v1/documents/{name}`    | The document as one Yjs v1 update (`application/octet-stream`); `404` if it was never written        |
| `POST /v1/documents/{name}`   | A Yjs v1 update to apply, as a peer would; connected peers receive it at once. `{ "changed": bool }` |
| `DELETE /v1/documents/{name}` | Deletes the document and its history and disconnects its peers. `204`                                |

`POST` refuses updates that break the rules above with `422`, and oversized ones with `413`.

## Convert

`POST /v1/convert` (API key) converts a document between `json` (ProseMirror and TipTap JSON), `html`, `markdown`, and `text`, with the same StarterKit schema the editor uses:

```json
{ "from": "markdown", "to": "html", "input": "# Plan\n\n- ship **it**" }
```

returns

```json
{
	"format": "html",
	"output": "<h1>Plan</h1><ul><li><p>ship <strong>it</strong></p></li></ul>"
}
```

`input` is a string, or for `json` an object (a string holding JSON works too); `output` is an object for `json` and a string otherwise. A stored document converts with `GET /v1/documents/{name}?format=markdown` (or `json`, `html`, `text`; the default `yjs` returns the raw state).

Limits: 8 MiB per request (`413` past it), 10 seconds per conversion (`504`), and a bounded number of conversions at once per instance. Unknown formats and wrong input types are `400`; input that does not parse is `422` with code `unconvertible`. Every conversion counts toward the month's usage. A 10,000-paragraph document converts in 15 to 50 ms.

## Usage and plans

A document is **active** in a month (UTC) once a sync session opens it or `POST /v1/documents/{name}` writes it. canvist Cloud bills per monthly active document (MAD), counted once per document per month however often it is used.

`GET /v1/usage?month=YYYY-MM` (API key; the month defaults to the current one) returns the whole project's usage:

```json
{
	"project": "prj_…",
	"month": "2026-10",
	"plan": "hobby",
	"included_documents": 1000,
	"active_documents": 812,
	"standing": "approaching",
	"subscription_status": null,
	"environments": [
		{ "environment": "production", "active_documents": 812, "conversions": 0 }
	]
}
```

`standing` is `ok`, `approaching` (80% of the included documents), `over` (a paid plan past them, billed as overage), or `capped`.

**The free plan has a hard cap that never breaks a client.** When it is full, a session on a document that is not yet active this month is **paused**: the socket stays open and alive, the client keeps editing locally, and nothing syncs. Every document already active this month keeps working. A paused session checks again every 30 seconds and syncs on its own once the project is upgraded or the month turns, sending what was written while it waited. y-websocket clients log a permission warning when they are paused; Hocuspocus clients authenticate normally and receive a `stateless` message `{ "type": "canvist.paused", "reason": "document_limit" }`. A REST write to a new document on a full free plan gets `429` with code `document_limit_reached`.

**Paid plans never pause.** Past the included documents, usage reports `over` and the overage is billed.

`subscription_status` is the Stripe subscription's status, or `null` for a project that never subscribed. `past_due` means a payment failed and Stripe is retrying it: the plan still applies, and the app should ask for a new card.

## Billing

These need an API key and a server with Stripe configured (`404` otherwise). See [billing](./billing.md) for the whole flow.

- `POST /v1/billing/checkout` with `{ "plan": "builder", "success_url": "https://…", "cancel_url": "https://…" }` returns `{ "url" }`, a Stripe Checkout page for a monthly Builder or Scale subscription. The subscription bills on the first of each month; the first partial month's base fee is prorated. A project that already pays gets `409` with code `already_subscribed`.
- `POST /v1/billing/plan` with `{ "plan": "scale" }` switches a paid subscription between Builder and Scale and returns the new terms. The base fee is prorated. Usage before the switch is billed at the old plan's rates, and the new plan's tiers, included documents and all, start from the switch. A project without a paid subscription gets `409` with code `not_subscribed`.
- `POST /v1/billing/portal` with `{ "return_url": "https://…" }` returns `{ "url" }`, a Stripe Customer Portal session for payment methods, invoices, billing details, tax IDs, and cancelling at the end of the period. A project that never subscribed gets `409` with code `no_customer`.
- `POST /v1/stripe/webhook` receives Stripe's subscription, checkout, and invoice events, signed with `STRIPE_WEBHOOK_SECRET`.

## Webhooks

`PUT /v1/webhook` (API key) with `{ "url": "https://…" }` sets the environment's webhook and returns a new signing secret, `{ "url", "secret": "whsec_…" }`. `DELETE /v1/webhook` removes it. URLs must be HTTPS on a public host.

When a document changes, the relay waits until it has been quiet for 2 seconds (or has kept changing for 30), then posts:

```json
{
	"type": "document.updated",
	"id": "evt_…",
	"project": "prj_…",
	"environment": "production",
	"document": "team/notes",
	"editors": ["user-42"],
	"first_change_at": 1792400000000,
	"last_change_at": 1792400004200
}
```

Verify the `Canvist-Signature: t=<unix seconds>,v1=<hex>` header: `v1` is the HMAC-SHA256 of `"<t>.<raw body>"` keyed by the secret. Reject old timestamps to stop replays. Deliveries are retried three times and then dropped; read the document with `GET /v1/documents/{name}` rather than relying on every delivery.

## Admin

With `Authorization: Bearer $CANVIST_ADMIN_TOKEN`:

- `POST /v1/admin/projects` with `{ "name": "acme", "environments": ["production", "staging"] }` creates a project and returns one API key per environment. Keys are shown once.
- `POST /v1/admin/projects/{project}/keys` with `{ "environment": "staging" }` adds a key.
- `PUT /v1/admin/projects/{project}/terms` with `{ "plan": "builder", "included_documents": 20000, "stripe_customer": "cus_…" }` changes a project's plan (`hobby`, `builder`, `scale`, or `enterprise`). `included_documents` overrides the plan's default; `stripe_customer` on a paid plan turns on metered billing. Self-serve projects never need this: Stripe's webhooks set their terms.
- `GET /v1/admin/stats` returns loaded documents and connected peers.

## Health

`GET /healthz` is `200` while the process runs. `GET /readyz` is `200` when the store answers and `503` while shutting down, so a platform stops routing new connections before the server closes the old ones.
