# canvist Cloud on Railway

canvist Cloud runs on [Railway](https://railway.com) as one project, `canvist`, with two environments, `production` and `staging`. Every service builds from this repository, so a deploy is a merge to `main`; nobody builds or runs an instance by hand.

## Live

| Environment  | Sync                                          | Convert                                          |
| ------------ | --------------------------------------------- | ------------------------------------------------ |
| `production` | <https://sync-production-5340.up.railway.app> | <https://convert-production-16ce.up.railway.app> |
| `staging`    | <https://sync-staging-0f98.up.railway.app>    | <https://convert-staging.up.railway.app>         |

Clients connect to `wss://<sync host>/v1/sync` (y-websocket) or `wss://<sync host>/v1/hocuspocus`. Both environments run in `us-east4` (Virginia) next to their Postgres: the best single region for users on both sides of the Atlantic. Check a deployment end to end with the smoke test, which creates a throwaway project and syncs real Yjs clients:

```sh
railway run --service sync --environment staging -- \
	deno run --allow-net --allow-env services/cloud/tests/js/smoke.ts \
	https://sync-staging-0f98.up.railway.app https://convert-staging.up.railway.app
```

## Services

| Service    | Source                       | Role                   | Notes                                                    |
| ---------- | ---------------------------- | ---------------------- | -------------------------------------------------------- |
| `sync`     | `services/cloud/Dockerfile`  | `CANVIST_ROLE=sync`    | The relay and the document API. One replica (see below). |
| `convert`  | `services/cloud/Dockerfile`  | `CANVIST_ROLE=convert` | Stateless conversions; scale replicas freely.            |
| `Postgres` | Railway's Postgres template  |                        | Reached over the private network.                        |
| `website`  | `website/` (its own session) |                        | The marketing site, in the same project.                 |

`sync` and `convert` are the same image; only `CANVIST_ROLE` differs. Both use:

- **Build:** the Dockerfile at `services/cloud/Dockerfile`, with the repository root as the context (`RAILWAY_DOCKERFILE_PATH`). Pushes that touch nothing under `services/cloud`, `crates`, `Cargo.toml`, `Cargo.lock`, or `rust-toolchain.toml` do not rebuild.
- **Health:** Railway waits for `GET /readyz` before switching traffic, up to 120 seconds.
- **Shutdown:** 30 seconds of draining after `SIGTERM`, which covers the server's graceful shutdown (close sockets with `1012`, persist and compact every document, send queued webhooks).
- **Restarts:** on failure, up to five times.

`sync` runs one replica: each instance keeps the documents it serves in memory, and Railway does not route WebSockets by document. Scale it vertically until cross-instance fan-out exists.

## Variables

| Variable                  | `sync`                                  | `convert`                        |
| ------------------------- | --------------------------------------- | -------------------------------- |
| `CANVIST_ROLE`            | `sync`                                  | `convert`                        |
| `DATABASE_URL`            | `${{Postgres.DATABASE_URL}}`            | `${{Postgres.DATABASE_URL}}`     |
| `CANVIST_TOKEN_SECRET`    | 32 random bytes, hex, per environment   | `${{sync.CANVIST_TOKEN_SECRET}}` |
| `CANVIST_ADMIN_TOKEN`     | 32 random bytes, hex, per environment   | `${{sync.CANVIST_ADMIN_TOKEN}}`  |
| `RAILWAY_DOCKERFILE_PATH` | `services/cloud/Dockerfile`             | `services/cloud/Dockerfile`      |
| `STRIPE_SECRET_KEY`       | a **test-mode** key, when billing is on | unset                            |
| `STRIPE_WEBHOOK_SECRET`   | the webhook's signing secret            | unset                            |

Secrets live only in Railway variables, never in the repository. Generate one without printing it:

```sh
openssl rand -hex 32 | railway variable set CANVIST_ADMIN_TOKEN --stdin --service sync --environment staging --skip-deploys
```

Read the admin token from the Railway dashboard (service, Variables) when you need it.

## How changes ship

- **Staging** deploys every push to `main` as soon as it lands.
- **Production** deploys the same `main` commits once GitHub's checks pass on them.

Both environments watch `main`, so staging runs each commit first and production follows when CI is green. To gate production harder, point its services at a release branch.

Every service has a GitHub deploy trigger on `ifiokjr/canvist@main`; production's has **Wait for CI** on. The [Railway GitHub app](https://github.com/apps/railway-app) is installed on that repository only. Until the cloud stack merges, `main` has no `services/cloud/Dockerfile`, so pushes there fail to build and the running deployments stay up; deploy a branch by hand with `railway up --service sync --environment staging --ci` (and likewise for `convert` and `production`).

## Why there is no `railway.json`

Railway now refuses config-as-code files (`railway.json`, `railway.toml`) in favour of [infrastructure as code](https://docs.railway.com/infrastructure-as-code) in `.railway/railway.ts`, which describes the whole project in one file. That DSL has no Dockerfile path yet, so the settings above were applied through Railway's API (`serviceInstanceUpdate`, and `environmentPatchCommit` to add the services to `production`), with `RAILWAY_DOCKERFILE_PATH` pointing the builder at the Dockerfile. Moving the whole project, the website included, to `.railway/railway.ts` is a follow-up once the DSL covers Dockerfile builds.

## Performance

`services/cloud/tests/js/load.ts` drives real y-websocket clients against a deployment, each typing into its own block with its caret in awareness, and times every keystroke from one client to the others:

```sh
railway run --service sync --environment staging -- \
	deno run --allow-net --allow-env services/cloud/tests/js/load.ts \
	https://sync-staging-0f98.up.railway.app 20 5 30 5
```

From London to `us-east4`, 100 clients in 20 rooms typing 5 keystrokes a second each (500 a second, about 60,000 deliveries in 30 seconds) see every keystroke arrive, at p50 107 ms, p90 170 ms, and p99 300 ms. That is the network: the relay adds well under a millisecond, and the same load used 0.02 vCPU and 8 MB on `sync`. Users nearer Virginia see correspondingly less. Moving from `us-west2` cut the p50 from 156 ms.

What keeps it cheap and fast:

- **No echo.** A client's own caret is not sent back to it, which was a third of a typing client's traffic; a quiet socket gets a few-byte keepalive every 15 seconds instead, so clients never hit their 30-second reconnect.
- **One write per tick.** Each 250 ms flush writes every edited document in one transaction of two statements, so Postgres sees four writes a second however many documents are open. 1,000 edited documents flush in 20 ms against Postgres, down from 155 ms when each wrote its own transaction.
- **Small sockets and documents.** 17 KiB per connection and about 2.4 KiB per block in memory, and idle documents leave memory after a minute.

## Spending controls

- The workspace has a **$20 hard limit** and a **$10 email alert** (`railway usage limit status`).
- Each service has resource caps, so a runaway process stops at its cap rather than growing the bill: `sync` 2 vCPU and 2 GB, `convert` 2 vCPU and 1 GB.
- Staging's `sync` and `convert` sleep after 10 minutes without traffic and wake on the next request.

## Cost

Railway bills usage per second: $10 per GB of memory a month, $20 per vCPU a month, $0.15 per GB of volume a month, and $0.05 per GB of egress. The measurements in [self-hosting](./self-hosting.md#capacity) turn into these costs for 1,000 monthly active documents (MADs):

| Cost               | Assumption                                                                                                                                             | Per 1,000 MADs a month |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- |
| Memory, typical    | A 50-paragraph document (132 KiB loaded) open 10 hours a month plus the one-minute idle tail: 14 loaded at once                                        | $0.02                  |
| Memory, worst case | Every document loaded all month, two connections each (17 KiB per connection)                                                                          | $1.60                  |
| CPU                | 36,000 keystrokes per document, two peers, about 5 µs of CPU per keystroke and peer                                                                    | under $0.01            |
| Storage            | 50 KiB per document in Postgres, snapshots and uncompacted rows together                                                                               | under $0.01            |
| Egress             | Measured: a keystroke reaches each other peer as about 115 bytes (update and caret), so 36,000 keystrokes to one other peer is about 4 MB per document | $0.21                  |

So active documents cost canvist about $0.25 per thousand a month in the typical case, mostly egress, and about $2 per thousand at worst.

The fixed cost is small too. Measured idle on Railway: `sync` holds 5.9 MB, `convert` 2.8 MB, and Postgres 78 MB with a 135 MB volume, at under 0.01 vCPU each: about $1 a month per environment, inside the Hobby plan's $5 of included usage.

Against the provisional prices in [the business model](../v2/business.md): the free 1,000 MADs cost about $0.25 a month, and Builder's $2 per 1,000 overage leaves a wide margin even in the worst case. Caret traffic depends on the client, so keep checking egress in Railway's metrics as real traffic arrives.
