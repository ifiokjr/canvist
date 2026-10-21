---
"@canvist/headless": patch
---

# Document mention, hashtag, and emoji arguments as comments in the generated descriptors

The generated descriptors for the mention, hashtag, and emoji extensions now carry their command and argument docs as comments, like every other extension, instead of `doc` fields. `engine.metadata()` still returns the docs at runtime.
