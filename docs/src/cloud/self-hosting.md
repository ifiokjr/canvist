# Self-hosting

In this guide you run canvist Cloud's server yourself: the same binary canvist Cloud runs, free, with no caps. You'll start it, create a project, configure it, and learn how it stores documents and scales.

## 1. Start the server

The server is one binary, `canvist-cloud`, in `services/cloud`. Build its image from the repository root, then run it with a token secret, an admin token, and a Postgres database:

```sh
docker build -f services/cloud/Dockerfile -t canvist-cloud .
docker run -p 8080:8080 \
  -e CANVIST_TOKEN_SECRET="$(openssl rand -hex 32)" \
  -e CANVIST_ADMIN_TOKEN="$(openssl rand -hex 32)" \
  -e DATABASE_URL=postgres://user:password@db.internal:5432/canvist \
  canvist-cloud
```

Or run it from source:

```sh
CANVIST_TOKEN_SECRET=$(openssl rand -hex 32) \
CANVIST_ADMIN_TOKEN=$(openssl rand -hex 32) \
DATABASE_URL=postgres://user:password@localhost:5432/canvist \
cargo run --release -p canvist_cloud
```

Without `DATABASE_URL`, documents live in memory and vanish on restart, which is only useful for trying it out. Database migrations run on startup.

## 2. Create a project

Create a project with the admin token. It returns one API key per environment, shown once:

```sh
curl -X POST http://localhost:8080/v1/admin/projects \
  -H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{ "name": "my-app", "environments": ["production", "staging"] }'
```

New projects start on the free plan's 1,000-document cap. Remove the cap:

```sh
curl -X PUT http://localhost:8080/v1/admin/projects/$PROJECT/terms \
  -H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{ "plan": "enterprise" }'
```

Other admin routes: `POST /v1/admin/projects/{project}/keys` with `{ "environment": "staging" }` adds a key, and `GET /v1/admin/stats` shows loaded documents and connected clients.

Then follow [Sync](./sync.md) and [Convert](./convert.md) with `localhost:8080` as the host.

## Configuration

| Variable                        | Default      | Meaning                                                                      |
| ------------------------------- | ------------ | ---------------------------------------------------------------------------- |
| `CANVIST_TOKEN_SECRET`          | required     | Signs client tokens; at least 32 bytes. Rotating it signs everyone out.      |
| `DATABASE_URL`                  | in memory    | Postgres. Connections are unencrypted: use a private network or a TLS proxy. |
| `CANVIST_ADMIN_TOKEN`           | off          | Turns on `/v1/admin`.                                                        |
| `HOST`, `PORT`                  | `::`, `8080` | Where to listen. `::` accepts IPv4 and IPv6.                                 |
| `CANVIST_ROLE`                  | `all`        | `all`, `sync`, or `convert`: which routes this instance serves.              |
| `CANVIST_IDLE_EVICTION_SECONDS` | `60`         | How long a document with no connections stays in memory.                     |
| `CANVIST_FLUSH_INTERVAL_MS`     | `250`        | How often changes are written to Postgres.                                   |
| `CANVIST_MAX_MESSAGE_BYTES`     | `1048576`    | The largest WebSocket message and REST body.                                 |
| `CANVIST_MAX_DOCUMENT_BYTES`    | `16777216`   | The largest document, as Yjs state.                                          |
| `CANVIST_WEBHOOK_ALLOW_PRIVATE` | `false`      | Allow HTTP and private hosts as webhook targets, inside your network.        |
| `CANVIST_CONVERT_MAX_BYTES`     | `8388608`    | The largest conversion request.                                              |
| `CANVIST_CONVERT_TIMEOUT_MS`    | `10000`      | The longest a conversion may run.                                            |
| `CANVIST_LOG_FORMAT`            | text         | `json` for structured logs.                                                  |
| `RUST_LOG`                      | `info`       | Log filter, such as `canvist_cloud=debug`.                                   |

The server also reads settings for compaction, webhook timing, conversion concurrency, and Stripe billing. They're documented in `services/cloud`, and you rarely need them when self-hosting.

## How documents are stored

Changes are written behind, every flush interval, and folded into a snapshot once enough build up, when a document goes idle, and at shutdown. If the process dies between flushes, clients send the missing changes again when they reconnect, because every sync session starts by exchanging what each side lacks.

On `SIGTERM`, the server stops reporting ready, asks every client to reconnect (close code `1012`), and saves every loaded document. Give it at least 15 seconds before a hard kill.

## Scaling

Each instance holds the documents it serves in memory, so every client of one document must reach the same instance. Run one sync instance, or route by document name. Conversions are stateless and scale out freely.

What one instance handles, measured on Apple silicon:

| Measure                                 | Measured |
| --------------------------------------- | -------- |
| One keystroke to the last of 100 peers  | 0.6 ms   |
| Applying one keystroke                  | 1.5 µs   |
| Memory per idle connection              | 17 KiB   |
| Memory per loaded 50-paragraph document | 132 KiB  |
| 10,000 paragraphs, Markdown to HTML     | 14 ms    |

A loaded document costs about 2.4 KiB per block, plus 10 KiB. Documents stay in memory only while someone is connected, so memory follows concurrent documents, not total ones.
