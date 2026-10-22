---
name: canvist
description: Integrate canvist, the plugin-based rich-text editor with a Rust core, into an app. Use when a task mentions canvist, @canvist/headless, canvist_headless, canvist_flutter, or asks to add a rich-text editor, convert between ProseMirror/TipTap JSON, HTML, and Markdown, run editor commands on a server, or sync documents with Yjs using canvist.
---

# canvist

canvist is a rich-text editor you compose from extensions (bold, headings, lists, tasks, links, …). One Rust core runs everywhere; each platform gets a thin binding. Its APIs are typed by the extensions you register, so calling an unregistered extension is a compile error. Lean on that: it is how you avoid the most common editor-integration bugs.

Full docs for agents: `/llms-full.txt` on the docs site, or `docs/src` in https://github.com/canvist/canvist. The human-readable version of this skill is `docs/src/agents/readme.md`.

## 1. Check the platform exists

canvist has not had its first release. Nothing is on crates.io, npm, JSR, or pub.dev yet: install from GitHub.

| Target                   | Package             | Install                                                                                                                    |
| ------------------------ | ------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Rust                     | `canvist_headless`  | `cargo add canvist_headless --git https://github.com/canvist/canvist`                                                      |
| Node, Deno, Bun, Workers | `@canvist/headless` | Clone, then in `packages/canvist-headless`: `npm ci && npm run build:wasm && npm pack`; install the tarball                |
| Dart                     | `canvist`           | pubspec git dependency: `url: https://github.com/canvist/canvist`, `path: platforms/dart`                                  |
| Flutter                  | `canvist_flutter`   | pubspec git dependency with `path: platforms/flutter`. `CanvistEditor` plus a `CanvistController`                          |
| Browser editor           | `@canvist/web`      | Clone, build and pack `packages/canvist-headless` as above, then the same in `packages/canvist-web`; install both tarballs |

These **do not exist yet**: `@canvist/core`, `@canvist/react`, `@canvist/vue`, `@canvist/ssr`, `@canvist/react-native`, `@canvist/lynx`, a native iOS or Android `CanvistView`, `canvist_desktop`. If asked for one, say it is coming soon and offer the closest working path (Flutter for mobile, `@canvist/headless` on the server, `@canvist/web` in the browser).

## 2. Integrate

Always the same shape: one engine with only the needed extensions, load or create a document, `select`, run commands, export.

```ts
import { bold, createEngine, heading, link } from "@canvist/headless";

const engine = await createEngine([bold, heading, link]); // once per process
const doc = engine.parse("markdown", "# Welcome\n\nRead the guide");

doc.select({ path: [1], offset: 9 }, { path: [1], offset: 14 });
doc.commands.setLink({ href: "https://example.com" });

console.log(doc.export("html"));
// <h1>Welcome</h1><p>Read the <a href="https://example.com">guide</a></p>
```

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Hello world")?;

	doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
	doc.run(&kit.bold.toggle())?;

	assert_eq!(doc.export(Format::Markdown), "Hello **world**");
	Ok(())
}
```

```dart
import 'package:canvist/canvist.dart';

Future<void> main() async {
  final (:engine, :kit) = await Engine.starterKit();
  final doc = engine.parse(Format.markdown, 'Hello world');

  doc.select(const Position([0], 6), const Position([0], 11));
  doc.run(kit.bold.toggleBold());

  print(doc.export(Format.markdown));
  // Hello **world**
  doc.dispose();
}
```

Key API, same in every language (Rust is snake_case):

- Engine: `createEngine([...])` / `Engine::new(|b| …)` / `EngineBuilder().add(...)`; `parse(format, text)`, `fromJson(json)`, `create()`, `fromYjs(state, clientId)`, `restore(snapshot)`.
- Doc: `select(anchor, head?)`, `selectAll()`, `commands.X()` / `run(handle.x())`, `can.X()`, `query.X()`, `insertText`, `typeText`, `press("Mod-b")`, `paste(format, text)`, `undo`, `redo`, `export(format)`, `toJson()`, `snapshot()`.
- Collaboration: `share(clientId)`, `encodeState()`, `applyUpdate(u)`, `takeUpdates()`, `syncStart()`, `receiveSync(msg)` → `{ reply, broadcast }`, `flushSync()`, `validate()`.
- Formats: `json` (ProseMirror/TipTap JSON; must fit the schema), `html` and `markdown` (never fail; unknown content falls back to paragraphs), `text`.
- Extensions (TypeScript names): `heading`, `blockquote`, `codeBlock`, `hardBreak`, `horizontalRule`, `bulletList`, `orderedList`, `listItem`, `taskList`, `bold`, `italic`, `underline`, `strike`, `code`, `link`, `history`, `trailingNode`. `createEngine()` with no list is the whole StarterKit. Commands follow TipTap names: `toggleBold`, `setHeading({ level })`, `setLink({ href })`, `toggleTaskList`, `sinkListItem`, …

## 3. Verify

- TypeScript: `deno check` or `tsc --noEmit`, then run.
- Rust: `cargo build`, then `cargo test`.
- Dart: `dart analyze`, then `dart run` (the first run compiles Rust; allow minutes).

If the compiler says `Property 'toggleX' does not exist on type 'Commands<…>'`, register the extension. **Never** cast with `as any`, `@ts-ignore`, or `dynamic` to get past it. Untyped calls are only for data arriving at runtime (a model's tool call parsed from JSON); the engine validates those and throws `CanvistError` with a code.

## 4. Checklist

- [ ] The platform exists (section 1), or you told the user it doesn't.
- [ ] Installed from GitHub.
- [ ] One engine, created once, with only the requested extensions.
- [ ] Every command runs after an explicit `select`.
- [ ] Positions are `{ path, offset }`: block path, UTF-16 offset.
- [ ] Stored documents are JSON.
- [ ] Errors handled by code: `isCanvistError(e) && e.code` (TS), `e.code()` (Rust), `e.code` (Dart).
- [ ] Collaborating peers use distinct client ids; the server seeds and `share`s documents.
- [ ] Type check and tests pass with no casts around canvist calls.

## 5. Pitfalls

- Offsets are UTF-16 code units in every language, Rust included. Emoji count as two.
- Text in a list item is at `[list, item, 0]`. Selecting a container path fails with `invalidPosition`.
- `fromJson` fails with `schema` for unregistered blocks. Register them, or import via HTML/Markdown.
- Markdown export drops underline. Store JSON.
- Rust `Engine::new` registers only what you add: include `Document`, `Paragraph`, and `BaseKeymap`, or use `Engine::starter_kit()`.
- Dart: `dispose()` docs and engines; build binaries with `dart build cli`, not `dart compile exe`.
- Custom extensions exist only in Rust; TypeScript and Dart register StarterKit extensions by name.

## Evaluations

The eval suite for this skill will live in `skills/canvist/evals/`: integration tasks scored on whether the agent's code compiles, uses only registered extensions, and passes the task's tests.
