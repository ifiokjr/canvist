# Internal docs

Design records, research, and plans for people working on canvist itself. These pages are deliberately outside `docs/src`, so they aren't part of the user docs, the book, or the website.

- [Architecture](./architecture.md): the design decisions behind canvist, and the alternatives considered.
- [Roadmap](./roadmap.md): the milestones and what each one delivers.
- [SSR CSS contract](./ssr-css-contract.md): what the server-rendered HTML pins so it wraps where the canvas does.
- [Native toolchain](./native-toolchain.md): building and testing the iOS, Android, and Flutter bindings.
- [Business model](./business.md): products, pricing principles, and infrastructure. User-facing plans live in [`docs/src/cloud/billing.md`](../src/cloud/billing.md).
- canvist Cloud operations: [Railway deployment](./cloud/railway.md), [the Stripe plan](./cloud/stripe-plan.md), and [billing operations](./cloud/billing-operations.md).
- Research: [the editor landscape](./research/editor-landscape.md) and [monetisation](./research/monetisation.md).

The performance budget table lives in the user docs, at [`docs/src/guides/performance.md`](../src/guides/performance.md).
