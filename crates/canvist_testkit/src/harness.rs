//! The harness: one editor, driven the way a person drives it.
//!
//! A test types, presses keys, pastes, selects, runs commands, and reads
//! state back — the same calls on every target. On [`Target::Headless`]
//! they run the editor in this process; on [`Target::Dom`] they type into
//! a real browser page with real keyboard events; on [`Target::Ios`] and
//! [`Target::Android`] they run the platform's own FFI editor on a
//! simulator or emulator.
//!
//! ```no_run
//! # async fn example() -> canvist_testkit::TestResult<()> {
//! use canvist_headless::Format;
//! use canvist_testkit::EditorConfig;
//! use canvist_testkit::EditorHarness;
//! use canvist_testkit::Target;
//!
//! let mut editor = EditorHarness::start(Target::Headless, EditorConfig::default()).await?;
//! editor.load(Format::Text, "").await?;
//!
//! editor.type_text("Hello").await?;
//! editor.press("Mod-b").await?;
//! editor.type_text("world").await?;
//!
//! editor.assert_text("Hello world").await?;
//! editor.assert_json(&serde_json::json!({
//!     "type": "doc",
//!     "content": [{
//!         "type": "paragraph",
//!         "content": [
//!             {"type": "text", "text": "Hello "},
//!             {"type": "text", "marks": [{"type": "bold"}], "text": "world"}
//!         ]
//!     }]
//! })).await?;
//! # Ok(())
//! # }
//! ```

use canvist_headless::Format;
use canvist_headless::canvist_core::Platform;
use canvist_headless::canvist_core::parse_binding;
use serde_json::Value as Json;

use crate::EditorConfig;
use crate::Position;
use crate::Target;
use crate::TestResult;
use crate::TestkitError;
use crate::backend::Backend;
use crate::wire::Read;

/// What a target can do. Tests ask with
/// [`EditorHarness::require`](crate::EditorHarness::require), which skips
/// the test on targets that lack the capability instead of failing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
	/// IME composition: [`EditorHarness::compose`] and friends. The browser
	/// target types real text but does not synthesize composition events.
	Ime,
	/// Collaborative editing: [`EditorHarness::share`] and friends.
	Collab,
}

/// Call one method on whichever backend the harness holds. Every backend
/// implements the same methods; unsupported ones return
/// [`TestkitError::Bridge`].
macro_rules! dispatch {
	($self:ident.$method:ident($($argument:expr),* $(,)?)) => {
		match &mut $self.backend {
			Backend::Headless(backend) => backend.$method($($argument),*).await,
			#[cfg(feature = "dom")]
			Backend::Dom(backend) => backend.$method($($argument),*).await,
			Backend::Agent(backend) => backend.$method($($argument),*).await,
		}
	};
}

/// One editor under test, on one target.
pub struct EditorHarness {
	target: Target,
	config: EditorConfig,
	backend: Backend,
}

impl std::fmt::Debug for EditorHarness {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("EditorHarness")
			.field("target", &self.target)
			.field("config", &self.config)
			.finish_non_exhaustive()
	}
}

impl EditorHarness {
	/// Start an editor on `target` with one empty paragraph.
	///
	/// # Errors
	///
	/// The target cannot run: its feature is off, its tool is missing, or
	/// its build failed. See [`Target`] for what each target needs.
	pub async fn start(target: Target, config: EditorConfig) -> TestResult<Self> {
		let backend = Backend::start(target, &config).await?;

		Ok(Self {
			target,
			config,
			backend,
		})
	}

	/// Join a shared document from `state` (a v1 Yjs update, such as
	/// [`EditorHarness::encode_state`] returns), as a collaborating peer.
	/// The editor arrives shared, with `config`'s client id, which must
	/// differ from the peer that made the state.
	///
	/// # Errors
	///
	/// The target cannot run, or the state cannot be joined.
	#[cfg(feature = "collab")]
	pub async fn join(target: Target, config: EditorConfig, state: &[u8]) -> TestResult<Self> {
		use base64::Engine as _;

		let state = base64::engine::general_purpose::STANDARD.encode(state);
		let backend = Backend::join(target, &config, &state).await?;

		Ok(Self {
			target,
			config,
			backend,
		})
	}

	/// The target this editor runs on.
	#[must_use]
	pub fn target(&self) -> Target {
		self.target
	}

	/// The configuration this editor was built from.
	#[must_use]
	pub fn config(&self) -> &EditorConfig {
		&self.config
	}

	/// The platform key bindings read: Apple keyboards make `Mod` Command,
	/// others Control.
	#[must_use]
	pub fn platform(&self) -> Platform {
		self.backend.platform()
	}

	/// Skip this test unless the target has `capability`, by returning
	/// [`TestkitError::Skip`], which [`crate::cross_test!`] reports as a
	/// skip rather than a failure.
	///
	/// # Errors
	///
	/// [`TestkitError::Skip`] when the target lacks the capability.
	pub fn require(&self, capability: Capability) -> TestResult<()> {
		if self.backend.supports(capability) {
			Ok(())
		} else {
			Err(TestkitError::Skip(format!(
				"the {} target does not support {capability:?}",
				self.target
			)))
		}
	}

	/// Replace the document with `content` read as `format`.
	///
	/// # Errors
	/// Whatever the editor rejects: HTML or JSON that does not fit the
	/// schema, or a format the target cannot read.
	pub async fn load(&mut self, format: Format, content: &str) -> TestResult<()> {
		dispatch!(self.load(format, content))
	}

	/// Replace the document with plain `text`: one paragraph per line.
	///
	/// # Errors
	/// See [`EditorHarness::load`].
	pub async fn load_text(&mut self, text: &str) -> TestResult<()> {
		self.load(Format::Text, text).await
	}

	/// Type `text` one character at a time, as a keyboard would, so input
	/// rules fire: `"## "` turns the block into a heading. Returns whether
	/// anything changed.
	///
	/// On the browser target this is a real keyboard typing into the page;
	/// on the others it is the same character loop the platform's input
	/// path runs.
	///
	/// # Errors
	/// Whatever the editor rejects, or the target's bridge failed.
	pub async fn type_text(&mut self, text: &str) -> TestResult<bool> {
		dispatch!(self.type_text(text))
	}

	/// Insert `text` at the selection in one call, without input rules.
	/// Returns whether anything changed.
	///
	/// # Errors
	/// Whatever the editor rejects, or the target's bridge failed.
	pub async fn insert_text(&mut self, text: &str) -> TestResult<bool> {
		dispatch!(self.insert_text(text))
	}

	/// Press a key or chord, written as key bindings are: `Enter`,
	/// `Backspace`, `Mod-b`, `Shift-Tab`. `Mod` is Command on Apple
	/// platforms and Control elsewhere. Returns whether a bound command
	/// applied.
	///
	/// # Errors
	/// A binding that does not parse, or whatever the editor rejects.
	pub async fn press(&mut self, binding: &str) -> TestResult<bool> {
		let (key, modifiers) = parse_binding(binding, self.platform()).map_err(|error| {
			TestkitError::editor(self.target, canvist_headless::HeadlessError::Editor(error))
		})?;

		dispatch!(self.press(&key, modifiers))
	}

	/// Select from `anchor` to `head`, or place a caret at `anchor` when
	/// `head` is `None`. Positions outside the document are repaired to
	/// the nearest valid ones.
	///
	/// # Errors
	/// Positions the target cannot resolve.
	pub async fn select(&mut self, anchor: &Position, head: Option<&Position>) -> TestResult<()> {
		dispatch!(self.select(anchor, head))
	}

	/// Place the caret at `position`.
	///
	/// # Errors
	/// See [`EditorHarness::select`].
	pub async fn move_to(&mut self, position: &Position) -> TestResult<()> {
		self.select(position, None).await
	}

	/// Place the caret at the end of the block `path` names: where typing
	/// continues a document. The end is computed from the document's own
	/// JSON, so it is the same caret on every target.
	///
	/// # Errors
	/// See [`EditorHarness::select`].
	pub async fn move_to_end(&mut self, path: impl Into<Vec<u32>>) -> TestResult<()> {
		let path = path.into();
		let end = crate::position::text_end(&self.json().await?, &path);

		self.select(&Position::new(path, end), None).await
	}

	/// Select the whole document.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn select_all(&mut self) -> TestResult<()> {
		dispatch!(self.select_all())
	}

	/// Delete the selected text or blocks. Returns whether there was a
	/// selection to delete.
	///
	/// # Errors
	/// Whatever the editor rejects.
	pub async fn delete_selection(&mut self) -> TestResult<bool> {
		dispatch!(self.delete_selection())
	}

	/// Paste `content` read as `format` over the selection, running paste
	/// rules. Returns whether anything changed.
	///
	/// # Errors
	/// Whatever the editor rejects.
	pub async fn paste(&mut self, format: Format, content: &str) -> TestResult<bool> {
		dispatch!(self.paste(format, content))
	}

	/// Run the command an extension registered as `name`, such as
	/// `toggleBold` or `setHeading` with `{ "level": 2 }`. `args` is a JSON
	/// object, or `null`. Returns whether it applied.
	///
	/// # Errors
	/// An unknown command, arguments that do not match its declaration, or
	/// whatever the editor rejects.
	pub async fn run_command(&mut self, name: &str, args: &Json) -> TestResult<bool> {
		dispatch!(self.run_command(name, args))
	}

	/// Whether the command would apply. Nothing changes.
	///
	/// # Errors
	/// See [`EditorHarness::run_command`].
	pub async fn can_command(&mut self, name: &str, args: &Json) -> TestResult<bool> {
		dispatch!(self.can_command(name, args))
	}

	/// Answer the query an extension registered as `name`, such as
	/// `isBold`.
	///
	/// # Errors
	/// An unknown query, or arguments that do not match.
	pub async fn query(&mut self, name: &str, args: &Json) -> TestResult<Json> {
		dispatch!(self.query(name, args))
	}

	/// Undo the last change. Returns whether there was anything to undo.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn undo(&mut self) -> TestResult<bool> {
		dispatch!(self.undo())
	}

	/// Redo the last undone change. Returns whether there was anything to
	/// redo.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn redo(&mut self) -> TestResult<bool> {
		dispatch!(self.redo())
	}

	/// Whether [`EditorHarness::undo`] has anything to undo.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn can_undo(&mut self) -> TestResult<bool> {
		self.read_flag(Read::CanUndo).await
	}

	/// Whether [`EditorHarness::redo`] has anything to redo.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn can_redo(&mut self) -> TestResult<bool> {
		self.read_flag(Read::CanRedo).await
	}

	/// IME: start composing `text`, which becomes the marked (preedit)
	/// text at the caret.
	///
	/// # Errors
	/// Whatever the editor rejects, or
	/// [`TestkitError::Skip`](crate::TestkitError::Skip) through
	/// [`EditorHarness::require`] when the target cannot compose.
	pub async fn compose(&mut self, text: &str) -> TestResult<()> {
		self.require(Capability::Ime)?;
		dispatch!(self.compose(text))
	}

	/// IME: finish composing, replacing the marked text with `text`.
	///
	/// # Errors
	/// See [`EditorHarness::compose`].
	pub async fn commit_composition(&mut self, text: &str) -> TestResult<()> {
		self.require(Capability::Ime)?;
		dispatch!(self.commit_composition(text))
	}

	/// IME: cancel composing, removing the marked text.
	///
	/// # Errors
	/// See [`EditorHarness::compose`].
	pub async fn cancel_composition(&mut self) -> TestResult<()> {
		self.require(Capability::Ime)?;
		dispatch!(self.cancel_composition())
	}

	/// The document as plain text: one paragraph per line.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn text(&mut self) -> TestResult<String> {
		self.read_string(Read::Text).await
	}

	/// The document as Markdown.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn markdown(&mut self) -> TestResult<String> {
		self.read_string(Read::Markdown).await
	}

	/// The document as semantic HTML.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn html(&mut self) -> TestResult<String> {
		self.read_string(Read::Html).await
	}

	/// The document as ProseMirror JSON, with block ids in `attrs.id`.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn json(&mut self) -> TestResult<Json> {
		let text = self.read_string(Read::Json).await?;
		serde_json::from_str(&text).map_err(|error| {
			TestkitError::protocol(
				self.target,
				format!("the JSON export did not parse: {error}"),
			)
		})
	}

	/// The selection as `(anchor, head)`, or `None` for a block selection.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn selection(&mut self) -> TestResult<Option<(Position, Position)>> {
		let value = self.read_value(Read::Selection).await?;

		if value.is_null() {
			return Ok(None);
		}

		#[derive(serde::Deserialize)]
		struct Both {
			anchor: Position,
			head: Position,
		}

		let both = serde_json::from_value::<Both>(value)
			.map_err(|error| TestkitError::protocol(self.target, error.to_string()))?;

		Ok(Some((both.anchor, both.head)))
	}

	/// Release the editor. Tests rarely call this: [`crate::cross_test!`]
	/// closes at the end of the body.
	///
	/// # Errors
	/// Whatever the target's bridge failed.
	pub async fn close(&mut self) -> TestResult<()> {
		dispatch!(self.close())
	}

	/// Start sharing this editor as Yjs client `client_id`, so its changes
	/// become updates other editors can apply.
	///
	/// # Errors
	/// Already shared, or the target cannot collaborate (see
	/// [`Capability::Collab`]).
	#[cfg(feature = "collab")]
	pub async fn share(&mut self, client_id: u64) -> TestResult<()> {
		self.require(Capability::Collab)?;
		dispatch!(self.share(client_id))
	}

	/// Integrate a v1 Yjs update from another editor. Returns whether the
	/// document changed.
	///
	/// # Errors
	/// Not shared, or the update cannot be applied.
	#[cfg(feature = "collab")]
	pub async fn apply_update(&mut self, update: &[u8]) -> TestResult<bool> {
		self.require(Capability::Collab)?;
		dispatch!(self.apply_update(update))
	}

	/// Drain the v1 updates this editor produced for broadcasting.
	///
	/// # Errors
	/// Not shared.
	#[cfg(feature = "collab")]
	pub async fn take_updates(&mut self) -> TestResult<Vec<Vec<u8>>> {
		self.require(Capability::Collab)?;
		dispatch!(self.take_updates())
	}

	/// The whole shared state as one v1 update.
	///
	/// # Errors
	/// Not shared.
	#[cfg(feature = "collab")]
	pub async fn encode_state(&mut self) -> TestResult<Vec<u8>> {
		self.require(Capability::Collab)?;
		dispatch!(self.encode_state())
	}

	/// Deliver everything this editor produced since the last sync to
	/// `other`: the one-line version of a collaborator's websocket.
	///
	/// # Errors
	/// See [`EditorHarness::take_updates`] and
	/// [`EditorHarness::apply_update`].
	#[cfg(feature = "collab")]
	pub async fn sync_to(&mut self, other: &mut Self) -> TestResult<()> {
		for update in self.take_updates().await? {
			other.apply_update(&update).await?;
		}

		Ok(())
	}

	/// Assert the document's plain text.
	///
	/// # Errors
	/// The assertion failed, with both texts in the message.
	pub async fn assert_text(&mut self, expected: &str) -> TestResult<()> {
		let actual = self.text().await?;
		assert_eq_output(self.target, "text", &actual, expected)
	}

	/// Assert the document's Markdown.
	///
	/// # Errors
	/// The assertion failed, with both documents in the message.
	pub async fn assert_markdown(&mut self, expected: &str) -> TestResult<()> {
		let actual = self.markdown().await?;
		assert_eq_output(self.target, "markdown", &actual, expected)
	}

	/// Assert the document's HTML.
	///
	/// # Errors
	/// The assertion failed, with both documents in the message.
	pub async fn assert_html(&mut self, expected: &str) -> TestResult<()> {
		let actual = self.html().await?;
		assert_eq_output(self.target, "html", &actual, expected)
	}

	/// Assert the document's ProseMirror JSON, ignoring block `id`s the
	/// way the conformance fixtures do.
	///
	/// # Errors
	/// The assertion failed, with both documents in the message.
	pub async fn assert_json(&mut self, expected: &Json) -> TestResult<()> {
		let actual = self.json().await?;
		let (actual, expected) = (strip_ids(actual), strip_ids(expected.clone()));

		if actual == expected {
			Ok(())
		} else {
			Err(TestkitError::bridge(
				self.target,
				format!(
					"json is\n      {}\n    expected\n      {}",
					serde_json::to_string_pretty(&actual).unwrap_or_default(),
					serde_json::to_string_pretty(&expected).unwrap_or_default(),
				),
			))
		}
	}

	/// Assert the selection's anchor and head.
	///
	/// # Errors
	/// The assertion failed, with both selections in the message.
	pub async fn assert_selection(&mut self, anchor: &Position, head: &Position) -> TestResult<()> {
		let actual = self.selection().await?;
		let expected = Some((anchor.clone(), head.clone()));

		if actual == expected {
			Ok(())
		} else {
			Err(TestkitError::bridge(
				self.target,
				format!("selection is {actual:?}, expected {expected:?}"),
			))
		}
	}

	/// Assert a block selection, which has no anchor and head to compare.
	///
	/// # Errors
	/// The assertion failed.
	pub async fn assert_block_selection(&mut self) -> TestResult<()> {
		let actual = self.selection().await?;

		if actual.is_none() {
			Ok(())
		} else {
			Err(TestkitError::bridge(
				self.target,
				format!("selection is {actual:?}, expected a block selection"),
			))
		}
	}

	/// Assert the whole document equals `other`'s, the way two synced
	/// collaborators must agree: same JSON, ignoring block ids.
	///
	/// # Errors
	/// The documents differ.
	pub async fn assert_synced_with(&mut self, other: &mut Self) -> TestResult<()> {
		let mine = self.json().await?;
		let theirs = other.json().await?;

		if strip_ids(mine) == strip_ids(theirs) {
			Ok(())
		} else {
			Err(TestkitError::bridge(
				self.target,
				"the two editors' documents differ after syncing",
			))
		}
	}

	async fn read_string(&mut self, what: Read) -> TestResult<String> {
		match self.read_value(what).await? {
			Json::String(text) => Ok(text),
			other => {
				Err(TestkitError::protocol(
					self.target,
					format!("`{}` returned {other}, not a string", what.name()),
				))
			}
		}
	}

	async fn read_flag(&mut self, what: Read) -> TestResult<bool> {
		match self.read_value(what).await? {
			Json::Bool(flag) => Ok(flag),
			other => {
				Err(TestkitError::protocol(
					self.target,
					format!("`{}` returned {other}, not a boolean", what.name()),
				))
			}
		}
	}

	async fn read_value(&mut self, what: Read) -> TestResult<Json> {
		dispatch!(self.read(what))
	}
}

/// Compare and report, as a `Result` the test macros can propagate.
fn assert_eq_output(target: Target, format: &str, actual: &str, expected: &str) -> TestResult<()> {
	if actual == expected {
		Ok(())
	} else {
		Err(TestkitError::bridge(
			target,
			format!("{format} is\n      {actual}\n    expected\n      {expected}"),
		))
	}
}

/// Remove `id` from every `attrs` object, then every `attrs` left empty:
/// the comparison the conformance fixtures make.
pub(crate) fn strip_ids(json: Json) -> Json {
	match json {
		Json::Array(items) => Json::Array(items.into_iter().map(strip_ids).collect()),
		Json::Object(mut fields) => {
			if let Some(Json::Object(attrs)) = fields.get_mut("attrs") {
				attrs.remove("id");

				if attrs.is_empty() {
					fields.remove("attrs");
				}
			}

			Json::Object(
				fields
					.into_iter()
					.map(|(key, value)| (key, strip_ids(value)))
					.collect(),
			)
		}
		other => other,
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn strip_ids_removes_every_id_and_empty_attrs() {
		let json = json!({
			"type": "doc",
			"content": [
				{
					"type": "heading",
					"attrs": { "id": "abc", "level": 2 },
					"content": [{ "type": "text", "text": "Hi" }],
				},
				{ "type": "paragraph", "attrs": { "id": "def" } },
			],
		});

		assert_eq!(
			strip_ids(json),
			json!({
				"type": "doc",
				"content": [
					{
						"type": "heading",
						"attrs": { "level": 2 },
						"content": [{ "type": "text", "text": "Hi" }],
					},
					{ "type": "paragraph" },
				],
			}),
		);
	}
}
