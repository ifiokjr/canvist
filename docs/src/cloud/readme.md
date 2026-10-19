# canvist Cloud

canvist Cloud runs canvist's server-side features so you don't have to: real-time sync, conversions, and soon history. The code is free and the same binary is yours to self-host; Cloud charges for running it, per monthly active document.

- **Sync:** a y-sync relay for canvist documents that stock `y-websocket` and Hocuspocus clients connect to, with presence, short-lived client tokens, persistence, and webhooks.
- **Convert:** REST conversions between ProseMirror JSON, HTML, Markdown, and plain text.

Read on:

- [API reference](./api.md): credentials, sync, tokens, documents, conversions, usage, and webhooks.
- [Self-hosting](./self-hosting.md): run the same image, its configuration, durability, and measured capacity.
- [Railway](./railway.md): how canvist Cloud itself is deployed, and what a document costs to run.
