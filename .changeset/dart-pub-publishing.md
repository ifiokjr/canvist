---
canvist-dart: fix
canvist_flutter: fix
---

# Publish the Dart and Flutter packages to pub.dev

The `canvist` package's Rust crate no longer inherits anything from the Cargo workspace. It names `canvist_headless` by the exact crates.io release it was generated against, which monochange bumps on every release, and the workspace's `[patch.crates-io]` builds the local sources inside this repository. So the build hook compiles the crate from the pub cache, where the workspace does not exist.

Both packages now ship a `LICENSE`, a `README.md`, and a repository link, and allow `flutter_rust_bridge` patch releases. CI validates both archives with `pub publish --dry-run` and builds and tests the `canvist` archive outside the repository. The release workflow publishes them after crates.io.
