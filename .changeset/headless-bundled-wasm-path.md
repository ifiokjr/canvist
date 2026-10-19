---
"@canvist/headless": fix
---

# Read the bundled WebAssembly module from a file path

`load()` converts the bundled module's `file:` URL to a path before reading it, so the package type-checks under JSR's `deno publish` as well as with `@types/node`.
