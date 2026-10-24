All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add `canvist`, the one crate to install for canvist in Rust

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #190](https://github.com/canvist/canvist/pull/190)

`cargo add canvist` replaces adding `canvist_headless`, `canvist_extensions`, `canvist_core`, and the rest one by one. Each engine crate is a module of `canvist`, so `canvist_headless::Engine` is `canvist::headless::Engine`, and Cargo features choose the parts a build compiles.

- Default: `headless` and `starter-kit`, which is what a server, a script, or a test needs. `canvist::model`, `canvist::core`, and `canvist::extensions` are in every build.
- `extras` and `full` add every extension beyond the StarterKit; `collab`, `journal`, and `ssr` turn on the matching `canvist_headless` features and add `canvist::collab`.
- `layout` and `parley` add `canvist::layout`; `canvas`, `paint`, `png`, and `gpu` add the design canvas (`canvist::scene`, `canvist::canvas`, `canvist::paint`); `headless-scene` adds `canvist::headless::SceneEngine`.
