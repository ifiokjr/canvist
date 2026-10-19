# canvist business model

The editor, its server, and every binding are free forever under the Unlicense. canvist charges for **running** the service, never for the code. [The monetisation research](./research/monetisation.md) has the evidence behind this.

## Principles

- **No licence keys, watermarks, private registries, or loader-based billing.** The Unlicense makes them unenforceable, and both CKEditor and TinyMCE show they cost goodwill.
- **Bill per monthly active document (MAD):** a document with at least one sync session or API mutation in the billing month. It tracks real cost (memory per loaded `yrs` document) and treats web, desktop, mobile, Flutter, React Native, and Lynx the same.
- **Free plans have hard caps. Paid plans get warnings, never automatic upgrades.** Hosted features degrade on overage; the client never goes read-only.
- **Add-ons work on any paid plan.** One feature never forces a tier jump.
- **Self-hosting is a first-class path.** We publish the exact binary we run, with a guide.

## Products

| Product           | What it is                                                                        | Status   |
| ----------------- | --------------------------------------------------------------------------------- | -------- |
| canvist Sync      | Managed `yrs` relay: persistence, auth, presence, webhooks                        | Building |
| canvist Convert   | REST conversions between JSON, Markdown, HTML, and plain text; DOCX and PDF later | Building |
| canvist History   | Snapshots, named versions, diff, and restore                                      | Next     |
| canvist Review    | Comments, mentions, and tracked changes                                           | Later    |
| canvist AI        | Document-aware edits; bring your own key, or managed tokens at cost + 20%         | Later    |
| Self-host Support | Priority support and LTS for teams running their own server                       | Later    |
| UI Kits           | One-time purchase of polished per-platform toolbars and menus                     | Later    |

## Plans (provisional, pending real cost benchmarks)

The plan table lives in the user docs, at [`docs/src/cloud/billing.md`](../src/cloud/billing.md#plans), where the website's pricing page is tested against it.

OSI-licensed projects get Builder free. Companies with fewer than five people get 50% off for the first year.

These prices are **not final**. They are published only after canvist Cloud measures real memory per document and Railway cost per MAD, and the owner approves them.

## Infrastructure

Every hosted service runs on Railway. Each service has a `railway.json` and a Dockerfile in the repo, so a deploy is a git push and nobody runs their own instance by hand. The marketing site, docs, Sync, and Convert are separate Railway services in one project, with free dev and staging environments.
