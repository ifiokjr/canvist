# Contributing

Read this page if you want to change canvist itself. Everything here is for contributors; you don't need any of it to use canvist.

## Set up

canvist uses [devenv](https://devenv.sh) for a reproducible toolchain: Rust, Deno, Node, Dart, Flutter, and the native SDKs.

```sh
git clone https://github.com/canvist/canvist
cd canvist
direnv allow .     # or: devenv shell
install:all
```

## Make a change

1. Branch from `main` with a conventional prefix: `feat/`, `fix/`, `docs/`, `test/`, `refactor/`.
2. Make focused commits, and add tests with the code.
3. Run the checks:

   ```sh
   fix:all      # format and fix lints
   test:all     # Rust tests and doc tests
   lint:style   # readability and complexity gates
   test:perf    # performance budgets, in release mode
   ```

4. Add a changeset if your change affects a package, with `mc change` (see [Changesets and releases](#changesets-and-releases)).
5. Open a pull request. CI runs every platform's tests, the performance budgets, and the docs checks.

## Changesets and releases

canvist releases with [monochange](https://github.com/monochange/monochange), configured in `monochange.toml`. Two release units move independently:

- **`sdk`**: every library (the Rust crates, `@canvist/headless` on npm and JSR, the Dart and Flutter packages, and the Swift and Android bindings). They share one version and are tagged `v<version>`.
- **`app`**: the website and canvist Cloud, deployed to Railway and tagged `app/v<version>`.

Every pull request that changes a package needs a file in `.changeset/`. Its frontmatter names the packages and a change type, and its body starts with a `# Summary` heading:

```markdown
---
canvist_core: feat
---

# Add soft line breaks

`Shift+Enter` inserts a hard break inside the current block.
```

Changesets have two audiences, and one file serves exactly one:

- **Developers** (`breaking`, `feat`, `change`, `fix`, `refactor`, `perf`, `security`, `docs`, `test`, `none`) end up in the package changelogs and GitHub releases. Use `mc change`.
- **People who use canvist** (`user_feature`, `user_fix`, `user_change`, `pricing`) end up on the website's `/changelog` page, and nowhere else. Use `mc user-change`, target `app`, and explain the outcome under a `## User impact` heading in plain language, with no API names or implementation detail.

A change that matters to both audiences gets two files. `mc check` lints changesets, and `mc preview` shows the next release.

On every push to `main`, the `release-pr` workflow opens or updates a release pull request with the version bumps and changelogs. Merging it tags the release, publishes each package to its registry, and deploys the app.

## Where things live

| Folder           | What's in it                                                                      |
| ---------------- | --------------------------------------------------------------------------------- |
| `crates/`        | The Rust core: model, editor, extensions, layout, collaboration, and headless API |
| `packages/`      | JavaScript packages, including `@canvist/headless`                                |
| `platforms/`     | Dart, Flutter, iOS, and Android bindings                                          |
| `services/cloud` | The canvist Cloud server                                                          |
| `conformance/`   | Language-neutral test fixtures every binding must pass                            |
| `docs/src`       | These docs                                                                        |
| `docs/internal`  | Design records, research, and plans for contributors                              |
| `website/`       | The website, which renders these docs at `/docs`                                  |

## Rules worth knowing

- **Performance budgets are tests.** Each hot path has a budget in [Performance](../guides/performance.md), and a release-mode test fails when it's exceeded. New hot paths add a budget and a benchmark.
- **Every binding passes the same fixtures.** A change to behaviour updates `conformance/fixtures`, so Rust, TypeScript, and Dart stay identical.
- **Docs samples are tested.** Rust samples run as doc tests through `docs/src/lib.rs`. TypeScript and Dart samples are type-checked and run by `docs/check-snippets.ts`. A sample's expected output goes in comments after the line that prints it.
- **Style:** hard tabs and edition 2024 in Rust, and a doc comment on every public item.

The design decisions behind canvist, with the alternatives considered, are in [`docs/internal/architecture.md`](https://github.com/canvist/canvist/blob/main/docs/internal/architecture.md).
