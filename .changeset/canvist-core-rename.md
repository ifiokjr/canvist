---
canvist_core: breaking
---

# Rename `canvist_editor` to `canvist_core`

The editing engine (editor state, commands, selection motion, keymaps, input rules, and history) is now the `canvist_core` crate, and `canvist_headless` re-exports it as `canvist_headless::canvist_core`. Through the `canvist` crate it is `canvist::core`. Nothing else about the API changed, so updating the dependency and the paths is the whole migration:

```rust
// Before
// or, through the crate apps install
use canvist::core::Editor;
// After
use canvist_core::Editor;
use canvist_editor::Editor;
use canvist_headless::canvist_core::Platform;
use canvist_headless::canvist_editor::Platform;
```
