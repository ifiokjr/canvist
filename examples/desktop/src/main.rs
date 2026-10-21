//! canvist notes: a small Markdown editor on `canvist_desktop`.
//!
//! ```sh
//! cargo run -p canvist_desktop_example -- notes.md
//! cargo run -p canvist_desktop_example -- --collab ws://localhost:1234/notes
//! ```
//!
//! The toolbar runs the editor's typed StarterKit commands. Cmd-O and Cmd-S
//! (Ctrl elsewhere) open and save Markdown. With `--collab`, the document is
//! the y-websocket room's, shared live with every other client in it.

mod network;
mod toolbar;

use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use canvist_desktop::DesktopEditor;
use canvist_desktop::DesktopError;
use canvist_desktop::EditorOptions;
use canvist_desktop::InputEvent;
use canvist_desktop::Modifiers;
use canvist_desktop::current_platform;
use canvist_desktop::kurbo::Point;
use canvist_desktop::kurbo::Rect;
use canvist_desktop::winit::EditorWindow;
pub use canvist_desktop::winit::winit;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_core::Platform;
use canvist_headless::canvist_extensions::StarterKitHandles;
use winit::application::ApplicationHandler;
use winit::event::ElementState;
use winit::event::MouseButton;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;
use winit::window::WindowId;

use crate::network::Connection;
use crate::toolbar::Tool;
use crate::toolbar::Toolbar;

/// The document column's widest, and the space around it.
const COLUMN: f32 = 720.0;
const PADDING: f32 = 40.0;
/// The window's first size, in logical pixels.
const WINDOW: (f64, f64) = (1_040.0, 760.0);
/// Yjs client ids are 53-bit, so JavaScript peers read them exactly.
const CLIENT_ID_BITS: u32 = 53;

/// What the network thread tells the window.
#[derive(Debug)]
pub enum AppEvent {
	Connected,
	Sync(Vec<u8>),
	Disconnected(String),
}

/// What the command line asked for.
#[derive(Debug, Default)]
struct Args {
	file: Option<PathBuf>,
	collab: Option<String>,
}

fn args() -> Result<Args, String> {
	let mut args = Args::default();
	let mut words = std::env::args().skip(1);

	while let Some(word) = words.next() {
		match word.as_str() {
			"--collab" => args.collab = Some(words.next().ok_or("--collab needs a ws:// URL")?),
			"--help" | "-h" => {
				return Err(
					"usage: canvist-notes [FILE.md] [--collab ws://HOST:PORT/ROOM]".to_owned(),
				);
			}
			_ => args.file = Some(PathBuf::from(word)),
		}
	}

	Ok(args)
}

/// The collaboration session, once a server URL was given.
struct Collab {
	connection: Connection,
	client_id: u64,
	status: String,
}

struct App {
	args: Args,
	proxy: EventLoopProxy<AppEvent>,
	window: Option<EditorWindow<StarterKitHandles>>,
	toolbar: Toolbar,
	file: Option<PathBuf>,
	/// The document version last saved or opened.
	saved: u64,
	collab: Option<Collab>,
	modifiers: Modifiers,
	pointer: Option<Point>,
	error: Option<DesktopError>,
}

fn main() {
	let args = match args() {
		Ok(args) => args,
		Err(usage) => {
			eprintln!("{usage}");
			std::process::exit(2);
		}
	};

	let event_loop = match EventLoop::<AppEvent>::with_user_event().build() {
		Ok(event_loop) => event_loop,
		Err(error) => {
			eprintln!("canvist-notes could not start: {error}");
			std::process::exit(1);
		}
	};

	let mut app = App {
		proxy: event_loop.create_proxy(),
		args,
		window: None,
		toolbar: Toolbar::new(),
		file: None,
		saved: 0,
		collab: None,
		modifiers: Modifiers::NONE,
		pointer: None,
		error: None,
	};

	if let Err(error) = event_loop.run_app(&mut app) {
		eprintln!("canvist-notes stopped: {error}");
		std::process::exit(1);
	}

	if let Some(error) = app.error {
		eprintln!("canvist-notes stopped: {error}");
		std::process::exit(1);
	}
}

impl App {
	fn open_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), DesktopError> {
		let options = EditorOptions::new().max_width(COLUMN).padding(PADDING);
		let mut editor = DesktopEditor::new(Engine::starter_kit(), options)?;

		if let Some(path) = self.args.file.clone() {
			let markdown = std::fs::read_to_string(&path).unwrap_or_default();
			editor.load(Format::Markdown, &markdown)?;
			self.file = Some(path);
		}

		self.saved = editor.doc().document().version();
		let attributes = Window::default_attributes()
			.with_title("canvist notes")
			.with_inner_size(winit::dpi::LogicalSize::new(WINDOW.0, WINDOW.1));
		let mut window = EditorWindow::new(event_loop, attributes, editor)?;
		window.set_editor_bounds(Some(editor_bounds(&window)));
		let _ = window.handle(&InputEvent::Focus(true))?;
		self.window = Some(window);

		if let Some(url) = self.args.collab.clone() {
			self.collab = Some(Collab {
				connection: Connection::open(url.clone(), self.proxy.clone()),
				client_id: client_id(),
				status: "Connecting…".to_owned(),
			});
		}

		self.update_title();
		Ok(())
	}

	/// Handle a window event. The toolbar takes clicks on its buttons and
	/// follows the pointer for hover; everything else goes to the editor,
	/// and keys the editor did not use go to the app's shortcuts.
	fn window_event(&mut self, event: &WindowEvent) -> Result<(), DesktopError> {
		match event {
			WindowEvent::RedrawRequested => return self.render(),
			WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
				return self.resize(event);
			}
			WindowEvent::ModifiersChanged(modifiers) => self.modifiers = held(modifiers.state()),
			WindowEvent::CursorMoved { position, .. } => self.hover(*position),
			WindowEvent::MouseInput {
				state: ElementState::Pressed,
				button: MouseButton::Left,
				..
			} => {
				let tool = self.pointer.and_then(|point| self.toolbar.tool_at(point));

				if let Some(tool) = tool {
					return self.use_tool(tool);
				}
			}
			_ => {}
		}

		self.forward(event)
	}

	/// Give `event` to the editor, then run the app's shortcut for a key the
	/// editor did not use.
	fn forward(&mut self, event: &WindowEvent) -> Result<(), DesktopError> {
		let Some(window) = self.window.as_mut() else {
			return Ok(());
		};
		let response = window.window_event(event)?;

		let unused = match event {
			WindowEvent::KeyboardInput { event: key, .. }
				if !response.consumed && key.state == ElementState::Pressed =>
			{
				Some(key.logical_key.clone())
			}
			_ => None,
		};

		if let Some(key) = unused {
			self.shortcut(&key)?;
		}

		self.after_input();
		Ok(())
	}

	/// Keep the editor below the toolbar as the window changes size.
	fn resize(&mut self, event: &WindowEvent) -> Result<(), DesktopError> {
		let Some(window) = self.window.as_mut() else {
			return Ok(());
		};
		let _ = window.window_event(event)?;
		window.set_editor_bounds(Some(editor_bounds(window)));
		Ok(())
	}

	/// Follow the pointer, in logical pixels, for toolbar hover and clicks.
	fn hover(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
		let Some(window) = self.window.as_ref() else {
			return;
		};
		let point = position.to_logical::<f64>(window.window().scale_factor());
		let point = Point::new(point.x, point.y);
		self.pointer = Some(point);
		self.toolbar.hover = Some(point);
		window.request_redraw();
	}

	/// The app's own shortcuts, for keys the editor did not use.
	fn shortcut(&mut self, key: &winit::keyboard::Key) -> Result<(), DesktopError> {
		let held = self.modifiers;

		let is_mod = match current_platform() {
			Platform::Apple => held.meta,
			Platform::Other => held.ctrl,
		};

		let winit::keyboard::Key::Character(character) = key else {
			return Ok(());
		};

		match (is_mod, character.to_lowercase().as_str(), held.shift) {
			(true, "o", _) => self.open(),
			(true, "s", false) => {
				self.save();
				Ok(())
			}
			(true, "s", true) => {
				self.save_as();
				Ok(())
			}
			_ => Ok(()),
		}
	}

	fn use_tool(&mut self, tool: Tool) -> Result<(), DesktopError> {
		match tool {
			Tool::Open => return self.open(),
			Tool::Save => {
				self.save();
				return Ok(());
			}
			_ => {}
		}

		let Some(window) = self.window.as_mut() else {
			return Ok(());
		};
		let kit = window.editor().handles();

		if let Some(command) = tool.command(kit) {
			window.editor_mut().run(&command)?;
			let _ = window.handle(&InputEvent::Focus(true))?;
			window.request_redraw();
		}

		self.after_input();
		Ok(())
	}

	fn open(&mut self) -> Result<(), DesktopError> {
		if self.collab.is_some() {
			Self::report("Open is off while collaborating: the room owns the document.");

			return Ok(());
		}

		let Some(path) = rfd::FileDialog::new()
			.add_filter("Markdown", &["md", "markdown"])
			.pick_file()
		else {
			return Ok(());
		};
		let Some(window) = self.window.as_mut() else {
			return Ok(());
		};

		match std::fs::read_to_string(&path) {
			Ok(markdown) => {
				window.editor_mut().load(Format::Markdown, &markdown)?;
				self.saved = window.editor().doc().document().version();
				self.file = Some(path);
				window.request_redraw();
			}
			Err(error) => Self::report(&format!("Could not open {}: {error}", path.display())),
		}

		self.update_title();
		Ok(())
	}

	fn save(&mut self) {
		match self.file.clone() {
			Some(path) => self.write(&path),
			None => self.save_as(),
		}
	}

	fn save_as(&mut self) {
		let dialog = rfd::FileDialog::new()
			.add_filter("Markdown", &["md"])
			.set_file_name("notes.md");

		if let Some(path) = dialog.save_file() {
			self.write(&path);
		}
	}

	fn write(&mut self, path: &std::path::Path) {
		let Some(window) = self.window.as_ref() else {
			return;
		};
		let markdown = window.editor().export(Format::Markdown);

		match std::fs::write(path, markdown + "\n") {
			Ok(()) => {
				self.saved = window.editor().doc().document().version();
				self.file = Some(path.to_owned());
				self.update_title();
			}
			Err(error) => Self::report(&format!("Could not save {}: {error}", path.display())),
		}
	}

	fn user_event(&mut self, event: AppEvent) -> Result<(), DesktopError> {
		let (Some(window), Some(collab)) = (self.window.as_mut(), self.collab.as_mut()) else {
			return Ok(());
		};

		match event {
			AppEvent::Connected => {
				let request = window.editor_mut().connect(collab.client_id);
				collab.connection.send(vec![request]);
				"Live".clone_into(&mut collab.status);
			}
			AppEvent::Sync(message) => {
				let replies = window.editor_mut().receive_sync(&message)?;
				collab.connection.send(replies);
			}
			AppEvent::Disconnected(reason) => {
				eprintln!("collaboration stopped: {reason}");
				"Offline".clone_into(&mut collab.status);
			}
		}

		window.request_redraw();
		Ok(())
	}

	/// Send local changes to collaborators and keep the title current.
	fn after_input(&mut self) {
		if let (Some(window), Some(collab)) = (self.window.as_mut(), self.collab.as_ref())
			&& window.editor().is_shared()
		{
			match window.editor_mut().flush_sync() {
				Ok(updates) => collab.connection.send(updates),
				Err(error) => Self::report(&error.to_string()),
			}
		}

		self.update_title();
	}

	fn render(&mut self) -> Result<(), DesktopError> {
		let Some(window) = self.window.as_mut() else {
			return Ok(());
		};
		let kit = window.editor().handles();
		let state = window.editor().state().clone();
		let width = window.editor_bounds().width();
		let status = status_line(self.file.as_ref(), self.collab.as_ref());
		let toolbar = &mut self.toolbar;

		window.render(|scene, scale| {
			toolbar.paint(
				scene,
				scale,
				width,
				|tool| toolbar::active(tool, kit, &state),
				&status,
			);
		})
	}

	fn update_title(&self) {
		let Some(window) = self.window.as_ref() else {
			return;
		};
		let name = self
			.file
			.as_ref()
			.and_then(|path| path.file_name())
			.map_or_else(
				|| "Untitled".to_owned(),
				|name| name.to_string_lossy().into_owned(),
			);
		let edited = window.editor().doc().document().version() != self.saved;
		let dot = if edited && self.collab.is_none() {
			" •"
		} else {
			""
		};

		window
			.window()
			.set_title(&format!("{name}{dot} — canvist notes"));
	}

	/// Tell the person about a problem that does not stop the app.
	fn report(message: &str) {
		rfd::MessageDialog::new()
			.set_title("canvist notes")
			.set_description(message)
			.set_level(rfd::MessageLevel::Warning)
			.show();
	}

	fn fail(&mut self, event_loop: &ActiveEventLoop, error: DesktopError) {
		self.error = Some(error);
		event_loop.exit();
	}
}

impl ApplicationHandler<AppEvent> for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		if self.window.is_none()
			&& let Err(error) = self.open_window(event_loop)
		{
			self.fail(event_loop, error);
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
		if matches!(event, WindowEvent::CloseRequested) {
			event_loop.exit();

			return;
		}

		if let Err(error) = App::window_event(self, &event) {
			self.fail(event_loop, error);
		}
	}

	fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
		if let Err(error) = App::user_event(self, event) {
			self.fail(event_loop, error);
		}
	}

	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		if let Some(window) = &self.window {
			window.about_to_wait(event_loop);
		}
	}
}

/// The editor fills the window below the toolbar.
fn editor_bounds(window: &EditorWindow<StarterKitHandles>) -> Rect {
	let size = window
		.window()
		.inner_size()
		.to_logical::<f64>(window.window().scale_factor());
	Rect::new(
		0.0,
		toolbar::HEIGHT,
		size.width,
		size.height.max(toolbar::HEIGHT),
	)
}

fn status_line(file: Option<&PathBuf>, collab: Option<&Collab>) -> String {
	if let Some(collab) = collab {
		return collab.status.clone();
	}

	file.and_then(|path| path.file_name()).map_or_else(
		|| "Not saved yet".to_owned(),
		|name| name.to_string_lossy().into_owned(),
	)
}

/// A yrs client id for this run: unique enough among a room's peers.
fn client_id() -> u64 {
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_nanos());
	let process = u64::from(std::process::id()).rotate_left(u32::BITS);
	(nanos as u64 ^ process) & ((1 << CLIENT_ID_BITS) - 1)
}

/// winit's modifier state in the editor's terms.
fn held(state: winit::keyboard::ModifiersState) -> Modifiers {
	Modifiers {
		shift: state.shift_key(),
		ctrl: state.control_key(),
		alt: state.alt_key(),
		meta: state.super_key(),
	}
}
