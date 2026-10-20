# Integrate canvist with an agent

This guide is for AI coding agents, and the people who work with them. You'll learn the shortest correct path to add canvist to a project on each platform, why canvist's typed API turns an agent's usual mistakes into compile errors, and how to let an AI in your app edit documents safely. It ends with a checklist and the pitfalls agents hit most.

Machine-readable versions of these docs live at [`/llms.txt`](/llms.txt) (an index) and [`/llms-full.txt`](/llms-full.txt) (every page in one file). The [`canvist` agent skill](https://github.com/canvist/canvist/tree/main/skills/canvist) packages this guide for agents that load skills.

## 1. Check what exists before you write code

canvist hasn't had its first release, and several platforms are still coming. Don't guess package names. Use this table:

| Target                         | Install                                                                                                                  | Exists today? |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------ | ------------- |
| Rust                           | `cargo add canvist_headless --git https://github.com/canvist/canvist`                                                    | Yes           |
| Node, Deno, Bun, Workers       | Build `packages/canvist-headless` and install the tarball ([how](../quick-start/server-typescript.md))                   | Yes           |
| Dart                           | Git dependency on `platforms/dart` ([how](../quick-start/server-dart.md))                                                | Yes           |
| Flutter                        | Git dependency on `platforms/flutter`: `CanvistEditor` ([how](../quick-start/flutter.md))                                | Yes           |
| Browser editor                 | Build `packages/canvist-web` and install the tarball ([how](../quick-start/web.md)): `createEditor`                      | Yes           |
| React and Vue                  | `@canvist/react` and `@canvist/vue` over `@canvist/web` ([React](../quick-start/react.md), [Vue](../quick-start/vue.md)) | Yes           |
| Server-side rendering          | `@canvist/headless/ssr` ([how](../quick-start/ssr.md))                                                                   | Yes           |
| Design canvas                  | `@canvist/canvas` ([how](../quick-start/canvas.md))                                                                      | Yes           |
| Desktop: macOS, Windows, Linux | `canvist_desktop` ([how](../quick-start/desktop.md))                                                                     | Yes           |
| iOS                            | `Canvist` Swift package: `CanvistView` ([how](../quick-start/ios.md))                                                    | Yes           |
| Android, React Native, Lynx    | None                                                                                                                     | No            |

If the user asks for a platform marked "No", say so. Offer the closest working path, such as Flutter for mobile or `@canvist/headless` on the server, rather than inventing `@canvist/react-native` or an Android `CanvistView`.

## 2. The minimal integration, per platform

Each path has the same shape: install, build one engine with only the extensions the user asked for, load or create a document, run commands against an explicit selection, export.

### TypeScript

```ts
import { bold, createEngine, heading, link } from "@canvist/headless";

// 1. One engine per process, with only the extensions the product needs.
const engine = await createEngine([bold, heading, link]);

// 2. Load content. JSON for stored documents, Markdown or HTML for imports.
const doc = engine.parse("markdown", "# Welcome\n\nRead the guide");

// 3. Set the selection, then run commands.
doc.select({ path: [1], offset: 9 }, { path: [1], offset: 14 });
doc.commands.setLink({ href: "https://example.com" });

// 4. Export. Store JSON; render HTML.
console.log(doc.export("html"));
// <h1>Welcome</h1><p>Read the <a href="https://example.com">guide</a></p>
```

Verify with `deno check` or `tsc --noEmit`, then run it.

### Rust

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "# Welcome\n\nRead the guide")?;

	doc.select_range(&Position::new([1], 9), &Position::new([1], 14))?;
	doc.run(&kit.link.set("https://example.com"))?;

	assert_eq!(
		doc.export(Format::Html),
		r#"<h1>Welcome</h1><p>Read the <a href="https://example.com">guide</a></p>"#
	);
	Ok(())
}
```

Verify with `cargo build`, then `cargo test`.

### Dart and Flutter

```dart
import 'package:canvist/canvist.dart';

Future<void> main() async {
  final builder = EngineBuilder();
  final link = builder.add(Extensions.link);
  final engine = await builder.build();

  final doc = engine.parse(Format.markdown, 'Read the guide');
  doc.select(const Position([0], 9), const Position([0], 14));
  doc.run(link.setLink(href: 'https://example.com'));

  print(doc.export(Format.markdown));
  // Read the [guide](https://example.com)
  doc.dispose();
  engine.dispose();
}
```

In Flutter, import `package:canvist_flutter/canvist_flutter.dart` instead; the API is the same. Verify with `dart analyze`, then `dart run` (the first run compiles Rust, so allow a few minutes).

## 3. Let the compiler catch your mistakes

The most common agent mistake with editor libraries is calling a feature that isn't set up: running `toggleItalic` on an editor without italic, or passing a heading level of 7. Most editors accept those calls and do nothing. canvist rejects them at compile time.

```ts
import { bold, createEngine } from "@canvist/headless";

const engine = await createEngine([bold]);
const doc = engine.create();

function mistakes() {
	// @ts-expect-error: Property 'toggleItalic' does not exist. Italic isn't registered.
	doc.commands.toggleItalic();
	// @ts-expect-error: Property 'setHeading' does not exist. Neither is heading.
	doc.commands.setHeading({ level: 2 });
}
```

When you see `Property 'toggleX' does not exist on type 'Commands<…>'`, the fix is to register the extension in `createEngine([...])`, not to cast. The same holds in Rust and Dart, where commands only exist on the handle that registering an extension returns.

**Never silence these errors** with `as any`, `// @ts-ignore`, or `dynamic`. They are the point. The one place untyped calls belong is data that arrives at runtime, such as a model's tool call, and there the engine checks every call instead (section 4).

## 4. Let an AI in your app edit documents

To give a model in your product the power to edit documents, expose each registered command as a tool. The metadata the types come from also describes every command's arguments, so you can generate tool definitions instead of writing them, and the engine checks every call the model makes:

```ts
import {
	bold,
	createEngine,
	heading,
	isCanvistError,
	type ValueSpec,
} from "@canvist/headless";

const engine = await createEngine([bold, heading]);

/** A JSON Schema for one argument type. */
function schema(type: ValueSpec): Record<string, unknown> {
	switch (type.type) {
		case "bool":
			return { type: "boolean" };
		case "int":
			return { type: "integer", minimum: type.min, maximum: type.max };
		case "float":
			return { type: "number" };
		case "string":
			return type.enum
				? { type: "string", enum: type.enum }
				: { type: "string" };
		case "nullable":
			return { anyOf: [schema(type.of), { type: "null" }] };
		case "list":
			return { type: "array", items: schema(type.of) };
		case "record":
			return {
				type: "object",
				properties: Object.fromEntries(
					type.fields.map((field) => [field.name, schema(field.type)]),
				),
			};
	}
}

// One tool per command the engine registered. `metadata()` carries the descriptions.
const tools = engine.metadata().flatMap((extension) =>
	extension.commands.map((command) => ({
		name: command.name,
		description: command.doc ?? command.name,
		input_schema: {
			type: "object",
			properties: Object.fromEntries(
				command.args.map((arg) => [arg.name, schema(arg.type)]),
			),
			required: command.args.map((arg) => arg.name),
		},
	}))
);
console.log(tools.map((tool) => tool.name).join(", "));
// setParagraph, toggleHeading, setHeading, toggleBold, setBold, unsetBold

// Run a tool call from the model. It arrives as JSON, so the engine checks it at runtime,
// and errors go back to the model as text it can act on.
const doc = engine.parse("text", "Quarterly report");
doc.selectAll();

function runTool(call: string): string {
	const { name, input } = JSON.parse(call);
	try {
		return doc.run(name, input)
			? "done"
			: "the command did not apply at this selection";
	} catch (error) {
		if (!isCanvistError(error)) throw error;
		return `error ${error.code}: ${error.message}`;
	}
}

console.log(runTool('{ "name": "setHeading", "input": { "level": 1 } }'));
// done
console.log(runTool('{ "name": "setHeading", "input": { "level": 9 } }'));
// error invalidArgs: invalid arguments for `setHeading`: `level` must be an integer from 1 to 6
console.log(runTool('{ "name": "toggleItalic", "input": {} }'));
// error unknownName: no command or query is named `toggleItalic`
```

Give the model the document as Markdown or JSON, and the positions it needs, so it can call `select` before each command. Validate the result with `doc.export("json")` before you save it.

## 5. Checklist

- [ ] The platform exists today (section 1). If not, you told the user and chose the closest working path.
- [ ] Installed from GitHub, not a registry, until the first release.
- [ ] One engine per process, created at startup and shared.
- [ ] Only the extensions the user asked for are registered.
- [ ] Every command runs after an explicit `select`.
- [ ] Positions use block paths and UTF-16 offsets.
- [ ] Stored documents are JSON; imports go through `parse("markdown" | "html", …)`.
- [ ] Errors are narrowed with `isCanvistError` (TypeScript), `HeadlessError::code()` (Rust), or `CanvistException.code` (Dart), and handled by code.
- [ ] Collaborating peers have distinct client ids, and the server seeds shared documents.
- [ ] Type checks and tests pass with no casts or ignores around canvist calls.

## 6. Common pitfalls

- **Inventing packages.** `@canvist/core`, `@canvist/ssr`, an Android `CanvistView`, and `@canvist/react-native` don't exist yet. Check section 1.
- **Byte offsets.** Offsets count UTF-16 code units in every language, including Rust. An emoji is two units. Convert with `text[..byte].encode_utf16().count()` in Rust.
- **Flat positions in nested blocks.** Text inside a list item lives at `[list, item, 0]`, not `[list]`. Selecting a container fails with `invalidPosition`.
- **Running commands without a selection.** Commands apply to the selection. A new document's selection is at its start, which is rarely what you want.
- **Loading foreign JSON without the right extensions.** `fromJson` fails with `schema` if the JSON has blocks you didn't register. Register them, or import through HTML or Markdown, which never fail.
- **Expecting Markdown to keep everything.** Markdown has no underline, so exporting drops it. Store JSON.
- **Rust engines without the basics.** `Engine::new` registers exactly what you add. Add `Document` and `Paragraph`, and `BaseKeymap` for editing keys, or use `Engine::starter_kit()`.
- **Dart binaries that can't find the engine.** Use `dart build cli`, not `dart compile exe`.
- **Forgetting `dispose()` in Dart.** Documents and engines hold native memory. Dispose them in `State.dispose`.
- **Two peers with the same client id.** Their edits collide. Give every peer its own.

## Where the skill's evaluations will live

The `canvist` skill in [`skills/canvist`](https://github.com/canvist/canvist/tree/main/skills/canvist) will get an evaluation suite in `skills/canvist/evals/`: real integration tasks that score whether an agent's code compiles, uses only registered extensions, and passes the task's tests.
