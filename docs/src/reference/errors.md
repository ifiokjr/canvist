# Errors

Use this page to look up an error code and fix what caused it. Every package uses the same codes, so you can handle errors the same way in Rust, TypeScript, and Dart.

## Reading the code

| Language   | Error type         | Code                                        |
| ---------- | ------------------ | ------------------------------------------- |
| TypeScript | `CanvistError`     | `error.code`, after `isCanvistError(error)` |
| Rust       | `HeadlessError`    | `error.code()`, an `ErrorCode`              |
| Dart       | `CanvistException` | `exception.code`, an `ErrorCode`            |

```ts
import { createEngine, isCanvistError } from "@canvist/headless";

const engine = await createEngine();
const stored = '{ "type": "doc", "content": [{ "type": "image" }] }';
try {
	engine.fromJson(JSON.parse(stored));
} catch (error) {
	if (!isCanvistError(error)) throw error;
	console.log(error.code);
	// schema
}
```

## Codes

| Code              | What happened                                                     | Fix                                                                                                 |
| ----------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `config`          | The engine's extensions don't fit together                        | Check the extensions you registered, and the ones they depend on                                    |
| `parse`           | Input isn't valid in its format, such as malformed JSON           | Check the input, or load it as HTML or Markdown, which never fail                                   |
| `schema`          | JSON contains blocks or marks the engine didn't register          | Register the missing extensions, or import through HTML or Markdown                                 |
| `unknownName`     | A command or query name doesn't exist on this engine              | Check the [catalogue](../extensions/readme.md), and register the extension                          |
| `invalidArgs`     | A command's arguments are missing or out of range                 | Match the command's arguments, such as a heading level from 1 to 6                                  |
| `invalidPosition` | A position points at a block that doesn't exist, or past its text | Use the path of a text block, and an offset in UTF-16 code units within its text                    |
| `invalidKey`      | A key name passed to `press` isn't valid                          | Write keys like `Mod-b` or `Shift-Enter`; see [Keyboard shortcuts](../guides/keyboard-shortcuts.md) |
| `editor`          | The editing engine refused a change                               | Report it, with the document and the call that failed                                               |
| `snapshot`        | A snapshot passed to `restore` is malformed                       | Pass the string `snapshot()` returned, unchanged                                                    |
| `notShared`       | A collaboration call on a document that isn't shared              | Call `share(clientId)` first, or load the document with `fromYjs`                                   |
| `alreadyShared`   | `share` on a document that's already shared                       | Share once                                                                                          |
| `collab`          | A Yjs update or sync message couldn't be decoded or applied       | Check the transport passes binary messages through unchanged                                        |
| `render`          | Server-side rendering failed                                      | Check registered fonts are valid TrueType or OpenType files                                         |
