# Self-hosting canvist Cloud

canvist Cloud is one binary, `canvist-cloud`, from `services/cloud`. Self-hosting runs exactly what canvist Cloud runs.

## Run the image

The image canvist Cloud deploys builds from the repository root:

```sh
docker build -f services/cloud/Dockerfile -t canvist-cloud .
docker run -p 8080:8080 \
	-e CANVIST_TOKEN_SECRET="$(openssl rand -hex 32)" \
	-e CANVIST_ADMIN_TOKEN="$(openssl rand -hex 32)" \
	-e DATABASE_URL=postgres://user:password@db.internal:5432/canvist \
	canvist-cloud
```

It is a Debian slim base with the one binary, running as an unprivileged user, logging JSON. [Railway](./railway.md) runs exactly this image.

## Run from source

```sh
CANVIST_TOKEN_SECRET=$(openssl rand -hex 32) \
CANVIST_ADMIN_TOKEN=$(openssl rand -hex 32) \
DATABASE_URL=postgres://user:password@localhost:5432/canvist \
cargo run --release -p canvist_cloud
```

Without `DATABASE_URL` documents live in memory and vanish on restart, which is only useful for trying it out. Migrations run on startup; several instances starting together take turns.

Create a project and its keys:

```sh
curl -X POST http://localhost:8080/v1/admin/projects \
	-H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" \
	-H "Content-Type: application/json" \
	-d '{ "name": "my-app" }'
```

## Configuration

| Variable                        | Default                   | Meaning                                                                                        |
| ------------------------------- | ------------------------- | ---------------------------------------------------------------------------------------------- |
| `CANVIST_TOKEN_SECRET`          | required                  | HMAC key for client tokens, at least 32 bytes. Rotating it signs everyone out.                 |
| `DATABASE_URL`                  | memory                    | Postgres connection string. Connections are unencrypted: use a private network or a TLS proxy. |
| `CANVIST_ADMIN_TOKEN`           | off                       | Bearer token for `/v1/admin`.                                                                  |
| `HOST`, `PORT`                  | `::`, `8080`              | Where to listen. `::` accepts IPv4 and IPv6.                                                   |
| `CANVIST_ROLE`                  | `all`                     | `all`, `sync`, or `convert`: which route groups this instance serves.                          |
| `CANVIST_IDLE_EVICTION_SECONDS` | `60`                      | How long a document with no connections stays in memory.                                       |
| `CANVIST_FLUSH_INTERVAL_MS`     | `250`                     | How often changes are written to Postgres.                                                     |
| `CANVIST_COMPACT_AFTER_UPDATES` | `200`                     | Update rows after which a document is folded into a snapshot.                                  |
| `CANVIST_MAX_MESSAGE_BYTES`     | `1048576`                 | The largest WebSocket message and REST body.                                                   |
| `CANVIST_MAX_DOCUMENT_BYTES`    | `16777216`                | The largest document, as encoded Yjs state.                                                    |
| `CANVIST_WEBHOOK_DEBOUNCE_MS`   | `2000`                    | Quiet time before a `document.updated` webhook.                                                |
| `CANVIST_WEBHOOK_MAX_WAIT_MS`   | `30000`                   | The longest a webhook waits while changes keep coming.                                         |
| `CANVIST_WEBHOOK_ALLOW_PRIVATE` | `false`                   | Allow HTTP and private hosts as webhook targets, inside your network.                          |
| `CANVIST_CONVERT_MAX_BYTES`     | `8388608`                 | The largest conversion request.                                                                |
| `CANVIST_CONVERT_TIMEOUT_MS`    | `10000`                   | The longest a conversion may run before the request gives up.                                  |
| `CANVIST_CONVERT_CONCURRENCY`   | twice the CPUs            | Conversions running at once.                                                                   |
| `CANVIST_PAUSE_RECHECK_SECONDS` | `30`                      | How often a session paused by a full free plan checks again.                                   |
| `STRIPE_SECRET_KEY`             | off                       | A Stripe **test-mode** key (`sk_test_` or `rk_test_`); live keys are refused.                  |
| `STRIPE_WEBHOOK_SECRET`         | off                       | The signing secret of the Stripe webhook endpoint; turns on `POST /v1/stripe/webhook`.         |
| `STRIPE_METER_EVENT`            | `canvist_active_document` | The Stripe Billing Meter event name.                                                           |
| `CANVIST_LOG_FORMAT`            | text                      | `json` for structured logs.                                                                    |
| `RUST_LOG`                      | `info`                    | Log filter, such as `canvist_cloud=debug`.                                                     |

## Metering and Stripe

Every project starts on the free `hobby` plan, capped at 1,000 monthly active documents. Self-hosters who want no caps set every project to `enterprise` with no included documents:

```sh
curl -X PUT http://localhost:8080/v1/admin/projects/$PROJECT/terms \
	-H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" -H "Content-Type: application/json" \
	-d '{ "plan": "enterprise" }'
```

With `STRIPE_SECRET_KEY` set, each document that becomes active in a month on a project with a `stripe_customer` sends one [Billing Meter](https://docs.stripe.com/billing/subscriptions/usage-based/meters) event with value 1 and a deterministic identifier, so retries never double count. Create the meter in the Stripe dashboard (test mode) with the event name above, `sum` aggregation, and the customer mapped from `stripe_customer_id`, then attach a metered price to each paid plan's subscription.

## Durability

Changes are written behind: each flush interval, a document's new updates are merged and appended to Postgres, and the document is compacted into a snapshot once enough rows build up, when it goes idle, and at shutdown. If the process dies between flushes, at most one interval of changes is missing on the server, and clients send it again when they reconnect, because y-sync starts every connection by exchanging what each side lacks.

On `SIGTERM` the server stops reporting ready, closes every WebSocket with `1012` so clients reconnect to the next instance, persists and compacts every loaded document, and gives queued webhooks five seconds. Give it at least 15 seconds before a hard kill.

## Scaling

One instance holds each document it serves in memory, so every client of a document must reach the same instance. Run one sync instance, or route by document name; cross-instance fan-out is not built yet. Conversions are stateless and scale out freely.

## Capacity

`cargo test --release -p canvist_cloud --test perf_budgets -- --ignored --nocapture` measures, on Apple silicon:

| Measure                                  | Measured | Budget   |
| ---------------------------------------- | -------- | -------- |
| One keystroke to the last of 100 peers   | 0.6 ms   | 10 ms    |
| Applying one keystroke                   | 1.5 µs   | 20 µs    |
| Persisting a 42,000-update backlog       | 21 ms    | 150 ms   |
| Opening one connection                   | 0.1 ms   | 2 ms     |
| Memory per idle connection (both ends)   | 17 KiB   | 64 KiB   |
| Memory per loaded 5-paragraph document   | 22 KiB   | 32 KiB   |
| Memory per loaded 50-paragraph document  | 132 KiB  | 192 KiB  |
| 10,000 paragraphs, Markdown to HTML      | 14 ms    | 60 ms    |
| 10,000 paragraphs, HTML to Markdown      | 16 ms    | 60 ms    |
| 10,000 paragraphs, JSON to Markdown      | 52 ms    | 200 ms   |
| Memory per loaded 500-paragraph document | 1.2 MiB  | 1.75 MiB |

A loaded document costs about 2.4 KiB per block plus 10 KiB. Documents only stay in memory while someone is connected and for the idle eviction period after, so memory follows concurrent documents, not total ones.
