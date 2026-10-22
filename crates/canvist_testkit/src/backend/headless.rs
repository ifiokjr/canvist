//! The headless backend: the editor in this process, driven directly.

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::canvist_core::CommandResult;
use canvist_headless::canvist_core::Editor;
use canvist_headless::canvist_core::Env;
use canvist_headless::canvist_core::KeyPress;
use canvist_headless::canvist_core::Modifiers;
use canvist_headless::canvist_core::ParagraphNavigator;
use canvist_headless::canvist_core::Platform;
use serde_json::Value as Json;

use crate::EditorConfig;
use crate::Position;
use crate::Target;
use crate::TestResult;
use crate::TestkitError;
use crate::wire::Read;

/// A `canvist_headless` engine and document: the same editor the server
/// bindings run, with no UI and no clock of its own.
pub(crate) struct HeadlessBackend {
	engine: Engine,
	doc: Doc,
	platform: Platform,
	/// The typing clock. `Doc`'s own calls advance a second at a time, so
	/// each character would be its own undo step; a real host's typing
	/// lands within the undo group delay, so this moves 100 ms per
	/// character and `Doc::edit` clamps it past whatever the last native
	/// call reached.
	clock: std::time::Duration,
}

#[allow(
	clippy::unused_async,
	reason = "the dispatch macro calls every backend the same way"
)]
impl HeadlessBackend {
	pub(crate) async fn start(config: &EditorConfig) -> TestResult<Self> {
		let names: Vec<&str> = config.extensions.iter().map(String::as_str).collect();
		let platform = config.platform.unwrap_or(Platform::Other);
		let engine = Engine::named(&names)
			.map_err(error)?
			.with_platform(platform);
		let doc = engine.create().map_err(error)?;

		Ok(Self {
			engine,
			doc,
			platform,
			clock: std::time::Duration::ZERO,
		})
	}

	pub(crate) fn platform(&self) -> Platform {
		self.platform
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn join(config: &EditorConfig, state: &str) -> TestResult<Self> {
		use base64::Engine as _;

		let names: Vec<&str> = config.extensions.iter().map(String::as_str).collect();
		let platform = config.platform.unwrap_or(Platform::Other);
		let engine = Engine::named(&names)
			.map_err(error)?
			.with_platform(platform);
		let state = base64::engine::general_purpose::STANDARD
			.decode(state)
			.map_err(|error| TestkitError::protocol(Target::Headless, error.to_string()))?;
		let doc = engine
			.from_yjs(&state, u64::from(config.client.max(1)))
			.map_err(error)?;

		Ok(Self {
			engine,
			doc,
			platform,
			clock: std::time::Duration::ZERO,
		})
	}

	#[allow(clippy::unused_self, reason = "every backend answers the same way")]
	pub(crate) fn supports(&self, capability: crate::Capability) -> bool {
		match capability {
			crate::Capability::Ime => true,
			#[cfg(feature = "collab")]
			crate::Capability::Collab => true,
			#[cfg(not(feature = "collab"))]
			crate::Capability::Collab => false,
		}
	}

	pub(crate) async fn load(&mut self, format: Format, content: &str) -> TestResult<()> {
		self.doc = self.engine.parse(format, content).map_err(error)?;

		Ok(())
	}

	pub(crate) async fn type_text(&mut self, text: &str) -> TestResult<bool> {
		let mut changed = false;
		let mut buffer = [0; 4];

		for character in text.chars() {
			let typed = character.encode_utf8(&mut buffer).to_owned();
			let update = self.edit(|editor, env| editor.insert_text(&typed, env))?;

			changed |= update.is_some();
		}

		Ok(changed)
	}

	pub(crate) async fn insert_text(&mut self, text: &str) -> TestResult<bool> {
		self.doc.insert_text(text).map_err(error)
	}

	pub(crate) async fn press(&mut self, key: &str, modifiers: Modifiers) -> TestResult<bool> {
		self.edit(|editor, env| editor.handle_key(KeyPress::new(key, modifiers), env))
			.map(|update| update.is_some())
	}

	pub(crate) async fn select(
		&mut self,
		anchor: &Position,
		head: Option<&Position>,
	) -> TestResult<()> {
		let anchor: canvist_headless::Position = anchor.clone().into();
		let head: canvist_headless::Position =
			head.map_or_else(|| anchor.clone(), |head| head.clone().into());

		self.doc.select_range(&anchor, &head).map_err(error)
	}

	pub(crate) async fn select_all(&mut self) -> TestResult<()> {
		self.doc.select_all().map_err(error)
	}

	pub(crate) async fn delete_selection(&mut self) -> TestResult<bool> {
		self.doc.delete_selection().map_err(error)
	}

	pub(crate) async fn paste(&mut self, format: Format, content: &str) -> TestResult<bool> {
		self.doc.paste(format, content).map_err(error)
	}

	pub(crate) async fn run_command(&mut self, name: &str, args: &Json) -> TestResult<bool> {
		self.doc.run_named(name, args).map_err(error)
	}

	pub(crate) async fn can_command(&mut self, name: &str, args: &Json) -> TestResult<bool> {
		self.doc.can_named(name, args).map_err(error)
	}

	pub(crate) async fn query(&mut self, name: &str, args: &Json) -> TestResult<Json> {
		self.doc.query(name, args).map_err(error)
	}

	pub(crate) async fn undo(&mut self) -> TestResult<bool> {
		self.doc.undo().map_err(error)
	}

	pub(crate) async fn redo(&mut self) -> TestResult<bool> {
		self.doc.redo().map_err(error)
	}

	pub(crate) async fn compose(&mut self, text: &str) -> TestResult<()> {
		self.edit(|editor, env| editor.set_composition(text, env))
			.map(|_| ())
	}

	pub(crate) async fn commit_composition(&mut self, text: &str) -> TestResult<()> {
		self.edit(|editor, env| editor.commit_composition(text, env))
			.map(|_| ())
	}

	pub(crate) async fn cancel_composition(&mut self) -> TestResult<()> {
		self.edit(Editor::cancel_composition).map(|_| ())
	}

	pub(crate) async fn read(&mut self, what: Read) -> TestResult<Json> {
		let value = match what {
			Read::Text => Json::String(self.doc.export(Format::Text)),
			Read::Markdown => Json::String(self.doc.export(Format::Markdown)),
			Read::Html => Json::String(self.doc.export(Format::Html)),
			Read::Json => Json::String(self.doc.export(Format::Json)),
			Read::Selection => {
				match self.doc.selected_range() {
					Some((anchor, head)) => {
						serde_json::json!({
							"anchor": Position::from(anchor),
							"head": Position::from(head),
						})
					}
					None => Json::Null,
				}
			}
			Read::CanUndo => Json::Bool(self.doc.can_undo()),
			Read::CanRedo => Json::Bool(self.doc.can_redo()),
		};

		Ok(value)
	}

	pub(crate) async fn close(&mut self) -> TestResult<()> {
		Ok(())
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn share(&mut self, client_id: u64) -> TestResult<()> {
		self.doc.share(client_id).map_err(error)
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn apply_update(&mut self, update: &[u8]) -> TestResult<bool> {
		self.doc.apply_update(update).map_err(error)
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn take_updates(&mut self) -> TestResult<Vec<Vec<u8>>> {
		self.doc.take_updates().map_err(error)
	}

	#[cfg(feature = "collab")]
	pub(crate) async fn encode_state(&mut self) -> TestResult<Vec<u8>> {
		self.doc.encode_state().map_err(error)
	}

	/// Run one editor call a heartbeat after the last, the way a person's
	/// next keystroke lands. `Doc::edit` clamps the time past whatever
	/// `Doc`'s own second-at-a-time calls reached.
	fn edit(
		&mut self,
		change: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> TestResult<Option<canvist_headless::canvist_core::Update>> {
		self.clock += std::time::Duration::from_millis(100);

		self.doc
			.edit(self.clock, &mut ParagraphNavigator, change)
			.map_err(error)
	}
}

/// Mark an editor error as the headless target's.
fn error(error: HeadlessError) -> TestkitError {
	TestkitError::editor(Target::Headless, error)
}
