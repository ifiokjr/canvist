# Contributing

Thank you for considering contributing to canvist!

## Development setup

See [Getting Started](./getting-started.md) for environment setup.

## Making changes

1. Create a feature branch from `main`.
2. Make your changes with clear, focused commits.
3. Add a changeset documenting your changes (see [Changesets and releases](#changesets-and-releases)):

   ```bash
   mc change --package canvist_editor --type feat --reason "Add soft line breaks"
   ```

4. Push your branch and open a pull request.

## Changesets and releases

canvist releases with [monochange](https://github.com/monochange/monochange), configured in `monochange.toml`. Two release units move independently:

- **`sdk`**: every library (the Rust crates, `@canvist/headless` on npm and JSR, the Dart and Flutter packages, and the Swift and Android bindings). They share one version and are tagged `v<version>`.
- **`app`**: the website and canvist Cloud, deployed to Railway and tagged `app/v<version>`.

Every pull request that changes a package needs a file in `.changeset/`. Its frontmatter names the packages and a change type, and its body starts with a `# Summary` heading:

```md
---
canvist_editor: feat
---

# Add soft line breaks

`Shift+Enter` inserts a hard break inside the current block.
```

Changesets have two audiences, and one file serves exactly one:

- **Developers** (`breaking`, `feat`, `change`, `fix`, `refactor`, `perf`, `security`, `docs`, `test`, `none`) end up in the package changelogs and GitHub releases. Use `mc change`.
- **People who use canvist** (`user_feature`, `user_fix`, `user_change`, `pricing`) end up on the website's `/changelog` page, and nowhere else. Use `mc user-change`, target `app`, and explain the outcome under a `## User impact` heading in plain language, with no API names or implementation detail.

A change that matters to both audiences gets two files. `mc check` lints changesets, and `mc preview` shows the next release.

On every push to `main`, the `release-pr` workflow opens or updates a release pull request with the version bumps and changelogs. Merging it tags the release, publishes each package to its registry, and deploys the app.

## Code style

- All Rust code uses **hard tabs** and edition 2024.
- Run `fix:all` before committing to auto-fix formatting and lints.
- Every public item must have a documentation comment.
- Use `thiserror` for error types.

## Testing

- Run `test:all` to execute the full test suite.
- Add tests for new functionality.
- Use `insta` for snapshot testing where appropriate.

## Architecture

See [Architecture](./architecture.md) for an overview of the crate structure.
