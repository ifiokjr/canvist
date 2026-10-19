---
"canvist_cloud": minor
---

Cut canvist Sync's traffic and database load. A client's own caret is no longer echoed back to it, which was a third of a typing client's traffic; quiet sockets get a keepalive instead, so clients never hit their reconnect timeout. Each flush now writes every edited document in one transaction of two statements, so Postgres sees four writes a second however many documents are open: 1,000 edited documents flush in 20 ms, down from 155 ms. Adds a live load test (`services/cloud/tests/js/load.ts`), and the smoke and load tests no longer let same-process clients bypass the relay over `BroadcastChannel`.
