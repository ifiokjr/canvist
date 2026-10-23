//! The editor object hosts own, and the changes it reports.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use serde_json::Value as Json;

use crate::Command;
use crate::DocFormat;
use crate::DocPosition;
use crate::EditorChange;
use crate::EditorOptions;
use crate::Frame;
use crate::Modifiers;
use crate::PlatformMeasurer;
use crate::TextSelection;
use crate::ThemeOptions;
use crate::error::CanvistError;
use crate::error::Result;
use crate::measure::HostMeasurer;
use crate::session::Session;

/// Told after every call that changed the editor, once the editor is ready
/// for more calls: an observer may read from or call into the editor.
///
/// The iOS and Android views observe their editor to repaint, so commands
/// an app runs from a toolbar show up without further work.
#[uniffi::export(with_foreign)]
pub trait EditorObserver: Send + Sync {
	/// Something changed.
	fn changed(&self, change: EditorChange);
}

/// Measurement and cache counters, for performance tests and tuning.
#[derive(Clone, Copy, Debug, Default, PartialEq, uniffi::Record)]
pub struct EditorStats {
	/// Calls to the platform measurer.
	pub measure_calls: u64,
	/// Segments the platform measured.
	pub measured_segments: u64,
	/// Segment lookups answered from the cache.
	pub cache_hits: u64,
	/// Segment lookups that had to measure.
	pub cache_misses: u64,
}

/// A rich-text editor: a document, the commands of its extensions, its
/// layout, and what to paint, with no UI of its own.
///
/// The iOS and Android `CanvistView`s drive one; other hosts (React Native,
/// Lynx, tests) can drive one directly. Every method is safe to call from
/// any thread, one call at a time; views call from the main thread.
///
/// Text input methods address the document as one string of UTF-16 code
/// units: every text block in order, joined by line feeds, with inline
/// embeds as one code unit. That is what `UITextInput` and Android's
/// `InputConnection` expect.
#[derive(uniffi::Object)]
pub struct Editor {
	session: Mutex<Session>,
	observers: Mutex<Vec<(u64, Arc<dyn EditorObserver>)>>,
	next_observer: Mutex<u64>,
}

impl std::fmt::Debug for Editor {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.debug_struct("Editor").finish_non_exhaustive()
	}
}

/// The version of the Rust core, so hosts can check they loaded the library
/// they were built against.
#[uniffi::export]
#[must_use]
pub fn core_version() -> String {
	env!("CARGO_PKG_VERSION").to_owned()
}

/// Options for an editor with every StarterKit extension, client 1, and
/// the default theme, drawing its own caret and selection. Views adjust
/// the platform fields.
#[uniffi::export]
#[must_use]
pub fn default_editor_options() -> EditorOptions {
	EditorOptions::starter_kit()
}

/// The default theme: light, the system UI font at 17 points.
#[uniffi::export]
#[must_use]
pub fn default_theme_options() -> ThemeOptions {
	ThemeOptions::default()
}

/// The metadata of every extension `extensions` names, plus the ones every
/// editor has, as JSON: names, nodes, commands, and queries with their
/// arguments. The typed handles are generated from it.
#[uniffi::export]
#[allow(
	clippy::needless_pass_by_value,
	reason = "UniFFI hands records and lists over by value"
)]
pub fn extension_metadata(extensions: Vec<String>) -> Result<String> {
	let engine = canvist_headless::Engine::named(&extensions)?;
	Ok(engine.metadata().to_string())
}

#[uniffi::export]
#[allow(
	clippy::needless_pass_by_value,
	reason = "UniFFI hands records and lists over by value"
)]
impl Editor {
	/// An editor with one empty paragraph, measuring text with `measurer`.
	#[uniffi::constructor]
	pub fn new(options: EditorOptions, measurer: Arc<dyn PlatformMeasurer>) -> Result<Arc<Self>> {
		let session = Session::new(&options, HostMeasurer::new(measurer))?;

		Ok(Arc::new(Self {
			session: Mutex::new(session),
			observers: Mutex::new(Vec::new()),
			next_observer: Mutex::new(0),
		}))
	}

	/// Replace the document with `content` read as `format`. A shared
	/// document stops being shared.
	pub fn load(&self, format: DocFormat, content: &str) -> Result<()> {
		self.change(|session| {
			let doc = session.engine().parse(format.into(), content)?;
			session.set_doc(doc);
			Ok(())
		})
	}

	/// The document as `format`.
	pub fn export(&self, format: DocFormat) -> String {
		self.lock().doc.export(format.into())
	}

	/// The registered extensions' metadata as JSON.
	pub fn metadata(&self) -> String {
		self.lock().engine().metadata().to_string()
	}

	/// Run `command` from a typed handle. Returns whether it applied.
	pub fn run(&self, command: Command) -> Result<bool> {
		let args = parse_args(&command)?;
		self.change(|session| {
			let command = session.doc.command_named(&command.name, &args)?;
			session.edit(|editor, env| editor.run(&command, env))
		})
	}

	/// Whether `command` would apply. Nothing changes.
	pub fn can(&self, command: Command) -> Result<bool> {
		let args = parse_args(&command)?;
		Ok(self.lock().doc.can_named(&command.name, &args)?)
	}

	/// Answer the query an extension registered as `name`, with `args` as a
	/// JSON object (or `null`), returning JSON. The typed handles decode it.
	pub fn query(&self, name: &str, args: &str) -> Result<String> {
		let args: Json =
			serde_json::from_str(args).map_err(|error| CanvistError::invalid_args(name, error))?;
		Ok(self.lock().doc.query(name, &args)?.to_string())
	}

	/// Run the binding for a key press, named as DOM `KeyboardEvent.key`
	/// names keys: `Enter`, `Backspace`, `ArrowUp`, `Tab`, or `b` with
	/// Command for bold. Returns whether a binding applied; when none did,
	/// the platform should handle the key.
	pub fn press(&self, key: &str, modifiers: Modifiers) -> Result<bool> {
		self.change(|session| session.press(key, modifiers))
	}

	/// Undo the last change. On a shared document, only this editor's own
	/// changes are undone.
	pub fn undo(&self) -> Result<bool> {
		self.change(|session| session.with_doc(canvist_headless::Doc::undo))
	}

	/// Redo the last undone change.
	pub fn redo(&self) -> Result<bool> {
		self.change(|session| session.with_doc(canvist_headless::Doc::redo))
	}

	/// Whether [`Editor::undo`] has anything to undo.
	pub fn can_undo(&self) -> bool {
		self.lock().doc.can_undo()
	}

	/// Whether [`Editor::redo`] has anything to redo.
	pub fn can_redo(&self) -> bool {
		self.lock().doc.can_redo()
	}

	/// The selection as text offsets.
	pub fn selection(&self) -> TextSelection {
		self.lock().selection()
	}

	/// Select from `anchor` to `head`, text offsets clamped to the text.
	/// Returns whether the selection moved.
	pub fn select(&self, anchor: u32, head: u32) -> Result<bool> {
		self.change(|session| session.select(anchor, head))
	}

	/// Select the whole document.
	pub fn select_all(&self) -> Result<bool> {
		self.change(|session| {
			let command = canvist_headless::canvist_core::command(
				canvist_headless::canvist_core::commands::select_all,
			);
			session.edit(|editor, env| editor.run(&command, env))
		})
	}

	/// The selection's anchor and head as positions, the address every
	/// binding and the conformance fixtures share, or `None` for a block
	/// selection.
	pub fn selected_positions(&self) -> Option<Vec<DocPosition>> {
		self.lock()
			.doc
			.selected_range()
			.map(|(anchor, head)| vec![anchor.into(), head.into()])
	}

	/// Select from `anchor` to `head`, given as positions.
	pub fn select_positions(&self, anchor: DocPosition, head: DocPosition) -> Result<bool> {
		self.change(|session| {
			let anchor = session.doc.point(&anchor.into())?;
			let head = session.doc.point(&head.into())?;
			let selection = canvist_headless::canvist_model::Selection::text(anchor, head);
			session.edit(|editor, env| editor.select(selection, env))
		})
	}

	/// Size the viewport: the document is laid out `width` wide, and frames
	/// cover `height` from the scroll position down.
	pub fn set_viewport(&self, width: f32, height: f32) {
		self.lock().set_viewport(width, height);
	}

	/// Scroll so document coordinate `top` is at the viewport's top edge.
	pub fn set_scroll(&self, top: f32) {
		self.lock().set_scroll(top);
	}

	/// Show the caret (focused) or hide it (blurred).
	pub fn set_focused(&self, focused: bool) {
		self.lock().set_focused(focused);
	}

	/// Blink the caret: hosts toggle this on a timer while focused.
	pub fn set_caret_visible(&self, visible: bool) {
		self.lock().set_caret_visible(visible);
	}

	/// Restyle the document, such as for dark mode or a new text size.
	pub fn set_theme(&self, theme: ThemeOptions) -> Result<()> {
		self.lock().set_theme(&theme)
	}

	/// Measure everything again, as when the platform's fonts changed.
	pub fn forget_measurements(&self) {
		self.lock().forget_measurements();
	}

	/// The document's height, including estimates for blocks not laid out.
	pub fn content_height(&self) -> f32 {
		self.lock().height()
	}

	/// Lay out the viewport and report what to paint since the last frame.
	pub fn frame(&self) -> Result<Frame> {
		self.lock().frame()
	}

	/// Measurement and cache counters.
	pub fn stats(&self) -> EditorStats {
		let session = self.lock();
		let cache = session.layout.measure_stats();

		EditorStats {
			measure_calls: session.measurer.calls,
			measured_segments: session.measurer.segments,
			cache_hits: cache.hits,
			cache_misses: cache.misses,
		}
	}

	/// Tell `observer` about every change from now on. Returns an id for
	/// [`Editor::remove_observer`]. The editor holds the observer strongly,
	/// so an observer that owns the editor should hold it weakly.
	pub fn add_observer(&self, observer: Arc<dyn EditorObserver>) -> u64 {
		let mut next = self
			.next_observer
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		*next += 1;
		self.observers
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.push((*next, observer));
		*next
	}

	/// Stop telling the observer `id` about changes.
	pub fn remove_observer(&self, id: u64) {
		self.observers
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.retain(|(observer, _)| *observer != id);
	}
}

impl Editor {
	/// The session. A panic while it was held may have left a change half
	/// applied to layout, but never to the document, which every change
	/// reaches through the model's transactions; a poisoned lock is still
	/// usable, and the next frame repaints from the document.
	pub(crate) fn lock(&self) -> MutexGuard<'_, Session> {
		self.session.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// The session, laid out for geometry around `offsets` and the selection
	/// (see [`Session::lay_out`]).
	pub(crate) fn laid_out(&self, offsets: &[u32]) -> MutexGuard<'_, Session> {
		let mut session = self.lock();
		session.lay_out(offsets);
		session
	}

	/// Run `call` on the session, then tell observers what changed, after
	/// releasing the lock so they can call back in.
	pub(crate) fn change<T>(&self, call: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
		let (result, change) = {
			let mut session = self.lock();
			let before = session.observed();
			let result = call(&mut session);
			(result, session.change_since(&before))
		};

		if !change.is_empty() {
			self.notify(change);
		}

		result
	}

	fn notify(&self, change: EditorChange) {
		let observers: Vec<Arc<dyn EditorObserver>> = self
			.observers
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(_, observer)| observer.clone())
			.collect();

		for observer in observers {
			observer.changed(change);
		}
	}
}

fn parse_args(command: &Command) -> Result<Json> {
	serde_json::from_str(&command.args)
		.map_err(|error| CanvistError::invalid_args(&command.name, error))
}
