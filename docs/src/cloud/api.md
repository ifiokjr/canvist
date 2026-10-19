# canvist Cloud API reference

Use this page to look up any canvist Cloud route: credentials, tokens, documents, usage, billing, and webhooks. Every route works the same on a [self-hosted](./self-hosting.md) server.

## Credentials

| Credential   | Looks like     | Keep it                           | What it can do                                                  |
| ------------ | -------------- | --------------------------------- | --------------------------------------------------------------- |
| API key      | `ck_…`         | On your servers only              | Every REST route for one project environment; open any document |
| Client token | a JWT (`eyJ…`) | In browsers and apps, for an hour | Open the documents it covers, to `read` or `write`              |

Each project has environments, `production`, `staging`, and `development` by default. Keys, documents, webhooks, and usage belong to one environment.

Send the API key as `Authorization: Bearer ck_…`. Errors are JSON with a stable code:

```json
{ "error": { "code": "invalid_credentials", "message": "invalid credentials" } }
```

## Sync

- `GET /v1/sync/{document}?token=…` opens a y-websocket session. The token can also go in `Authorization: Bearer`.
- `GET /v1/hocuspocus` opens a Hocuspocus session, with one token per document.

See [Sync](./sync.md) for clients, limits, pausing, and close codes.

## Tokens

`POST /v1/tokens` (API key) mints a client token:

```json
{
	"subject": "user-42",
	"document": "team/*",
	"scope": "write",
	"ttl_seconds": 3600
}
```

| Field         | Meaning                                                                       |
| ------------- | ----------------------------------------------------------------------------- |
| `subject`     | Your id for the user, 1 to 256 bytes. Webhooks name editors by it.            |
| `document`    | A document name, or a prefix ending in `*`. `*` alone covers the environment. |
| `scope`       | `write` (the default) or `read`.                                              |
| `ttl_seconds` | 1 to 86,400. The default is 3,600.                                            |

It returns `{ "token": "eyJ…", "expires_at": 1792400000 }`.

## Documents

| Route                         | What it does                                                                                                                    |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `GET /v1/documents`           | Lists documents: `{ "documents": [{ "name", "pending_updates" }] }`                                                             |
| `GET /v1/documents/{name}`    | The document as one Yjs update. Add `?format=json`, `html`, `markdown`, or `text` to convert it. `404` if it was never written. |
| `POST /v1/documents/{name}`   | Applies a Yjs update, as a peer would. Connected clients receive it at once. Returns `{ "changed": bool }`.                     |
| `DELETE /v1/documents/{name}` | Deletes the document and its history, and disconnects its clients. `204`.                                                       |

`POST` refuses updates that break canvist's document structure with `422`, and oversized ones with `413`. Writing a new document on a full free plan returns `429` with the code `document_limit_reached`.

## Convert

`POST /v1/convert` (API key) converts between `json`, `html`, `markdown`, and `text`. See [Convert](./convert.md).

## Usage

`GET /v1/usage?month=YYYY-MM` (API key; the current month by default) returns the project's usage:

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

| `standing`    | Meaning                                                    |
| ------------- | ---------------------------------------------------------- |
| `ok`          | Under 80% of the included documents                        |
| `approaching` | At 80% or more                                             |
| `over`        | A paid plan past its included documents; billed as overage |
| `capped`      | A free plan at its cap; new documents pause                |

`subscription_status` is the Stripe subscription's status, or `null` if the project never subscribed. `past_due` means a payment failed and Stripe is retrying it: everything keeps working, and your app should ask for a new card.

## Billing

These need an API key. See [Plans and billing](./billing.md).

| Route                       | Body                                                 | Returns                                                                                      |
| --------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `POST /v1/billing/checkout` | `{ "plan": "builder", "success_url", "cancel_url" }` | `{ "url" }`: a Stripe Checkout page. `409 already_subscribed` if the project pays already.   |
| `POST /v1/billing/plan`     | `{ "plan": "scale" }`                                | The new terms. Switches between Builder and Scale. `409 not_subscribed` without a paid plan. |
| `POST /v1/billing/portal`   | `{ "return_url" }`                                   | `{ "url" }`: the Stripe Customer Portal. `409 no_customer` if the project never subscribed.  |

## Webhooks

`PUT /v1/webhook` (API key) with `{ "url": "https://…" }` sets the environment's webhook and returns a new signing secret, `{ "url", "secret": "whsec_…" }`. `DELETE /v1/webhook` removes it. URLs must use HTTPS on a public host.

When a document changes, Sync waits until it's been quiet for 2 seconds, or has kept changing for 30, then posts:

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

Verify the `Canvist-Signature: t=<unix seconds>,v1=<hex>` header: `v1` is the HMAC-SHA256 of `"<t>.<raw body>"`, keyed by the secret. Reject old timestamps to stop replays.

```ts
/** Whether a webhook body is genuine and under five minutes old. */
async function verify(
	body: string,
	header: string,
	secret: string,
): Promise<boolean> {
	const fields = new Map(
		header.split(",").map((pair) => pair.split("=", 2) as [string, string]),
	);
	const sent = Number(fields.get("t"));
	const hex = fields.get("v1") ?? "";
	if (!Number.isFinite(sent) || Math.abs(Date.now() / 1000 - sent) > 300) {
		return false;
	}
	if (!/^([0-9a-f]{2})+$/.test(hex)) return false;

	const key = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(secret),
		{ name: "HMAC", hash: "SHA-256" },
		false,
		["verify"],
	);
	const signature = Uint8Array.from(
		hex.match(/../g)!,
		(byte) => parseInt(byte, 16),
	);
	// `verify` compares in constant time, so the check leaks nothing about the secret.
	return crypto.subtle.verify(
		"HMAC",
		key,
		signature,
		new TextEncoder().encode(`${sent}.${body}`),
	);
}

console.log(await verify("{}", "t=0,v1=00", "whsec_test"));
// false
```

Deliveries are retried three times and then dropped. Treat a webhook as a hint, and read the document with `GET /v1/documents/{name}` rather than relying on every delivery.

## Health

`GET /healthz` is `200` while the process runs. `GET /readyz` is `200` when storage answers, and `503` while the server shuts down.
