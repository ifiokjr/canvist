---
sdk: docs
app: docs
---

# Rewrite the docs for people and agents using canvist

- New structure: get started (one quick start per platform, each marked available, preview, or coming soon), core concepts, an extension catalogue, a build-your-own-editor guide, guides, an AI agent guide, canvist Cloud, and reference.
- Every Rust, TypeScript, and Dart sample runs in CI: Rust pages are doc tests of `canvist_book`, and `docs/check-snippets.ts` type-checks, analyzes, and runs the others, checking printed output against the comments.
- The website serves `/llms.txt` and `/llms-full.txt`, and groups the docs sidebar by `SUMMARY.md` parts. A draft `canvist` agent skill lives in `skills/canvist/`.
- Design records, research, the roadmap, and canvist Cloud operations move to `docs/internal/`, outside the published docs.
