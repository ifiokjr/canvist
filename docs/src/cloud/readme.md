# canvist Cloud

Read this page to decide whether canvist Cloud is for you, and what it costs. canvist Cloud runs canvist's server-side features so you don't have to: real-time sync, format conversion, and, next, version history.

**Status: early access.** The services are live, and billing runs in Stripe's test mode, so nothing is charged yet. Projects are set up by hand during early access: [open an issue](https://github.com/ifiokjr/canvist/issues) to ask for one.

## What's included

| Service                 | What it does                                                                                  | Status    |
| ----------------------- | --------------------------------------------------------------------------------------------- | --------- |
| [Sync](./sync.md)       | A hosted y-websocket and Hocuspocus relay: persistence, client tokens, presence, and webhooks | Available |
| [Convert](./convert.md) | An HTTP API that converts between JSON, HTML, Markdown, and text                              | Available |
| History                 | Snapshots, named versions, and restore                                                        | Next      |

## The code is free; you pay for running it

canvist and canvist Cloud's server are both released under the Unlicense. You can [run the same server yourself](./self-hosting.md), for free, forever. canvist Cloud charges only for running it for you.

You pay per **monthly active document**: a document that someone opened in a sync session, or that your server wrote, during the calendar month. A document counts once a month, however much it's used. Web, mobile, desktop, and server clients all count the same. See [Plans and billing](./billing.md).

## Get started

1. Get a project and its API keys.
2. On your server, [mint a client token](./api.md#tokens) for each user.
3. Point your Yjs provider at Sync with that token. See [Sync](./sync.md).
