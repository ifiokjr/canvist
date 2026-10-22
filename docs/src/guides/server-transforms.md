# Server-side transforms

In this guide you change documents in bulk on a server: script edits with typed commands, process many documents in parallel, and build an import endpoint. The engine is the same one the editor uses, so a transformed document is exactly what an editor would have produced.

## Script an edit

Load the document, find what to change, select it, and run a command. This turns every paragraph that starts with `TODO:` into an unchecked task:

```rust
use canvist::headless::Engine;
use canvist::headless::Format;
use canvist::headless::Position;

fn main() -> Result<(), canvist::headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Intro\n\nTODO: write docs\n\nTODO: ship")?;

	let model = doc.document();
	let todos: Vec<u32> = model
		.children(None)
		.iter()
		.enumerate()
		.filter(|&(_, &id)| {
			model
				.inline(id)
				.is_some_and(|text| text.as_str().starts_with("TODO: "))
		})
		.map(|(index, _)| index as u32)
		.collect();

	// Turning a paragraph into a task keeps one top-level block per paragraph,
	// so the indexes collected above stay valid.
	for index in todos {
		doc.select_range(&Position::new([index], 0), &Position::new([index], 6))?;
		doc.delete_selection()?;
		doc.run(&kit.task_list.toggle())?;
	}

	assert_eq!(
		doc.export(Format::Markdown),
		"Intro\n\n- [ ] write docs\n- [ ] ship"
	);
	Ok(())
}
```

## Process many documents in parallel

An `Engine` is `Send + Sync` and cheap to clone, and each `Doc` is `Send`. Build one engine and share it across threads:

```rust
use canvist::headless::Engine;
use canvist::headless::Format;

fn main() -> Result<(), canvist::headless::HeadlessError> {
	let engine = Engine::starter_kit();
	let inputs = ["# One", "# Two", "# Three"];

	let html: Vec<String> = std::thread::scope(|scope| {
		let jobs: Vec<_> = inputs
			.iter()
			.map(|markdown| {
				let engine = &engine;
				scope.spawn(move || {
					engine
						.parse(Format::Markdown, markdown)
						.map(|doc| doc.export(Format::Html))
				})
			})
			.collect();
		jobs.into_iter()
			.map(|job| job.join().expect("a job panicked"))
			.collect::<Result<_, _>>()
	})?;

	assert_eq!(html, ["<h1>One</h1>", "<h1>Two</h1>", "<h1>Three</h1>"]);
	Ok(())
}
```

## Build an import endpoint

An API route that accepts Markdown or HTML and returns clean JSON to store. Build the engine once, outside the handler:

```ts no_run
import { createEngine, isCanvistError } from "@canvist/headless";

const engine = await createEngine();

Deno.serve(async (request) => {
	const format = new URL(request.url).searchParams.get("from") === "html"
		? "html"
		: "markdown";
	try {
		const doc = engine.parse(format, await request.text());
		return Response.json(doc.toJson());
	} catch (error) {
		if (!isCanvistError(error)) throw error;
		return Response.json({ code: error.code, message: error.message }, {
			status: 422,
		});
	}
});
```

## Migrate stored TipTap content

canvist reads TipTap's JSON directly. Load each stored document with `fromJson`, and the `schema` error code tells you which ones use extensions you haven't registered:

```ts
import { createEngine, isCanvistError } from "@canvist/headless";

const engine = await createEngine();
// Rows from your database, as JSON strings.
const rows = [
	'{ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Fine" }] }] }',
	'{ "type": "doc", "content": [{ "type": "mention", "attrs": { "id": "ana" } }] }',
];

for (const row of rows) {
	try {
		const doc = engine.fromJson(JSON.parse(row));
		console.log("ok:", doc.export("text"));
	} catch (error) {
		if (!isCanvistError(error)) throw error;
		console.log("needs work:", error.code);
	}
}
// ok: Fine
// needs work: schema
```

## Next

- [Import and export](./import-export.md): every format and what happens to unknown content.
- [Collaboration](./collaboration.md): transform shared documents and broadcast the change with `flushSync`.
