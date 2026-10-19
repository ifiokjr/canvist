# Commands and typed handles

Read this page to learn the four things you do with an extension: run its commands, check whether they would apply, ask about the selection, and react to keys. Every toolbar button, shortcut, and script is built from these.

## Run a command

A command changes the document at the current selection. Running it returns `true` if it changed something and `false` if it didn't apply, for example toggling a heading inside a code block.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "Hello world");
doc.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });

console.log(doc.commands.toggleBold());
// true
console.log(doc.export("markdown"));
// Hello **world**
```

## Check before you run

`can` runs a command as a dry run. Nothing changes. Use it to enable and disable toolbar buttons.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "- first\n- second");

// The caret is in the first list item, which has no item before it to nest under.
doc.select({ path: [0, 0, 0], offset: 0 });
console.log(doc.can.sinkListItem());
// false

// The second item can nest under the first.
doc.select({ path: [0, 1, 0], offset: 0 });
console.log(doc.can.sinkListItem());
// true
```

## Ask about the selection

Queries answer questions about the selection: is bold active, what heading level is this, what does this link point to. Use them to show active state in your UI.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse(
	"markdown",
	"## Title\n\nSee [the docs](https://example.com)",
);

doc.select({ path: [0], offset: 0 });
console.log(doc.query.headingLevel());
// 2

doc.select({ path: [1], offset: 6 });
console.log(doc.query.linkHref());
// https://example.com
```

## Keys and typing

`press` runs a key through the keyboard shortcuts and `typeText` types characters one by one, running input rules as it goes. That's how a user's keystrokes reach the engine, and it's handy in tests:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.create();

doc.typeText("## Shopping");
doc.press("Enter");
doc.typeText("[ ] milk");

console.log(doc.export("markdown"));
// ## Shopping
//
// - [ ] milk
```

`Mod` means Cmd on Apple platforms and Ctrl elsewhere, so `doc.press("Mod-b")` toggles bold on every platform.

## Typed handles

Commands and queries only exist for extensions you registered. Each language enforces that in its own way:

| Language   | How you reach a command                                                               | Calling an unregistered extension |
| ---------- | ------------------------------------------------------------------------------------- | --------------------------------- |
| TypeScript | `doc.commands.toggleBold()`                                                           | Type error                        |
| Rust       | `doc.run(&bold.toggle())`, with the handle `builder.add(Bold)` returned               | Compile error: there's no handle  |
| Dart       | `doc.run(bold.toggleBold())`, with the handle `builder.add(Extensions.bold)` returned | Compile error: there's no handle  |

Arguments are typed too. A heading level is `1 | 2 | 3 | 4 | 5 | 6` in TypeScript, and the engine checks it again at runtime for code that skips the types.

## Commands by name

When the command comes from data, such as a menu definition, a saved macro, or an AI agent's tool call, run it by name. The engine checks the name and arguments against the registered extensions and fails with a clear error code if they don't match:

```ts
import { createEngine, isCanvistError } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("text", "Title");
doc.selectAll();

console.log(doc.run("setHeading", { level: 1 }));
// true

// A call that arrived as data, such as an agent's tool call.
const call = JSON.parse('{ "name": "setHeading", "args": { "level": 9 } }');
try {
	doc.run(call.name, call.args);
} catch (error) {
	if (!isCanvistError(error)) throw error;
	console.log(error.code);
	// invalidArgs
}
```

In Rust, the same calls are `doc.run_named(name, &args)` and `doc.query(name, &args)`. In Dart, they're `doc.runNamed` and `doc.queryNamed`.

## Next

- [Selections and positions](./selections.md): choose where commands apply.
- [Add a toolbar](../guides/toolbar.md): turn commands, `can`, and queries into buttons.
