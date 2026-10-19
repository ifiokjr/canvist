# Build your own editor

In this guide you grow one editor in four steps: a plain chat input, a rich-text comment box, a structured notes editor, and a collaborative Google Docs-style editor. Then you write an extension of your own. Stop at whichever step your app needs.

Each step only changes the list of extensions. Your toolbar code and stored documents keep working as you go.

## Step 1: a plain chat input

Register nothing. Every engine has paragraphs and the standard editing keys, which is all a chat input needs:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine([]);
const doc = engine.parse("markdown", "# Hi **there**");

// Formatting the engine doesn't know becomes plain text.
console.log(doc.export("text"));
// Hi there
```

## Step 2: a rich-text comment box

Add the marks people expect in a comment, plus undo:

```ts
import { bold, code, createEngine, history, italic, link } from "@canvist/headless";

const engine = await createEngine([bold, italic, code, link, history]);
const doc = engine.create();

doc.typeText("Looks **good**, see https://canvist.dev ");
console.log(doc.export("markdown"));
// Looks **good**, see [https://canvist.dev](https://canvist.dev)
```

Typing `**good**` made it bold, and the URL became a link when the space followed it.

## Step 3: a structured notes editor

Add headings, lists, tasks, quotes, and code blocks. That's the StarterKit, so you can register it in one call:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine(); // the StarterKit
const doc = engine.create();

doc.typeText("## Launch");
doc.press("Enter");
doc.typeText("[ ] Write the docs");
doc.press("Enter");
doc.typeText("Ship it");

console.log(doc.export("markdown"));
// ## Launch
//
// - [ ] Write the docs
// - [ ] Ship it
```

To leave a few StarterKit extensions out, list the ones you want instead. In Rust, set fields of `StarterKit` to `false`, or turn off their Cargo features so they aren't compiled.

## Step 4: a Google Docs-style editor

A Google Docs-style editor adds people editing together. Share the document, and exchange Yjs updates between peers through any Yjs server, or [canvist Cloud](../cloud/readme.md). This example connects two peers directly, so you can run it on its own:

```ts
import { createEngine } from "@canvist/headless";

const alice = await createEngine(undefined, { client: 1 });
const doc = alice.parse("markdown", "# Minutes");
doc.share(1);

// Bob joins from Alice's state with his own client id.
const bob = alice.fromYjs(doc.encodeState(), 2);
bob.select({ path: [0], offset: 7 });
bob.insertText(" for Monday");

// Bob's changes travel as Yjs updates.
for (const update of bob.takeUpdates()) doc.applyUpdate(update);
console.log(doc.export("markdown"));
// # Minutes for Monday
```

In a real app, each peer sends `takeUpdates()` to the server and applies what arrives. [Collaboration](../guides/collaboration.md) shows the full setup with y-websocket, Hocuspocus, and canvist Cloud.

What else a Google Docs-style editor needs, and where it stands:

| Feature                         | Status                                                                 |
| ------------------------------- | ---------------------------------------------------------------------- |
| Headings, lists, tasks, quotes  | Available: the StarterKit                                              |
| Real-time collaboration         | Available: Yjs, y-websocket, Hocuspocus, canvist Cloud Sync            |
| Server-rendered previews        | Available in Rust: [server-side rendering](../quick-start/ssr.md)     |
| Import and export               | Available: JSON, HTML, Markdown, and text; DOCX and PDF later         |
| Live carets of other people     | Coming with the editor hosts                                           |
| Tables and images               | Coming soon                                                            |
| Comments and suggestions        | Coming soon                                                            |
| Version history                 | Coming soon, as canvist Cloud History                                  |

## Add your own extension

When the catalogue doesn't have what you need, write an extension in Rust. This one inserts a signature. It adds a command, a keyboard shortcut, and a typed handle, so only engines that registered it can call it:

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_editor::BaseKeymap;
use canvist_headless::canvist_editor::Binding;
use canvist_headless::canvist_editor::Command;
use canvist_headless::canvist_editor::CommandSpec;
use canvist_headless::canvist_editor::EditorError;
use canvist_headless::canvist_editor::Extension;
use canvist_headless::canvist_editor::Platform;
use canvist_headless::canvist_editor::Registration;
use canvist_headless::canvist_editor::TypedExtension;
use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_extensions::Document;
use canvist_headless::canvist_extensions::Paragraph;
use canvist_headless::canvist_model::Schema;

/// Inserts "— Ada" at the caret, with `Mod-Alt-s`.
struct Signature;

/// Typed access to [`Signature`]: only a registered extension has one.
#[derive(Clone, Copy)]
struct SignatureHandle {
	_registration: Registration,
}

impl SignatureHandle {
	fn insert(self) -> Command {
		insert_signature()
	}
}

fn insert_signature() -> Command {
	command(|state| commands::insert_text(state, "— Ada"))
}

impl Extension for Signature {
	fn name(&self) -> &'static str {
		"signature"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"insertSignature",
			"Insert the author's signature at the caret.",
			insert_signature(),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Alt-s", insert_signature())])
	}
}

impl TypedExtension for Signature {
	type Handle = SignatureHandle;

	fn handle(&self, registration: Registration) -> SignatureHandle {
		SignatureHandle { _registration: registration }
	}
}

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let engine = Engine::new(|builder| {
		builder.add(Document);
		builder.add(Paragraph);
		builder.add(BaseKeymap); // Enter, Backspace, arrows, and the other standard keys
		builder.add(Signature)
	})?;
	let signature = engine.handles();
	let mut doc = engine.create()?;

	doc.insert_text("Thanks! ")?;
	doc.run(&signature.insert())?; // through the typed handle
	doc.press("Enter")?;
	doc.press("Mod-Alt-s")?; // through the shortcut
	doc.run_named("insertSignature", &serde_json::json!({}))?; // by name

	assert_eq!(doc.export(Format::Text), "Thanks! — Ada\n— Ada— Ada");
	Ok(())
}
```

An extension can also add blocks, marks, and embeds with `Extension::schema`, plus queries, input rules, and paste rules. The StarterKit's own extensions in `canvist_extensions` are good models to copy.

## Next

- [Add a toolbar](../guides/toolbar.md) for the editor you just built.
- [Extension catalogue](./readme.md): every extension and its commands.
