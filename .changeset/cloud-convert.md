---
"canvist_cloud": minor
---

# Add canvist Convert between ProseMirror JSON, HTML, Markdown, and text

Add canvist Convert to `canvist_cloud`: `POST /v1/convert` turns documents between ProseMirror JSON, HTML, Markdown, and plain text on `canvist_headless` with the StarterKit, and `GET /v1/documents/{name}?format=…` exports a synced document in any of them. Conversions run on the blocking pool under a size limit, a timeout, and a concurrency bound, and count toward the month's usage. `CANVIST_ROLE=convert` serves only these routes, so Convert scales as its own service from the same image.
