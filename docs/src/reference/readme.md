# API docs and changelog

Use this page to find the full API reference for each package, and to see what changed in each release.

## API reference

Every public type, function, and command is documented in its package. The registries host these docs from the first release on. Until then, generate them locally from a clone of the repository:

| Package              | Language   | After the first release                                                                                                 | Today                                                 |
| -------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `canvist_headless`   | Rust       | [docs.rs/canvist_headless](https://docs.rs/canvist_headless)                                                            | `cargo doc -p canvist_headless --all-features --open` |
| `canvist_extensions` | Rust       | [docs.rs/canvist_extensions](https://docs.rs/canvist_extensions)                                                        | `cargo doc -p canvist_extensions --open`              |
| `canvist_editor`     | Rust       | [docs.rs/canvist_editor](https://docs.rs/canvist_editor)                                                                | `cargo doc -p canvist_editor --open`                  |
| `canvist_layout`     | Rust       | [docs.rs/canvist_layout](https://docs.rs/canvist_layout)                                                                | `cargo doc -p canvist_layout --open`                  |
| `@canvist/headless`  | TypeScript | [jsr.io/@canvist/headless](https://jsr.io/@canvist/headless) and [npm](https://www.npmjs.com/package/@canvist/headless) | `deno doc packages/canvist-headless/src/mod.ts`       |
| `canvist`            | Dart       | pub.dev, later                                                                                                          | `dart doc platforms/dart`                             |

Each extension's commands and queries are also listed, with their arguments, in the [extension catalogue](../extensions/readme.md). The browser packages' options and methods are on one page: [Browser API](./web-api.md).

## Changelog

- **[The changelog](/changelog)** on the website lists the changes that affect you, in plain language.
- **[GitHub releases](https://github.com/canvist/canvist/releases)** list every package's changes in each release, and [`changelog.md`](https://github.com/canvist/canvist/blob/main/changelog.md) collects them in one file.
