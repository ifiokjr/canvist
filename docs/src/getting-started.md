# Getting Started

## Prerequisites

canvist uses [devenv](https://devenv.sh/) for a reproducible development environment. Follow the [getting started instructions](https://devenv.sh/getting-started/) to install it.

## Setup

```bash
# Clone the repository.
git clone https://github.com/ifiokjr/canvist.git
cd canvist

# Allow direnv to load the environment.
direnv allow .

# Install cargo binary tools.
install:all
```

## Quick start: Rust API

```rust
use canvist_core::{Document, EditorRuntime, EditorEvent, Position, Selection, Style};

// Create a runtime with an empty document.
let mut runtime = EditorRuntime::new(
    Document::new(),
    Selection::collapsed(Position::zero()),
    "user:demo",
);

// Insert text.
runtime.handle_event(EditorEvent::TextInsert {
    text: "Hello, canvist!".to_string(),
}).unwrap();

assert_eq!(runtime.document().plain_text(), "Hello, canvist!");

// Apply bold to "Hello".
runtime.apply_operation(canvist_core::operation::Operation::format(
    Selection::range(Position::new(0), Position::new(5)),
    Style::new().bold(),
));

// Export.
let html = runtime.document().to_html();
let md = runtime.document().to_markdown();
let json = runtime.document().to_json().unwrap();
```

## Quick start: TypeScript / Browser

```ts
import { createEditor } from "@canvist/canvist";

const editor = await createEditor("my-canvas");
editor.insertText("Hello, canvist!");
editor.render();

// Format selected text.
editor.setSelection(0, 5);
editor.toggleBold();
editor.render();

// Export.
console.log(editor.toHtml());
console.log(editor.toMarkdown());
```

## Quick start: Collaboration

```ts
const editor = await createEditor("canvas");
editor.enableCollab();

// Send your state to peers.
const state = editor.collabEncodeState();
sendToPeer(state);

// Apply updates from peers.
onPeerUpdate((update) => {
  editor.collabApplyUpdate(update);
  editor.render();
});
```

## Development commands

| Command           | Description                    |
| ----------------- | ------------------------------ |
| `build:all`       | Build all crates               |
| `build:wasm`      | Build the WASM package         |
| `build:book`      | Build the mdbook documentation |
| `test:all`        | Run all tests                  |
| `test:cargo`      | Run cargo tests with nextest   |
| `test:docs`       | Run documentation tests        |
| `test:playwright` | Run Playwright browser tests   |
| `fix:all`         | Auto-fix lints and formatting  |
| `fix:format`      | Format with dprint             |
| `fix:clippy`      | Fix clippy lints               |
| `lint:all`        | Run all linters                |
| `lint:format`     | Check formatting               |
| `lint:clippy`     | Check clippy lints             |
| `deny:check`      | Run cargo-deny security checks |
| `coverage:all`    | Generate code coverage         |

## Building for the web

```bash
# Build the WASM package.
build:wasm

# Serve the demo.
cd packages/canvist
deno run --allow-read --allow-net -c deno.json - <<'EOF'
import { serveDir } from "jsr:@std/http/file-server";
Deno.serve({ port: 8080 }, (req) => serveDir(req, { fsRoot: "." }));
EOF

# Open http://localhost:8080/demo/
```

## Project structure

```
canvist/
├── crates/
│   ├── canvist/          # Umbrella re-export crate
│   ├── canvist_core/     # Document model, runtime, operations, CRDT
│   ├── canvist_render/   # Platform-agnostic rendering traits
│   ├── canvist_wasm/     # WebAssembly + Canvas2D backend
│   └── canvist_test/     # Playwright integration tests
├── packages/
│   └── canvist/          # TypeScript/Deno package
│       ├── demo/         # Demo HTML pages (editor + collab)
│       ├── src/          # TypeScript wrapper + types
│       └── tests/        # Playwright + unit tests
└── docs/                 # mdbook documentation
```
