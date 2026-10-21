---
"canvist_cloud": patch
---

# Deploy canvist Cloud on Railway as the sync and convert services

Deploy canvist Cloud on Railway: a multi-stage Dockerfile (`services/cloud/Dockerfile`) producing a Debian slim image with only the `canvist-cloud` binary, run as the `sync` and `convert` services of the `canvist` project in `production` and `staging`. Add a live smoke test (`services/cloud/tests/js/smoke.ts`) that syncs real Yjs clients through a deployment, and document the deployment, its variables, and its measured cost per thousand monthly active documents.
