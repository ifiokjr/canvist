---
"@canvist/headless": change
---

# Keep descriptions out of the generated extension descriptors

The descriptors in `@canvist/headless/extensions` no longer carry each command, query, and field's `doc` as data: it is a comment above each entry in the generated file. Types never read it, and as data it shipped in every browser bundle (627 bytes gzipped of `@canvist/web`). `doc` is now optional on `CommandSpec`, `QuerySpec`, and `FieldSpec`, and `engine.metadata()` is typed as `ExtensionDescriptor[]`, with every description, for tools such as agent tool schemas that need them.
