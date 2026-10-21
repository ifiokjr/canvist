# Quick start: Desktop

In this guide you open a native canvist editor window on macOS, Windows, or Linux, run typed commands from your own code, and embed the editor in a winit app with its own toolbar. It takes about five minutes.

**Status: available.** `canvist_desktop` works on macOS, Windows, and Linux, and its tests run on all three on every change. It goes to crates.io with the first release; until then, install it from GitHub.

The editor is a Rust component on [winit](https://github.com/rust-windowing/winit), painted with [vello](https://github.com/linebender/vello) on the GPU and laid out by the same engine as canvist's server-side renderer, so a document wraps identically in the window, in a PNG, and in server-rendered HTML. It types, selects, copies, and pastes like a native text view, takes input methods (with the candidate window under the text), uses Cmd shortcuts on macOS and Ctrl elsewhere, and exposes its text to screen readers through [AccessKit](https://accesskit.dev).

## 1. Install

You need Rust 1.95 or newer. On Linux, a desktop with X11 or Wayland and Vulkan or OpenGL drivers.

```sh
cargo add canvist_desktop canvist_headless --git https://github.com/canvist/canvist
```

| Feature           | Adds                                                                                    |
| ----------------- | --------------------------------------------------------------------------------------- |
| `winit` (default) | The window host: GPU rendering, the system clipboard, IME, and the accessibility bridge |
| `collab`          | Live collaboration with a y-websocket or Hocuspocus server                              |

Without `winit`, the crate is the headless editor core, which still lays out, renders PNGs, and builds the accessibility tree, for tests and servers.

## 2. Open an editor window

Replace `src/main.rs` with this:

```rust no_run
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::winit::winit::window::Window;
use canvist_headless::Engine;
use canvist_headless::Format;

fn main() -> Result<(), canvist_desktop::DesktopError> {
	let options = EditorOptions::new().max_width(720.0).label("Notes");
	let mut editor = DesktopEditor::new(Engine::starter_kit(), options)?;
	editor.load(Format::Markdown, "# Hello\n\nStart typing.")?;

	canvist_desktop::winit::run(editor, Window::default_attributes().with_title("Notes"))
}
```

Run it:

```sh
cargo run
```

Type `##` to make a heading, `-` for a list, and `[ ]` for a task. Cmd-B (Ctrl-B on Windows and Linux) makes text bold, and Cmd-Z undoes.

## 3. Run typed commands

Commands come from the engine's typed handles, so commands of extensions you didn't register don't compile. Every change goes through the editor, so undo and layout follow it:

```rust
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::FontSource;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_extensions::HeadingLevel;

fn main() -> Result<(), canvist_desktop::DesktopError> {
	let options = EditorOptions::new().fonts(FontSource::Bundled);
	let mut editor = DesktopEditor::new(Engine::starter_kit(), options)?;
	let kit = editor.handles();

	editor.load(Format::Markdown, "Release notes")?;
	editor.run(&kit.heading.toggle(HeadingLevel::H2))?;

	assert_eq!(kit.heading.level(editor.state()), Some(HeadingLevel::H2));
	assert_eq!(editor.export(Format::Markdown), "## Release notes");
	Ok(())
}
```

`FontSource::Bundled` uses only the bundled Atkinson Hyperlegible font, so output is the same on every machine. Apps use the default, `FontSource::System`, which reads the fonts installed on the computer. Add your own with `editor.register_font(bytes)`.

## 4. Embed it in your winit app

`EditorWindow` puts the editor in any winit app. Forward the window's events to it, draw your own chrome into the same vello scene, and place the editor where you want it, such as below a toolbar:

```rust no_run
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::kurbo::Rect;
use canvist_desktop::winit::EditorWindow;
use canvist_desktop::winit::winit;
use canvist_headless::Engine;
use canvist_headless::canvist_extensions::StarterKitHandles;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowId;

#[derive(Default)]
struct App {
	window: Option<EditorWindow<StarterKitHandles>>,
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let editor =
			DesktopEditor::new(Engine::starter_kit(), EditorOptions::new()).expect("an editor");
		let mut window =
			EditorWindow::new(event_loop, Window::default_attributes(), editor).expect("a window");
		// Leave 48 logical pixels at the top for a toolbar.
		window.set_editor_bounds(Some(Rect::new(0.0, 48.0, 1024.0, 768.0)));
		self.window = Some(window);
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
		let Some(window) = self.window.as_mut() else {
			return;
		};

		match event {
			WindowEvent::CloseRequested => event_loop.exit(),
			WindowEvent::RedrawRequested => {
				window
					.render(|scene, scale| {
						// Draw your toolbar into `scene` here, scaled by `scale`.
						let _ = (scene, scale);
					})
					.expect("a frame");
			}
			event => {
				let response = window.window_event(&event).expect("an event");

				if !response.consumed {
					// The editor didn't use it: handle your own shortcuts here.
				}
			}
		}
	}

	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		if let Some(window) = &self.window {
			window.about_to_wait(event_loop);
		}
	}
}

fn main() {
	let event_loop = EventLoop::new().expect("an event loop");
	event_loop
		.run_app(&mut App::default())
		.expect("the app ran");
}
```

`window.editor_mut()` reaches the editor for commands, such as a toolbar button's `editor.run(&kit.bold.toggle())`. Call `window.request_redraw()` afterwards.

Using another windowing library? Drive `DesktopEditor` directly: translate its events into `canvist_desktop::InputEvent`s, call `frame` and `paint` with any `canvist_paint` painter, and put the IME candidate window at `ime_area()`.

## 5. Try the example app

[`examples/desktop`](https://github.com/canvist/canvist/tree/main/examples/desktop) is a small Markdown editor with a toolbar, open and save, and live collaboration:

```sh
git clone https://github.com/canvist/canvist && cd canvist
cargo run -p canvist_desktop_example -- notes.md
```

To collaborate, start a y-websocket server and open the same room in two windows:

```sh
npx y-websocket
```

```sh
cargo run -p canvist_desktop_example -- --collab ws://localhost:1234/notes
```

## Next steps

- [Collaboration](../guides/collaboration.md): how documents merge, and how to run your own server.
- [Fonts and text rendering](../guides/fonts.md): font stacks and loading fonts.
- [Theming](../guides/theming.md): restyle blocks, marks, the caret, and the selection.
- [Performance](../guides/performance.md): the desktop budgets, such as keystroke to frame.
