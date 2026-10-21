---
"canvist-rust": minor
---

# Add `canvist`, the one crate to install for canvist in Rust

`cargo add canvist` replaces adding `canvist_headless`, `canvist_extensions`, `canvist_editor`, and the rest one by one. Each engine crate is a module of `canvist`, so `canvist_headless::Engine` is `canvist::headless::Engine`, and Cargo features choose the parts a build compiles.

- Default: `headless` and `starter-kit`, which is what a server, a script, or a test needs. `canvist::model`, `canvist::editor`, and `canvist::extensions` are in every build.
- `extras` and `full` add every extension beyond the StarterKit; `collab`, `journal`, and `ssr` turn on the matching `canvist_headless` features and add `canvist::collab`.
- `layout` and `parley` add `canvist::layout`; `canvas`, `paint`, `png`, and `gpu` add the design canvas (`canvist::scene`, `canvist::canvas`, `canvist::paint`); `headless-scene` adds `canvist::headless::SceneEngine`.
