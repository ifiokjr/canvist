# canvist conformance fixtures

These fixtures are the contract every headless binding passes: the Rust `canvist_headless` crate, the TypeScript package, and the Dart package. Each one loads a document, runs a script of operations through the binding's own API, and compares the result. Because every binding wraps the same Rust core, a failure almost always means the binding passes something through wrongly: a format name, a command's arguments, an offset unit, or an error.

The fixtures are plain JSON, so a runner in any language is about a hundred lines.

## Runners

| Binding           | Runner                                                | Command                                                            |
| ----------------- | ----------------------------------------------------- | ------------------------------------------------------------------ |
| Rust              | `crates/canvist_headless/tests/conformance.rs`        | `cargo test -p canvist_headless --all-features --test conformance` |
| TypeScript (Node) | `packages/canvist-headless/tests/conformance.test.ts` | `npm test` in `packages/canvist-headless`                          |
| TypeScript (Deno) | the same file                                         | `deno task test` in `packages/canvist-headless`                    |
| Dart              | `platforms/dart/test/conformance_test.dart`           | `dart test` in `platforms/dart`                                    |

## File shape

Each file in `fixtures/` holds one topic:

```json
{
	"description": "What this file covers.",
	"cases": [
		{
			"name": "toggles bold over a word",
			"requires": [],
			"input": { "format": "markdown", "content": "Hello world" },
			"steps": [
				{
					"op": "select",
					"anchor": { "path": [0], "offset": 6 },
					"head": { "path": [0], "offset": 11 }
				},
				{ "op": "run", "command": "toggleBold", "applied": true }
			],
			"expect": { "markdown": "Hello **world**" }
		}
	]
}
```

- `requires` lists optional features the case needs: `collab`, or `extensions` for a case that uses extensions beyond the StarterKit. A runner without the feature skips the case and reports it as skipped, never as passed.
- `extensions` names extensions to register on top of the StarterKit, such as `["highlight", "textStyle", "color"]`. The names are the extension names in the metadata. Without it, a case runs on the StarterKit alone.
- `input` is optional; without it the case starts from an empty document (one empty paragraph). `format` is `json`, `html`, `markdown`, `text`, or `yjs`. JSON `content` is a ProseMirror document object rather than a string. Yjs `content` is a base64 v1 update joined as the yrs client `clientId`.
- `input.error` instead of `expect` means loading must fail with that [error code](#error-codes).
- Every case uses the StarterKit, plus any `extensions` it lists, with block-id client 1.

## Documents and positions

A position is `{ "path": [...], "offset": n }`:

- `path` walks the block tree from the root. `[2]` is the third top-level block; `[1, 0, 0]` is the paragraph inside the first item of a list that is the second top-level block.
- `offset` counts **UTF-16 code units** into the block's text, the unit of JavaScript and Dart strings. An emoji such as 😀 is 2; an inline embed such as a hard break is 1.

## Expectations

An `expect` object, at the end of a case or in an `expect` step, may hold any of:

| Key         | Compared with                                                                |
| ----------- | ---------------------------------------------------------------------------- |
| `json`      | The ProseMirror JSON export, after [removing block ids](#comparing-json).    |
| `html`      | The HTML export, exactly.                                                    |
| `markdown`  | The Markdown export, exactly.                                                |
| `text`      | The plain-text export, exactly.                                              |
| `selection` | `{ "anchor": position, "head": position }`, or `null` for a block selection. |

### Comparing JSON

Block ids depend on the order blocks were created in, which fixtures should not have to spell out. Before comparing, remove the `id` key from every `attrs` object, then remove every `attrs` object left empty. Apply this to both sides.

## Operations

Every step has an `op`. Any step may carry `"error": "<code>"`, meaning the operation must fail with that code; the document must be unchanged afterwards.

| `op`                | Fields                                                       | Does                                                                                                     |
| ------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `select`            | `anchor`, optional `head`                                    | Select from `anchor` to `head`, or place a caret at `anchor`.                                            |
| `selectAll`         |                                                              | Select the whole document.                                                                               |
| `insertText`        | `text`                                                       | Replace the selection with `text`, without input rules.                                                  |
| `typeText`          | `text`                                                       | Type `text` one character at a time, so input rules such as `##` apply.                                  |
| `deleteSelection`   |                                                              | Delete the selection.                                                                                    |
| `press`             | `key`                                                        | Press a key binding such as `Enter`, `Tab`, or `Mod-b` (`Mod` is Ctrl).                                  |
| `paste`             | `format`, `content`                                          | Paste content in that format over the selection.                                                         |
| `run`               | `command`, optional `args`, optional `applied`               | Run a named command. `applied` is whether it must apply.                                                 |
| `can`               | `command`, optional `args`, `expect`                         | Whether the named command would apply. Nothing changes.                                                  |
| `query`             | `query`, optional `args`, `expect`                           | Answer a named query; `expect` is its JSON result.                                                       |
| `undo`, `redo`      | optional `applied`                                           | Undo or redo one call.                                                                                   |
| `expect`            | any [expectation](#expectations)                             | Check the document now.                                                                                  |
| `snapshotRoundTrip` |                                                              | Take a snapshot, serialize it to JSON text, parse it back, and continue with the restored document.      |
| `share`             | `clientId`                                                   | Share the document as yrs client `clientId`. Requires `collab`.                                          |
| `applyUpdate`       | `update` (base64), optional `changed`                        | Apply a v1 Yjs update; `changed` is whether the document must change. Requires `collab`.                 |
| `receiveSync`       | `message` (base64), optional `reply`, `broadcast`, `changed` | Handle a y-sync message; `reply` and `broadcast` are the number of messages produced. Requires `collab`. |
| `takeUpdates`       | `count`                                                      | Drain this peer's own updates; there must be `count`. Requires `collab`.                                 |
| `validate`          | `rejectedBlocks`, `waiting`                                  | Check the shared state against the schema. Requires `collab`.                                            |
| `rejoin`            | `clientId`                                                   | Encode the shared state and join it again as a new peer, continuing with that peer. Requires `collab`.   |

Arguments (`args`) are a JSON object, or absent for none. Command and query names are the ones in the extension metadata, for example `toggleBold`, `setHeading` with `{ "level": 2 }`, and `isBold`.

## Error codes

`config`, `parse`, `schema`, `unknownName`, `invalidArgs`, `invalidPosition`, `invalidKey`, `editor`, `snapshot`, `notShared`, `alreadyShared`, `collab`, and `render`. `canvist_headless::ErrorCode` documents each one.

## Regenerating the Yjs payloads

`fixtures/collab.json` embeds Yjs updates built with fixed client ids, so every binding reproduces the states they build on:

```sh
cargo test -p canvist_headless --features collab --test yjs_fixtures -- --ignored --nocapture
```
