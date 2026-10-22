//! Quick state-A-to-state-B tests, for when the input path is not the
//! point.
//!
//! A [`TransformCase`] loads a document, runs a few steps, and checks the
//! result. It runs the headless engine in this process — no targets, no
//! async, microseconds a step — which makes it the place for the many
//! small checks that would bury an integration test: a mark applied to a
//! range, a join across blocks, an input rule firing.
//!
//! ```rust
//! use canvist_testkit::Position;
//! use canvist_testkit::transform::TransformCase;
//!
//! TransformCase::new("bold a word")
//!     .text("Hello world")
//!     .select(&Position::new([0], 0), &Position::new([0], 5))
//!     .press("Mod-b")
//!     .expect_markdown("**Hello** world")
//!     .expect_selection(&Position::new([0], 0), &Position::new([0], 5))
//!     .run();
//! ```
//!
//! When a case fails, the message names the case, the step, and both
//! documents, so one run is enough to see why. For the other kind of test
//! — real input, real targets — see [`crate::EditorHarness`].

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use serde_json::Value as Json;

use crate::Position;
use crate::harness::strip_ids;

/// One case: a document, a script, and expectations.
#[derive(Clone, Debug)]
pub struct TransformCase {
	name: String,
	extensions: Vec<String>,
	load: (Format, String),
	steps: Vec<(Step, String)>,
	expects: Vec<Expect>,
}

/// What a step does. The names mirror the harness, so a case that grows
/// up into a cross-platform test reads the same.
#[derive(Clone, Debug)]
enum Step {
	TypeText(String),
	InsertText(String),
	Press(String),
	Select(Position, Option<Position>),
	SelectEnd(Vec<u32>),
	SelectAll,
	DeleteSelection,
	Paste(Format, String),
	Run(String, Json),
	Undo,
	Redo,
}

/// What a case checks at the end.
#[derive(Clone, Debug)]
enum Expect {
	Text(String),
	Markdown(String),
	Html(String),
	Json(Json),
	Selection(Option<(Position, Position)>),
}

impl TransformCase {
	/// A case named `name`, which failure messages quote.
	#[must_use]
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			extensions: crate::STARTER_KIT
				.iter()
				.map(|name| (*name).to_owned())
				.collect(),
			load: (Format::Text, String::new()),
			steps: Vec::new(),
			expects: Vec::new(),
		}
	}

	/// Register only the extensions named, for cases that check one
	/// extension alone or a few together.
	#[must_use]
	pub fn extensions(mut self, names: &[&str]) -> Self {
		self.extensions = names.iter().map(|name| (*name).to_owned()).collect();

		self
	}

	/// Load `content` read as `format`.
	#[must_use]
	pub fn load(mut self, format: Format, content: &str) -> Self {
		self.load = (format, content.to_owned());

		self
	}

	/// Load plain `text`: one paragraph per line.
	#[must_use]
	pub fn text(self, text: &str) -> Self {
		self.load(Format::Text, text)
	}

	/// Load `markdown`.
	#[must_use]
	pub fn markdown(self, markdown: &str) -> Self {
		self.load(Format::Markdown, markdown)
	}

	/// Load ProseMirror JSON.
	#[must_use]
	pub fn json(self, json: &Json) -> Self {
		self.load(Format::Json, &json.to_string())
	}

	/// Type `text` one character at a time, so input rules fire.
	#[must_use]
	pub fn type_text(mut self, text: &str) -> Self {
		self.push(Step::TypeText(text.to_owned()));

		self
	}

	/// Insert `text` whole, without input rules.
	#[must_use]
	pub fn insert_text(mut self, text: &str) -> Self {
		self.push(Step::InsertText(text.to_owned()));

		self
	}

	/// Press a key or chord, such as `Enter` or `Mod-b`.
	#[must_use]
	pub fn press(mut self, binding: &str) -> Self {
		self.push(Step::Press(binding.to_owned()));

		self
	}

	/// Select from `anchor` to `head`.
	#[must_use]
	pub fn select(mut self, anchor: &Position, head: &Position) -> Self {
		self.push(Step::Select(anchor.clone(), Some(head.clone())));

		self
	}

	/// Place the caret at `at`.
	#[must_use]
	pub fn caret(mut self, at: &Position) -> Self {
		self.push(Step::Select(at.clone(), None));

		self
	}

	/// Place the caret at the end of the block `path` names.
	#[must_use]
	pub fn caret_end(mut self, path: impl Into<Vec<u32>>) -> Self {
		self.push(Step::SelectEnd(path.into()));

		self
	}

	/// Select the whole document.
	#[must_use]
	pub fn select_all(mut self) -> Self {
		self.push(Step::SelectAll);

		self
	}

	/// Delete the selection.
	#[must_use]
	pub fn delete_selection(mut self) -> Self {
		self.push(Step::DeleteSelection);

		self
	}

	/// Paste `content` read as `format`.
	#[must_use]
	pub fn paste(mut self, format: Format, content: &str) -> Self {
		self.push(Step::Paste(format, content.to_owned()));

		self
	}

	/// Run the command an extension registered as `name`.
	#[must_use]
	pub fn command(mut self, name: &str, args: &Json) -> Self {
		self.push(Step::Run(name.to_owned(), args.clone()));

		self
	}

	/// Undo the last change.
	#[must_use]
	pub fn undo(mut self) -> Self {
		self.push(Step::Undo);

		self
	}

	/// Redo the last undone change.
	#[must_use]
	pub fn redo(mut self) -> Self {
		self.push(Step::Redo);

		self
	}

	/// Expect the document's plain text.
	#[must_use]
	pub fn expect_text(mut self, expected: &str) -> Self {
		self.expects.push(Expect::Text(expected.to_owned()));

		self
	}

	/// Expect the document's Markdown.
	#[must_use]
	pub fn expect_markdown(mut self, expected: &str) -> Self {
		self.expects.push(Expect::Markdown(expected.to_owned()));

		self
	}

	/// Expect the document's HTML.
	#[must_use]
	pub fn expect_html(mut self, expected: &str) -> Self {
		self.expects.push(Expect::Html(expected.to_owned()));

		self
	}

	/// Expect the document's ProseMirror JSON, ignoring block ids.
	#[must_use]
	pub fn expect_json(mut self, expected: &Json) -> Self {
		self.expects.push(Expect::Json(expected.clone()));

		self
	}

	/// Expect the selection's anchor and head.
	#[must_use]
	pub fn expect_selection(mut self, anchor: &Position, head: &Position) -> Self {
		self.expects
			.push(Expect::Selection(Some((anchor.clone(), head.clone()))));

		self
	}

	/// Expect a caret at `at`.
	#[must_use]
	pub fn expect_caret(mut self, at: &Position) -> Self {
		self.expects
			.push(Expect::Selection(Some((at.clone(), at.clone()))));

		self
	}

	/// Expect a block selection, with no anchor and head to compare.
	#[must_use]
	pub fn expect_block_selection(mut self) -> Self {
		self.expects.push(Expect::Selection(None));

		self
	}

	/// Run the case, panicking with everything a fix needs when an
	/// expectation fails.
	///
	/// # Panics
	///
	/// A step failed or an expectation did not hold; the message names the
	/// case, the step, and both documents.
	pub fn run(self) {
		if let Err(failure) = self.try_run() {
			panic!("{failure}");
		}
	}

	/// Run the case, reporting failure instead of panicking.
	///
	/// # Errors
	///
	/// A step failed or an expectation did not hold.
	pub fn try_run(self) -> Result<(), String> {
		let names: Vec<&str> = self.extensions.iter().map(String::as_str).collect();
		let engine = Engine::named(&names)
			.map_err(|error| format!("`{}`: the extensions do not build: {error}", self.name))?;
		let (format, content) = &self.load;
		let mut doc = engine
			.parse(*format, content)
			.map_err(|error| format!("`{}`: loading failed: {error}", self.name))?;

		for (index, (step, spelled)) in self.steps.iter().enumerate() {
			let outcome = apply(&mut doc, step);

			if let Err(error) = outcome {
				return Err(format!(
					"`{}` step {index} (`{spelled}`) failed: {error}",
					self.name,
				));
			}
		}

		for expect in &self.expects {
			check(&doc, expect).map_err(|difference| format!("`{}`: {difference}", self.name))?;
		}

		Ok(())
	}

	fn push(&mut self, step: Step) {
		let spelled = match &step {
			Step::TypeText(text) | Step::InsertText(text) => {
				format!("{} `{text}`", kind_of(&step))
			}
			Step::Press(binding) => format!("press `{binding}`"),
			Step::Select(anchor, head) => {
				match head {
					Some(head) => format!("select {anchor:?} to {head:?}"),
					None => format!("caret at {anchor:?}"),
				}
			}
			Step::SelectEnd(path) => format!("caret at end of {path:?}"),
			Step::SelectAll => "select all".to_owned(),
			Step::DeleteSelection => "delete selection".to_owned(),
			Step::Paste(format, content) => format!("paste {format:?} `{content}`"),
			Step::Run(name, _) => format!("run `{name}`"),
			Step::Undo => "undo".to_owned(),
			Step::Redo => "redo".to_owned(),
		};

		self.steps.push((step, spelled));
	}
}

/// Apply one step. Typing moves the clock a heartbeat per character, the
/// way it lands on a real host, so the undo history groups a word.
fn apply(doc: &mut Doc, step: &Step) -> Result<(), canvist_headless::HeadlessError> {
	match step {
		Step::TypeText(text) => {
			use canvist_headless::canvist_core::ParagraphNavigator;

			let mut clock = std::time::Duration::ZERO;
			let mut buffer = [0; 4];

			for character in text.chars() {
				let typed = character.encode_utf8(&mut buffer);
				clock += std::time::Duration::from_millis(100);
				doc.edit(clock, &mut ParagraphNavigator, |editor, env| {
					editor.insert_text(typed, env)
				})?;
			}

			Ok(())
		}
		Step::InsertText(text) => doc.insert_text(text).map(|_| ()),
		Step::Press(binding) => doc.press(binding).map(|_| ()),
		Step::Select(anchor, head) => {
			let head = head.clone().unwrap_or_else(|| anchor.clone());
			let (anchor, head) = (anchor.clone().into(), head.into());

			doc.select_range(&anchor, &head)
		}
		Step::SelectEnd(path) => {
			let path = path.clone();
			let json = doc.to_json();
			let end = crate::position::text_end(&json, &path);

			doc.select_range(
				&canvist_headless::Position::new(path.clone(), end),
				&canvist_headless::Position::new(path, end),
			)
		}
		Step::SelectAll => doc.select_all(),
		Step::DeleteSelection => doc.delete_selection().map(|_| ()),
		Step::Paste(format, content) => doc.paste(*format, content).map(|_| ()),
		Step::Run(name, args) => doc.run_named(name, args).map(|_| ()),
		Step::Undo => doc.undo().map(|_| ()),
		Step::Redo => doc.redo().map(|_| ()),
	}
}

/// Check one expectation.
fn check(doc: &Doc, expect: &Expect) -> Result<(), String> {
	match expect {
		Expect::Text(expected) => compare(&doc.export(Format::Text), &expected.clone(), "text"),
		Expect::Markdown(expected) => {
			compare(&doc.export(Format::Markdown), &expected.clone(), "markdown")
		}
		Expect::Html(expected) => compare(&doc.export(Format::Html), &expected.clone(), "html"),
		Expect::Json(expected) => {
			let actual = strip_ids(doc.to_json());
			let expected = strip_ids(expected.clone());

			if actual == expected {
				Ok(())
			} else {
				Err(format!(
					"json is\n      {actual}\n    expected\n      {expected}",
				))
			}
		}
		Expect::Selection(expected) => {
			let actual = doc
				.selected_range()
				.map(|(anchor, head)| (Position::from(anchor), Position::from(head)));

			if &actual == expected {
				Ok(())
			} else {
				Err(format!("selection is {actual:?}, expected {expected:?}"))
			}
		}
	}
}

/// Compare two strings of one format.
fn compare(actual: &str, expected: &str, format: &str) -> Result<(), String> {
	if actual == expected {
		Ok(())
	} else {
		Err(format!(
			"{format} is\n      {actual}\n    expected\n      {expected}"
		))
	}
}

/// The step kind, for messages.
fn kind_of(step: &Step) -> &'static str {
	match step {
		Step::TypeText(_) => "type",
		Step::InsertText(_) => "insert",
		_ => "step",
	}
}

/// Run one [`TransformCase`] as a test.
///
/// ```rust
/// use canvist_testkit::transform::TransformCase;
/// use canvist_testkit::transform_test;
///
/// transform_test! {
///     /// Enter splits the paragraph.
///     enter_splits, |case| case
///         .text("one two")
///         .caret(&canvist_testkit::Position::new([0], 3))
///         .press("Enter")
///         .expect_text("one\ntwo")
/// }
/// ```
#[macro_export]
macro_rules! transform_test {
	($(#[$meta:meta])* $name:ident, |$case:ident| $body:expr) => {
		$(#[$meta])*
		#[test]
		fn $name() {
			let $case = $crate::transform::TransformCase::new(stringify!($name));
			let case = $body;

			case.run();
		}
	};
}
