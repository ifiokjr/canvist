# Product

<!-- impeccable:product-schema 1 -->

> Inferred from the lead session's brief (2026-10-19), not from a live interview with the owner. Every fact below is either in the repository or in that brief. Items marked _(inferred)_ need the owner's confirmation.

## Platform

web

## Stack

Astro (static output, islands only where needed), Rive for the hero mark animation, GSAP for scroll choreography. Hosted on Railway as a service in the `canvist` project. The docs site will live in the same project later.

## Users

- **App developers choosing a rich-text editor** for a product that ships on more than one platform (web plus native or Flutter, React Native, Lynx). They are comparing canvist with TipTap, Lexical, ProseMirror, Quill and CKEditor, usually on a laptop between other tabs, and want to know within seconds whether canvist is real, fast and typed. _(inferred)_
- **Teams who need collaboration and conversion without running servers**, who come back for the pricing and services pages once canvist is in their app. _(inferred)_

## Product Purpose

canvist is an open-source rich-text editor that owns every pixel: one Rust core lays out and paints text onto a canvas on every platform instead of delegating to `contenteditable` or native text views. The site exists to make visitors feel that difference, then get them to the docs, the GitHub repo, or canvist Cloud.

## Positioning

The only editor whose one Rust core renders the same document, pixel for pixel, on web, desktop, iOS, Android, Flutter, React Native and Lynx, with keystroke-to-paint latency as a tracked, budgeted number and typed extension APIs that turn a missing extension into a compile error.

## Operating Context

- Source: github.com/ifiokjr/canvist. Docs source: `docs/src` (mdBook markdown). v2 architecture and budgets: `docs/src/v2/architecture.md`. Business model: `docs/src/v2/business.md` (branch `docs/business-model`).
- The v1 web editor (`packages/canvist`, wasm) works today and can be embedded live. The v2 web host (M3) is not built yet.
- canvist Cloud (Sync, Convert, History) is being built on Railway in the same project.

## Capabilities and Constraints

- Licence: Unlicense. The editor, its server and every binding are free forever.
- Hosted products: Sync (managed `yrs` relay; building), Convert (REST conversions; building), History (snapshots, versions, diff, restore; next). Review, AI, Self-host Support and UI Kits are later.
- Billing is per monthly active document (MAD). Plans are **provisional**: Hobby $0 (1,000 MADs, hard cap), Builder $29/month or $24 annual (10,000, $2 per 1,000 over), Scale $349/month or $299 annual (150,000, $1.50 per 1,000 over), Enterprise from $2,000/month annual. OSI projects get Builder free; companies under five people get 50% off the first year. Prices must always be labelled preview or early access.
- Self-hosting is first class: the exact binary canvist runs is published.
- No custom domain yet; the site lives on a Railway subdomain.

## Brand Commitments

- Name is always lowercase: canvist.
- Logo: the **Wet Caret**, a text caret whose foot is still wet (`assets/brand/`). Use the files as they are; never retype the wordmark in a live font; the caret only ever takes Paint, Paint on dark, or one mono colour; no shadows, gradients, glows, or exaggerated drips. The caret may blink (about 1 s, no fade) or let the drip fall once on first paint.
- Colours: Ink `#17141F`, Paint `#3B2CF5`, Paper `#F5F3FF`, Paint on dark `#8A80FF`, Tile `#FFFFFF`.
- Type: Bricolage Grotesque (wordmark outlined from ExtraBold); social cards use Bricolage Grotesque Medium and Geist Mono.
- The owner builds visual, playful, different things and rejects generic SaaS design.

## Evidence on Hand

- Benchmarks: the performance budget table in `docs/src/v2/architecture.md` (budget and today's measurement per hot path), enforced by `tests/perf_budgets.rs` in each crate.
- Live v1 editor: `packages/canvist` (wasm, ~420 KB gzipped).
- No customers, testimonials, logos, star counts or press exist. Do not fabricate any.
- A Remotion demo video is being produced separately; the hero keeps a slot for it.

## Product Principles

1. Prove, don't claim: every claim on the site is demonstrated live or cited from the repo.
2. Speed is the product: the site itself must be fast (Lighthouse 95+, LCP < 1.5 s, CLS 0).
3. Free forever is the promise; money only for running services.
4. One core, every platform: the same document, everywhere.

## Accessibility & Inclusion

WCAG 2.2 AA, fully usable down to 320 px wide, every animation respects `prefers-reduced-motion`.
