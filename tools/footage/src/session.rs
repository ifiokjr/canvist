//! A StarterKit editor wired to a real layout, typing scripts one key at a
//! time and capturing a frame after each.
//!
//! A script is typed one character at a time, like the StarterKit's typing
//! harness: `⏎` Enter, `⌫` Backspace, `⇥` Tab, `←` `→` the arrows, `⏸` a
//! pause that closes the current undo group, `[Mod-b]` any binding, and `[[`
//! a literal `[`.

use std::iter::Peekable;
use std::str::Chars;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_editor::Platform;
use canvist_editor::command;
use canvist_editor::parse_binding;
use canvist_extensions::StarterKit;
use canvist_layout::DocumentLayout;
use canvist_layout::LayoutOptions;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_model::BlockId;
use canvist_model::Document;
use serde_json::json;

use crate::Error;
use crate::reel::Overlay;
use crate::reel::Reel;
use crate::reel::display_list;

/// The bundled Atkinson Hyperlegible faces every measurement uses (SIL Open
/// Font License, see `fonts/`). Regular comes first, so it is the fallback
/// for generic families.
const FONTS: [&[u8]; 4] = [
	include_bytes!("../../../crates/canvist_layout/tests/fonts/AtkinsonHyperlegible-Regular.ttf"),
	include_bytes!("../fonts/AtkinsonHyperlegible-Bold.ttf"),
	include_bytes!("../fonts/AtkinsonHyperlegible-Italic.ttf"),
	include_bytes!("../fonts/AtkinsonHyperlegible-BoldItalic.ttf"),
];

/// The time between typed keys.
const KEY_INTERVAL: Duration = Duration::from_millis(10);
/// A pause long enough to close an undo group.
const PAUSE: Duration = Duration::from_secs(1);

/// How far below the top layout keeps the document measured.
const VIEWPORT: f32 = 4_000.0;

/// A measurer that only knows the bundled faces, so every machine measures
/// the same widths.
pub fn measurer() -> ParleyMeasurer {
	let mut measurer = ParleyMeasurer::new();

	for font in FONTS {
		measurer.register_font(font.to_vec());
	}

	measurer
}

/// A StarterKit editor with Ctrl as `Mod`.
pub fn editor() -> Result<Editor, EditorError> {
	Editor::builder()
		.extensions(StarterKit::default().extensions())
		.platform(Platform::Other)
		.client(1)
		.build()
}

/// An editor, its layout, and the reel it records into.
pub struct Session {
	/// The editor being typed into.
	pub editor: Editor,
	/// The layout of the editor's document.
	pub layout: DocumentLayout,
	/// The frames captured so far.
	pub reel: Reel,
	measurer: ParleyMeasurer,
	now: Duration,
	recording: bool,
}

impl Session {
	/// A session for a fresh StarterKit document, laid out `width` pixels
	/// wide in `theme`.
	pub fn new(theme: Arc<Theme>, width: f32) -> Result<Self, Error> {
		let editor = editor()?;
		let mut measurer = measurer();
		let mut layout =
			DocumentLayout::new(editor.state().doc(), theme, LayoutOptions::new(width));
		layout.layout_all(editor.state().doc(), &mut measurer)?;

		Ok(Self {
			editor,
			layout,
			reel: Reel::new(width),
			measurer,
			now: Duration::ZERO,
			recording: false,
		})
	}

	/// The current document.
	pub fn doc(&self) -> &Document {
		self.editor.state().doc()
	}

	/// Type `script` without capturing frames, to set a scene up.
	pub fn prepare(&mut self, script: &str) -> Result<(), Error> {
		self.recording = false;
		self.type_script(script)
	}

	/// Type `script`, capturing a timed frame after every key.
	pub fn record(&mut self, script: &str) -> Result<(), Error> {
		self.recording = true;
		self.type_script(script)
	}

	/// Capture the current state as an untimed frame.
	pub fn still(&mut self, overlay: &Overlay) {
		let state = self.editor.state();
		self.reel
			.capture(state.doc(), &self.layout, Some(state.selection()), overlay);
	}

	/// Move the root-level block at `from` to `to` (counted after removal),
	/// as a drop does, and capture the result.
	pub fn move_root(
		&mut self,
		from: usize,
		to: usize,
		extra: serde_json::Value,
	) -> Result<(), Error> {
		let move_block = command(move |state| {
			state.change(EditKind::Structure, |transaction| {
				transaction.move_blocks((None, from, 1), (None, to))?;
				Ok(true)
			})
		});
		self.recording = true;
		self.timed("drop", extra, |editor, env| editor.run(&move_block, env))
	}

	/// The id of the root-level block at `index`.
	pub fn root(&self, index: usize) -> Option<BlockId> {
		self.doc().children(None).get(index).copied()
	}

	fn type_script(&mut self, script: &str) -> Result<(), Error> {
		let mut chars = script.chars().peekable();

		while let Some(next) = chars.next() {
			self.now += KEY_INTERVAL;
			self.key(next, &mut chars)?;
		}

		Ok(())
	}

	/// Press the key `next` stands for, reading the rest of a `[chord]`
	/// from `rest`.
	fn key(&mut self, next: char, rest: &mut Peekable<Chars<'_>>) -> Result<(), Error> {
		match next {
			'⏸' => {
				self.now += PAUSE;
				Ok(())
			}
			'[' if rest.next_if_eq(&'[').is_some() => self.text("["),
			'[' => {
				let chord: String = rest.by_ref().take_while(|&close| close != ']').collect();
				self.chord(&chord)
			}
			_ => self.glyph(next),
		}
	}

	fn glyph(&mut self, glyph: char) -> Result<(), Error> {
		let key = match glyph {
			'⏎' => "Enter",
			'⌫' => "Backspace",
			'⇥' => "Tab",
			'←' => "ArrowLeft",
			'→' => "ArrowRight",
			_ => {
				let mut buffer = [0; 4];
				return self.text(glyph.encode_utf8(&mut buffer));
			}
		};

		self.chord(key)
	}

	fn text(&mut self, text: &str) -> Result<(), Error> {
		let owned = text.to_owned();
		self.timed(text, json!(null), move |editor, env| {
			editor.insert_text(&owned, env)
		})
	}

	fn chord(&mut self, chord: &str) -> Result<(), Error> {
		let (key, modifiers) = parse_binding(chord, self.editor.platform())?;
		self.timed(chord, json!(null), move |editor, env| {
			editor.handle_key(KeyPress::new(&key, modifiers), env)
		})
	}

	/// Run one input, relay out, and build the display list, timing all of
	/// it: the cost from a key event to what a painter draws.
	fn timed(
		&mut self,
		input: &str,
		extra: serde_json::Value,
		run: impl FnOnce(&mut Editor, &mut Env<'_>) -> CommandResult,
	) -> Result<(), Error> {
		let start = Instant::now();
		let mut env = Env::headless(self.now);
		let update = run(&mut self.editor, &mut env)?;
		let doc = self.editor.state().doc();

		if let Some(update) = &update {
			self.layout.apply(doc, &update.commit);
		}

		self.layout.ensure(doc, &mut self.measurer, 0.0, VIEWPORT)?;
		let selection = self.editor.state().selection();
		let list = display_list(&self.layout, Some(selection));
		let micros = start.elapsed().as_secs_f64() * 1e6;

		if !self.recording {
			return Ok(());
		}

		let mut fields = json!({
			"kind": update.as_ref().map(|update| format!("{:?}", update.kind)),
			"block": caret_root_type(doc, selection),
		});

		if let (Some(fields), serde_json::Value::Object(extra)) = (fields.as_object_mut(), extra) {
			fields.extend(extra);
		}

		let overlay = Overlay {
			selection: Some(selection),
			micros: Some(micros),
			note: Some(input.to_owned()),
			peers: Vec::new(),
			extra: Some(fields),
		};
		self.reel
			.capture_list(doc, &self.layout, &list, Some(selection), &overlay);
		Ok(())
	}
}

/// The type name of the top-level block holding the caret.
fn caret_root_type(doc: &Document, selection: canvist_model::Selection) -> Option<String> {
	let mut block = crate::reel::head_of(selection)?.block;

	while let Some(parent) = doc.block(block).and_then(canvist_model::Block::parent) {
		block = parent;
	}

	let kind = doc.block(block)?.kind();
	Some(doc.schema().block(kind).name.to_string())
}
