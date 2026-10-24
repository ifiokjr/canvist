# canvist app: developer changelog

Deployment, infrastructure, and implementation notes for the website and canvist Cloud. The user-facing changelog lives at `/changelog` on the website.

## 0.1.1 (2026-10-24)

Grouped release for `app`.

### 🚀 Feature

- **Sell annual Builder and Scale plans self-serve.** `POST /v1/billing/checkout` now takes `"interval": "year"` (monthly stays the default). An annual plan bills its base fee yearly and its overage monthly, which Stripe Checkout cannot create, so the annual Checkout page saves a card and the Stripe webhook creates the subscription in flexible billing mode, anchored to the first of the month. A retried or repeated webhook never creates a second subscription, and `POST /v1/billing/plan` keeps an annual subscription annual when it switches between Builder and Scale. _Packages:_ 🟠 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #142](https://github.com/canvist/canvist/pull/142)
- **Bill canvist Cloud through a durable usage outbox, Checkout, and the Customer Portal.** Bill canvist Cloud the way Stripe recommends. Usage is reported through a durable Postgres outbox, so restarts and Stripe outages delay it without losing it, and conversions are now billed on the `canvist_conversion` meter as aggregated counts. Subscriptions start with `POST /v1/billing/checkout`, anchored to the first of the month so Stripe's billing periods match monthly active documents; `POST /v1/billing/plan` switches between Builder and Scale; and `POST /v1/billing/portal` opens the Customer Portal. The webhook now handles checkout and invoice events and applies each subscription's current state, so out-of-order events and old subscriptions cannot change a project's plan, and a failed payment keeps the plan while Stripe retries. The setup script adds product tax codes, tax-exclusive prices, the Customer Portal configuration, and a Stripe Tax readiness check. See `docs/src/cloud/stripe-plan.md`. _Packages:_ 🟠 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #122](https://github.com/canvist/canvist/pull/122)
- **Add canvist Convert between ProseMirror JSON, HTML, Markdown, and text.** Add canvist Convert to `canvist_cloud`: `POST /v1/convert` turns documents between ProseMirror JSON, HTML, Markdown, and plain text on `canvist_headless` with the StarterKit, and `GET /v1/documents/{name}?format=…` exports a synced document in any of them. Conversions run on the blocking pool under a size limit, a timeout, and a concurrency bound, and count toward the month's usage. `CANVIST_ROLE=convert` serves only these routes, so Convert scales as its own service from the same image. _Packages:_ 🟠 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #118](https://github.com/canvist/canvist/pull/118)

#### Count monthly active documents (MADs) and enforce plans

_Packages:_ 🟠 _canvist_cloud_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #117](https://github.com/canvist/canvist/pull/117)

- A document is active in a month once a sync session opens it or the REST API writes it, counted once per project environment and month in a `usage` table. `GET /v1/usage` reports it against the plan as `ok`, `approaching`, `over`, or `capped`.
- The free plan's cap pauses only sessions on documents new this month: the socket stays open, the client keeps editing locally, documents already active keep syncing, and a paused session syncs on its own after an upgrade. Admissions take a per-project lock, so concurrent sessions cannot overshoot the cap.
- Paid plans warn and never pause. New paid documents send one Stripe Billing Meter event each, test-mode keys only, with deterministic identifiers.

- **Stop echoing carets and batch persistence in canvist Sync.** A client's own caret is no longer echoed back to it, which was a third of a typing client's traffic; quiet sockets get a keepalive instead, so clients never hit their reconnect timeout. Each flush now writes every edited document in one transaction of two statements, so Postgres sees four writes a second however many documents are open: 1,000 edited documents flush in 20 ms, down from 155 ms. Adds a live load test (`services/cloud/tests/js/load.ts`), and the smoke and load tests no longer let same-process clients bypass the relay over `BroadcastChannel`. _Packages:_ 🟠 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #129](https://github.com/canvist/canvist/pull/129)
- **Turn Stripe subscription events into project plans.** Close the billing loop with Stripe: `POST /v1/stripe/webhook` verifies Stripe's signature and turns subscription events into project plans, so an upgrade unpauses a full free plan on its own. Add `services/cloud/stripe/setup.ts` (`deno task stripe:setup`), an idempotent, test-mode-only script that creates the Billing Meters, products, prices, and webhook endpoint for the provisional plans, and document billing in `docs/src/cloud/billing.md`. _Packages:_ 🟠 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #121](https://github.com/canvist/canvist/pull/121)

#### Add `canvist_cloud`, starting with Sync: a multi-tenant y-sync relay

_Packages:_ 🟠 _canvist_cloud_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #116](https://github.com/canvist/canvist/pull/116)

- Stock `y-websocket` and Hocuspocus clients connect to `/v1/sync/{document}` and `/v1/hocuspocus`, with presence, read-only sessions, and per-document access from short-lived HS256 tokens that API keys mint.
- Documents persist to Postgres as merged Yjs updates, compact into snapshots, and leave memory once idle. Updates that write outside the `blocks` root or put bare values in it are refused.
- Debounced, signed `document.updated` webhooks; REST endpoints to list, read, write, and delete documents; health and readiness probes; and a graceful shutdown that persists every room.
- `canvist_collab::Reorder` is now public, so hosts with a bare yrs document apply out-of-order updates safely.

#### Add the canvist website, deployed to Railway

_Packages:_ 🟠 _website_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #115](https://github.com/canvist/canvist/pull/115)

Add canvist's website in `website/`: an Astro site with the landing page, preview pricing with a monthly-active-document calculator, the Sync, Convert and History service pages, and a `/docs` section that renders `docs/src` with Pagefind search.

- The landing page is a "fan deck" of Paint swatches that fans open as you scroll. The hero's Wet Caret paints the wordmark in Rive (`website/rive/`, generated from the brand masters). The page also has the engine typing live with a keystroke-to-paint readout, one document morphing across seven hosts, simulated multiplayer carets, a TypeScript error that lights up when an extension is missing, and benchmark bars read from the architecture doc's budget table at build time.
- Every animation has a reduced-motion state, and GSAP, Rive and the wasm engine never load under reduced motion.
- The site deploys to Railway as the `website` service, built from `website/Dockerfile` at the repository root and served by Caddy.
- A new `website` CI workflow runs the type check, unit tests, the build, Playwright (navigation, reduced motion, responsiveness at 320, 768, 1280 and 1920 px, and axe accessibility checks) and Lighthouse CI. A macOS job compares visual snapshots.

- **Add the /changelog page for app releases.** `/changelog` renders the `changelog` collection (the user release stream monochange writes into `website/src/content/changelog/`), newest version first, with a linkable heading per release and an empty state until the first app release. The footer links to it, and the page joins the end-to-end sweeps (navigation, responsiveness and accessibility in both themes). _Packages:_ 🟠 _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #127](https://github.com/canvist/canvist/pull/127)

#### Embed the web editor on the landing page

_Packages:_ 🟠 _website_

The "Type into the real engine" demo now runs `@canvist/web`, the editor canvist ships.

- The page arrives with the demo's document as HTML, rendered at build time by `@canvist/headless/ssr` with the page's own Bricolage and Geist Mono faces. The editor hydrates from it and swaps in the canvas in one frame, with no layout shift. Visitors without JavaScript, and crawlers, see the text.
- The editor still loads only once the section is on screen and scrolling has settled. Under reduced motion or Save-Data it loads when the visitor presses the document or the button. Starting it leaves focus and the scroll position where they were.
- The sheet follows the site's light and dark themes, and the theme toggle recreates the editor in the other scheme, keeping the document, selection, and focus.
- The readout's values have room for "123.4 ms", so new numbers no longer move the labels.
- The website build compiles `@canvist/headless` and `@canvist/web` from the workspace, in a `packages` CI job and a Rust stage in `website/Dockerfile`.

#### Redesign the landing page for clarity and smooth motion

_Packages:_ 🟠 _website_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #126](https://github.com/canvist/canvist/pull/126)

- The landing page has six beats with one idea each: the hero, the live editor, one document on every platform, writing together, free forever plus the services, and the call to action. Benchmarks, the platform matrix and API samples now live in the docs.
- Motion is CSS transitions on `transform` and `opacity` only, with one easing. Nothing is pinned or scroll-linked, and GSAP is gone. Scrolling stays inside one 60 Hz frame with no long tasks, even with the CPU throttled 4×.
- Nothing is cut off at any common screen size. A new Playwright check scrolls every page across nine viewports and fails on clipped content or horizontal overflow. Visual snapshots cover each viewport's first screen.
- The live editor no longer steals keyboard focus when it loads, and starts only while its section is on screen and scrolling has paused.

### 📝 Changed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Packages:_ 🟢 _canvist_cloud_, 🟠 _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
- **Show every package readme in the docs, without its GitHub header.** The docs' Packages section now includes `@canvist/web`, `@canvist/react`, `@canvist/vue`, `@canvist/headless`, and `@canvist/canvas`, whose lowercase `readme.md` files the loader used to miss on case-sensitive file systems. Readmes open with a centred logo, links, and badges for GitHub and the registries; the site drops that whole header, since its own page header already carries the logo. _Packages:_ 🟠 _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #147](https://github.com/canvist/canvist/pull/147)
- **Run Lighthouse on mobile too and report scores in CI.** The website's CI runs Lighthouse with both the desktop preset and the mobile profile, and writes a median-per-page score table to every run's summary. Desktop must reach 95+ in every category, LCP under 1.5 s and CLS 0. Mobile must reach the same scores and CLS 0, and reports LCP over 1.5 s as a warning. The docs search hint no longer shifts layout. _Packages:_ 🟠 _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #124](https://github.com/canvist/canvist/pull/124)

### 🐛 Fixed

- **Deploy canvist Cloud on Railway as the sync and convert services.** Deploy canvist Cloud on Railway: a multi-stage Dockerfile (`services/cloud/Dockerfile`) producing a Debian slim image with only the `canvist-cloud` binary, run as the `sync` and `convert` services of the `canvist` project in `production` and `staging`. Add a live smoke test (`services/cloud/tests/js/smoke.ts`) that syncs real Yjs clients through a deployment, and document the deployment, its variables, and its measured cost per thousand monthly active documents. _Packages:_ 🟢 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #119](https://github.com/canvist/canvist/pull/119)

#### Fix three ways collaborating peers' documents could diverge

_Packages:_ 🟢 _canvist_cloud_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #135](https://github.com/canvist/canvist/pull/135)

- A block copy that loses its id to a concurrently moved copy is now evicted from where it sits in the shared array, not from its old place in the model. Blocks inserted after it in the same update used to land out of order on the receiving peer.
- A hidden copy of a block, such as one inside a list that no longer allows it, no longer claims the block's id when reading a document from scratch. `Binding::document` used to hide the shown copy and delete it from the shared document.
- `Reorder::take_ready` is replaced by `Reorder::take_next`, which releases one update at a time against the document's current state vector. An update that yrs keeps pending, because it is inserted next to another client's item that has not arrived, no longer lets that client's later updates overtake it. The binding and canvist Sync rooms both apply updates this way.

- **Update rustls-webpki to 0.103.16.** canvist Cloud verifies TLS certificates with rustls-webpki, which 0.103.16 patches for RUSTSEC-2026-0382. _Packages:_ 🟢 _canvist_cloud_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #158](https://github.com/canvist/canvist/pull/158)
- **Show iOS, Android, and Flutter as available on the landing page.** The landing page's platform list now marks iOS (UIKit and SwiftUI), Android (Views and Jetpack Compose), and Flutter as available, matching the docs, and says "coming soon" for React Native and Lynx instead of internal milestone codes. _Packages:_ 🟢 _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #197](https://github.com/canvist/canvist/pull/197)

<details>
<summary><strong>📖 Documentation</strong></summary>

#### Rewrite the docs for people and agents using canvist

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #130](https://github.com/canvist/canvist/pull/130)

- New structure: get started (one quick start per platform, each marked available, preview, or coming soon), core concepts, an extension catalogue, a build-your-own-editor guide, guides, an AI agent guide, canvist Cloud, and reference.
- Every Rust, TypeScript, and Dart sample runs in CI: Rust pages are doc tests of `canvist_book`, and `docs/check-snippets.ts` type-checks, analyzes, and runs the others, checking printed output against the comments.
- The website serves `/llms.txt` and `/llms-full.txt`, and groups the docs sidebar by `SUMMARY.md` parts. A draft `canvist` agent skill lives in `skills/canvist/`.
- Design records, research, the roadmap, and canvist Cloud operations move to `docs/internal/`, outside the published docs.

- **Order the integrations by ecosystem popularity.** The docs sidebar, the platform tables, the agent guide, and `/llms.txt` now list integrations by expected usage: the TypeScript ecosystem first (React, React Native, Vue, web, server-side rendering, the TypeScript server, and the design canvas), then Flutter with the Dart server, iOS, Android, desktop with the Rust server, and Lynx last. _Packages:_ ⚪ _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #209](https://github.com/canvist/canvist/pull/209)

#### Document React Native and Lynx, and list them as previews

_Packages:_ ⚪ _website_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #199](https://github.com/canvist/canvist/pull/199)

- New quick starts for [React Native](docs/src/quick-start/react-native.md) and [Lynx](docs/src/quick-start/lynx.md): install from the repository, show an editor, add a toolbar that follows the selection, save and load, and what calls from JavaScript cost.
- The platform tables in the README and the quick-start index, the website's platforms section, and the agent notes in `/llms.txt` list React Native and Lynx as previews, installed from GitHub.
- The agent guide and the `canvist` skill list React Native and Lynx, and the skill the iOS, Android, and desktop editors, among the packages that exist.

- **Mark the browser platforms Available.** The web, React, Vue, server-side rendering, and design canvas quick starts, the platform tables in the readme and the docs, the agent guide and skill, and the website's platform list now say "Available". The package readmes describe read-only editors, placeholders, menus, collaboration, server rendering, and the bundle budget. _Packages:_ ⚪ _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/canvist/canvist/pull/173)
- **List the desktop editor as available on the site and in llms.txt.** The platforms section shows Desktop as available on macOS, Windows, and Linux, and the agent notes in `/llms.txt` name `canvist_desktop` among the packages that work today. _Packages:_ ⚪ _website_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #163](https://github.com/canvist/canvist/pull/163)

</details>
