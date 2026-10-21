//! The window host (feature `winit`): the editor in a winit window, drawn
//! with vello on wgpu, with the system clipboard, the platform's input
//! methods, and the AccessKit platform adapter.
//!
//! [`EditorWindow`] embeds in any winit app: forward its window's events to
//! [`EditorWindow::window_event`], call [`EditorWindow::render`] on
//! `RedrawRequested` (drawing your own chrome, such as a toolbar, into the
//! same scene), and [`EditorWindow::about_to_wait`] before the loop sleeps.
//! For a window that holds only the editor, [`run`] does all of that.

mod clipboard;
mod events;
mod gpu;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

use accesskit::ActionHandler;
use accesskit::ActionRequest;
use accesskit::ActivationHandler;
use accesskit::DeactivationHandler;
use accesskit::TreeUpdate;
pub use clipboard::SystemClipboard;
pub use events::WinitInput;
pub use gpu::GpuSurface;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use vello::Scene;
pub use winit;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalPosition;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::CursorIcon;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

use crate::Context;
use crate::DesktopEditor;
use crate::DesktopError;
use crate::DesktopResult;
use crate::InputEvent;
use crate::Response;
use crate::Viewport;

/// How often a drag past the editor's edge scrolls.
const AUTOSCROLL_TICK: Duration = Duration::from_millis(16);

/// What the AccessKit adapter asked for since the last frame. Its handlers
/// may run on any thread, so they only queue requests and wake the window.
#[derive(Debug, Default)]
struct AccessRequests {
	active: bool,
	actions: Vec<ActionRequest>,
}

struct Handler {
	requests: Arc<Mutex<AccessRequests>>,
	window: Arc<Window>,
}

impl Handler {
	fn with(&self, change: impl FnOnce(&mut AccessRequests)) {
		change(&mut self.requests.lock().unwrap_or_else(PoisonError::into_inner));
		self.window.request_redraw();
	}
}

impl ActivationHandler for Handler {
	fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
		self.with(|requests| requests.active = true);
		None
	}
}

impl ActionHandler for Handler {
	fn do_action(&mut self, request: ActionRequest) {
		self.with(|requests| requests.actions.push(request));
	}
}

impl DeactivationHandler for Handler {
	fn deactivate_accessibility(&mut self) {
		self.with(|requests| requests.active = false);
	}
}

/// A [`DesktopEditor`] in a winit window.
pub struct EditorWindow<Handles> {
	editor: DesktopEditor<Handles>,
	window: Arc<Window>,
	gpu: GpuSurface,
	painter: canvist_paint::VelloPainter,
	/// The font generation the painter's shaper was built for.
	fonts: u64,
	scene: Scene,
	clipboard: SystemClipboard,
	input: WinitInput,
	adapter: accesskit_winit::Adapter,
	access: Arc<Mutex<AccessRequests>>,
	/// Where the editor sits in the window, in logical pixels, or `None` for
	/// the whole window.
	bounds: Option<Rect>,
	title: String,
	started: Instant,
}

impl<Handles> std::fmt::Debug for EditorWindow<Handles> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("EditorWindow")
			.field("editor", &self.editor)
			.field("bounds", &self.bounds)
			.finish_non_exhaustive()
	}
}

impl<Handles: Copy> EditorWindow<Handles> {
	/// Open a window with `attributes` and put `editor` in it. Call this from
	/// `ApplicationHandler::resumed`. The window is created hidden, so the
	/// accessibility adapter can attach first, and then shown unless
	/// `attributes` ask for it hidden.
	pub fn new(
		event_loop: &ActiveEventLoop,
		attributes: WindowAttributes,
		editor: DesktopEditor<Handles>,
	) -> DesktopResult<Self> {
		let visible = attributes.visible;
		let title = attributes.title.clone();
		let window = Arc::new(event_loop.create_window(attributes.with_visible(false))?);
		let access = Arc::new(Mutex::new(AccessRequests::default()));
		let handler = || {
			Handler {
				requests: access.clone(),
				window: window.clone(),
			}
		};
		let adapter = accesskit_winit::Adapter::with_direct_handlers(
			event_loop,
			&window,
			handler(),
			handler(),
			handler(),
		);
		window.set_visible(visible);
		let gpu = GpuSurface::new(window.clone())?;
		let mut host = Self {
			painter: canvist_paint::VelloPainter::new(),
			fonts: u64::MAX,
			editor,
			window,
			gpu,
			scene: Scene::new(),
			clipboard: SystemClipboard::new(),
			input: WinitInput::new(),
			adapter,
			access,
			bounds: None,
			title,
			started: Instant::now(),
		};
		host.resize_editor();
		host.window.request_redraw();
		Ok(host)
	}

	/// The editor.
	#[must_use]
	pub fn editor(&self) -> &DesktopEditor<Handles> {
		&self.editor
	}

	/// The editor, to run commands or load documents. Call
	/// [`EditorWindow::request_redraw`] afterwards.
	pub fn editor_mut(&mut self) -> &mut DesktopEditor<Handles> {
		&mut self.editor
	}

	/// The window.
	#[must_use]
	pub fn window(&self) -> &Arc<Window> {
		&self.window
	}

	/// The clipboard, for host commands such as a toolbar's Paste button.
	pub fn clipboard_mut(&mut self) -> &mut SystemClipboard {
		&mut self.clipboard
	}

	/// Time since the window opened: the clock input events are stamped
	/// with.
	#[must_use]
	pub fn now(&self) -> Duration {
		self.started.elapsed()
	}

	/// Place the editor at `bounds` in the window's logical pixels, such as
	/// below a toolbar. `None` fills the window.
	pub fn set_editor_bounds(&mut self, bounds: Option<Rect>) {
		self.bounds = bounds;
		self.resize_editor();
		self.window.request_redraw();
	}

	/// Where the editor is in the window's logical pixels.
	#[must_use]
	pub fn editor_bounds(&self) -> Rect {
		self.bounds.unwrap_or_else(|| {
			let size = self.window.inner_size().to_logical::<f64>(self.scale());
			Rect::new(0.0, 0.0, size.width, size.height)
		})
	}

	/// Ask for a frame.
	pub fn request_redraw(&self) {
		self.window.request_redraw();
	}

	/// Handle one of the window's events. Window events go to the
	/// accessibility adapter first, then input goes to the editor. The
	/// response says whether the editor used the event; anything else is the
	/// app's to handle. `RedrawRequested` is the app's too: call
	/// [`EditorWindow::render`].
	pub fn window_event(&mut self, event: &WindowEvent) -> DesktopResult<Response> {
		self.adapter.process_event(&self.window, event);

		match event {
			WindowEvent::Resized(size) => {
				self.gpu.resize(size.width, size.height);
				self.resize_editor();
				self.window.request_redraw();
				return Ok(Response::CHANGED);
			}
			WindowEvent::ScaleFactorChanged { .. } => {
				self.resize_editor();
				self.window.request_redraw();
				return Ok(Response::CHANGED);
			}
			_ => {}
		}

		let bounds = self.editor_bounds();
		let Some(input) = self.input.translate(event, bounds.origin(), self.scale()) else {
			return Ok(Response::IGNORED);
		};

		if !self.routes_to_editor(&input, bounds) {
			return Ok(Response::IGNORED);
		}

		let was_focused = self.editor.is_focused();
		let response = self.handle(&input)?;
		self.update_cursor(bounds);

		if self.editor.is_focused() != was_focused {
			self.window.set_ime_allowed(self.editor.is_focused());
		}

		Ok(response)
	}

	/// Handle an editor input event with this window's clock and clipboard.
	pub fn handle(&mut self, input: &InputEvent) -> DesktopResult<Response> {
		let now = self.now();
		let mut cx = Context::new(now, &mut self.clipboard);
		let response = self.editor.handle(input, &mut cx)?;

		if response.repaint {
			self.window.request_redraw();
		}

		Ok(response)
	}

	/// Draw a frame: `chrome` paints the app's own content into the scene
	/// first (a background, a toolbar), then the editor paints over its
	/// bounds. Call it on `RedrawRequested`.
	pub fn render(&mut self, chrome: impl FnOnce(&mut Scene, f64)) -> DesktopResult<()> {
		self.answer_accessibility()?;
		let now = self.now();
		self.editor.autoscroll(now)?;
		self.editor.frame(now)?;

		if self.fonts != self.editor.fonts().generation() {
			self.fonts = self.editor.fonts().generation();
			*self.painter.shaper_mut() = self.editor.fonts().shaper();
		}

		let scale = self.scale();
		let to_window = self.to_window();
		self.scene.reset();
		chrome(&mut self.scene, scale);
		self.painter.reset();
		self.editor.paint(&mut self.painter, to_window);
		self.scene.append(self.painter.scene(), None);

		let background = self.editor.options().theme.chrome().background;
		self.gpu.render(
			&self.scene,
			canvist_paint::convert::layout_color(background),
		)?;
		self.place_ime();
		self.update_accessibility(to_window);
		Ok(())
	}

	/// Schedule the next wake-up: when the caret blinks, or soon while a
	/// drag past the edge autoscrolls. Call it from
	/// `ApplicationHandler::about_to_wait`.
	pub fn about_to_wait(&self, event_loop: &ActiveEventLoop) {
		if self.editor.is_dragging() {
			self.window.request_redraw();
			event_loop.set_control_flow(ControlFlow::wait_duration(AUTOSCROLL_TICK));

			return;
		}

		let next = self.editor.next_blink().map(|at| self.started + at);

		match next {
			Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
			None => event_loop.set_control_flow(ControlFlow::Wait),
		}

		if next.is_some_and(|at| at <= Instant::now()) {
			self.window.request_redraw();
		}
	}

	/// Whether `input` belongs to the editor: presses and wheel turns over
	/// it, and every other event.
	fn routes_to_editor(&self, input: &InputEvent, bounds: Rect) -> bool {
		let inside = |pointer: Option<Point>| pointer.is_some_and(|point| bounds.contains(point));

		match input {
			InputEvent::PointerDown { .. } | InputEvent::Scroll(_) => inside(self.input.pointer()),
			InputEvent::PointerMove { .. } => {
				self.editor.is_dragging() || inside(self.input.pointer())
			}
			_ => true,
		}
	}

	fn scale(&self) -> f64 {
		self.window.scale_factor()
	}

	/// Maps the editor's logical coordinates to the window's physical pixels.
	fn to_window(&self) -> Affine {
		let origin = self.editor_bounds().origin();
		Affine::scale(self.scale()) * Affine::translate(origin.to_vec2())
	}

	fn resize_editor(&mut self) {
		let bounds = self.editor_bounds();
		self.editor.resize(Viewport {
			width: bounds.width() as f32,
			height: bounds.height() as f32,
			scale: self.scale() as f32,
		});
	}

	fn update_cursor(&self, bounds: Rect) {
		let over = self
			.input
			.pointer()
			.is_some_and(|point| bounds.contains(point));
		self.window.set_cursor(if over {
			CursorIcon::Text
		} else {
			CursorIcon::Default
		});
	}

	/// Put the input method's candidate window under the composition or the
	/// caret.
	fn place_ime(&self) {
		let Some(area) = self.editor.ime_area().filter(|_| self.editor.is_focused()) else {
			return;
		};
		let origin = self.editor_bounds().origin();
		self.window.set_ime_cursor_area(
			LogicalPosition::new(origin.x + f64::from(area.x), origin.y + f64::from(area.y)),
			LogicalSize::new(f64::from(area.width.max(1.0)), f64::from(area.height)),
		);
	}

	/// Run the actions screen readers asked for since the last frame.
	fn answer_accessibility(&mut self) -> DesktopResult<()> {
		let actions = std::mem::take(
			&mut self
				.access
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.actions,
		);

		for action in &actions {
			let _ = self.editor.accessibility_action(action)?;
		}

		Ok(())
	}

	fn update_accessibility(&mut self, to_window: Affine) {
		let active = self
			.access
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.active;

		if active {
			let editor = &mut self.editor;
			let title = &self.title;
			self.adapter
				.update_if_active(|| editor.accessibility_tree(title, to_window));
		}
	}
}

/// Open a window holding only `editor` and run until it closes.
///
/// ```no_run
/// use canvist_desktop::DesktopEditor;
/// use canvist_desktop::EditorOptions;
/// use canvist_desktop::winit::winit::window::Window;
/// use canvist_headless::Engine;
///
/// let editor = DesktopEditor::new(Engine::starter_kit(), EditorOptions::new().max_width(720.0))?;
/// canvist_desktop::winit::run(editor, Window::default_attributes().with_title("Notes"))?;
/// # Ok::<(), canvist_desktop::DesktopError>(())
/// ```
pub fn run<Handles: Copy + 'static>(
	editor: DesktopEditor<Handles>,
	attributes: WindowAttributes,
) -> DesktopResult<()> {
	let event_loop = EventLoop::new()?;
	let mut app = App {
		pending: Some((editor, attributes)),
		window: None,
		error: None,
	};
	event_loop.run_app(&mut app)?;
	app.error.map_or(Ok(()), Err)
}

/// The app [`run`] runs: one editor window.
struct App<Handles> {
	pending: Option<(DesktopEditor<Handles>, WindowAttributes)>,
	window: Option<EditorWindow<Handles>>,
	error: Option<DesktopError>,
}

impl<Handles: Copy> App<Handles> {
	fn fail(&mut self, event_loop: &ActiveEventLoop, error: DesktopError) {
		self.error = Some(error);
		event_loop.exit();
	}
}

impl<Handles: Copy> ApplicationHandler for App<Handles> {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let Some((editor, attributes)) = self.pending.take() else {
			return;
		};

		match EditorWindow::new(event_loop, attributes, editor) {
			Ok(window) => self.window = Some(window),
			Err(error) => self.fail(event_loop, error),
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
		let Some(window) = self.window.as_mut() else {
			return;
		};

		let result = match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
				return;
			}
			WindowEvent::RedrawRequested => window.render(|_, _| {}),
			event => window.window_event(&event).map(drop),
		};

		if let Err(error) = result {
			self.fail(event_loop, error);
		}
	}

	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		if let Some(window) = &self.window {
			window.about_to_wait(event_loop);
		}
	}
}
