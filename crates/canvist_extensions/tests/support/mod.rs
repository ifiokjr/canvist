//! A headless harness that types scripts into a StarterKit editor and
//! snapshots the result as JSON.
//!
//! A script is typed one character at a time, 10 ms apart. Glyphs stand
//! for keys: `⏎` Enter, `⌫` Backspace, `⌦` Delete, `⇥` Tab, `⇤` Shift-Tab,
//! `←` `→` `↑` `↓` the arrows, and `⏸` a one-second pause that closes the
//! current undo group. `[Mod-b]` presses any binding; `[[` types a `[`.

#![allow(dead_code)]

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::EditorState;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_editor::Platform;
use canvist_editor::parse_binding;
use canvist_extensions::StarterKit;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;

/// A StarterKit editor on `platform`.
pub fn editor_on(platform: Platform) -> Editor {
	Editor::builder()
		.extensions(StarterKit::default().extensions())
		.platform(platform)
		.client(1)
		.build()
		.expect("the StarterKit builds")
}

/// A StarterKit editor with Ctrl as `Mod`.
pub fn editor() -> Editor {
	editor_on(Platform::Other)
}

/// Types scripts into an editor, keeping time.
pub struct Typist {
	/// The editor being typed into.
	pub editor: Editor,
	now: Duration,
}

impl Typist {
	/// A typist for a fresh StarterKit editor.
	pub fn new() -> Self {
		Self::on(Platform::Other)
	}

	/// A typist for a fresh StarterKit editor on `platform`.
	pub fn on(platform: Platform) -> Self {
		Self {
			editor: editor_on(platform),
			now: Duration::ZERO,
		}
	}

	/// Type `script`.
	pub fn type_script(&mut self, script: &str) -> &mut Self {
		let mut chars = script.chars().peekable();

		while let Some(next) = chars.next() {
			self.now += Duration::from_millis(10);

			match next {
				'⏸' => self.now += Duration::from_secs(1),
				'[' if chars.next_if_eq(&'[').is_some() => self.text("["),
				'[' => {
					let chord: String = chars.by_ref().take_while(|&close| close != ']').collect();
					self.chord(&chord);
				}
				_ => self.glyph(next),
			}
		}

		self
	}

	fn glyph(&mut self, glyph: char) {
		let key = match glyph {
			'⏎' => "Enter",
			'⌫' => "Backspace",
			'⌦' => "Delete",
			'⇥' => "Tab",
			'⇤' => "Shift-Tab",
			'←' => "ArrowLeft",
			'→' => "ArrowRight",
			'↑' => "ArrowUp",
			'↓' => "ArrowDown",
			_ => {
				let mut buffer = [0; 4];
				self.text(glyph.encode_utf8(&mut buffer));
				return;
			}
		};

		self.chord(key);
	}

	fn text(&mut self, text: &str) {
		let env = Env::headless(self.now);
		self.editor
			.insert_text(text, &env)
			.expect("typing never fails");
	}

	/// Press a binding such as `Mod-Shift-z`.
	pub fn chord(&mut self, chord: &str) {
		let (key, modifiers) = parse_binding(chord, self.editor.platform()).expect("valid chord");
		let env = Env::headless(self.now);
		self.editor
			.handle_key(KeyPress::new(&key, modifiers), &env)
			.expect("keys never fail");
	}

	/// Paste plain text.
	pub fn paste(&mut self, text: &str) {
		self.now += Duration::from_millis(10);
		let env = Env::headless(self.now);
		self.editor
			.paste_text(text, &env)
			.expect("pasting never fails");
	}

	/// Undo or redo until there is nothing left, returning how many steps
	/// it took.
	pub fn unwind(&mut self, redo: bool) -> usize {
		let env = Env::headless(self.now);
		let mut steps = 0;

		loop {
			let update = if redo {
				self.editor.redo(&env)
			} else {
				self.editor.undo(&env)
			};

			if update.expect("history steps apply").is_none() {
				return steps;
			}

			steps += 1;
		}
	}

	/// The document and selection as JSON, without block ids.
	pub fn snapshot(&self) -> Json {
		snapshot(self.editor.state())
	}
}

/// Type `script` into a fresh StarterKit editor and snapshot the result.
pub fn typed(script: &str) -> Json {
	Typist::new().type_script(script).snapshot()
}

/// The document as ProseMirror JSON without ids, plus the selection
/// written with block paths such as `1.0.0@3`.
pub fn snapshot(state: &EditorState) -> Json {
	let doc = state.doc();
	let mut content = doc.to_json();
	strip_ids(&mut content);

	json!({
		"doc": content["content"],
		"selection": selection(doc, state.selection()),
	})
}

fn strip_ids(node: &mut Json) {
	if let Some(attrs) = node.get_mut("attrs").and_then(Json::as_object_mut) {
		attrs.remove("id");
	}

	let empty = node
		.get("attrs")
		.and_then(Json::as_object)
		.is_some_and(serde_json::Map::is_empty);

	if let (true, Some(object)) = (empty, node.as_object_mut()) {
		object.remove("attrs");
	}

	if let Some(children) = node.get_mut("content").and_then(Json::as_array_mut) {
		children.iter_mut().for_each(strip_ids);
	}
}

fn path(doc: &Document, id: BlockId) -> String {
	let mut indexes: Vec<String> = canvist_editor::position::ancestors_or_self(doc, id)
		.filter_map(|block| doc.index_in_parent(block))
		.map(|index| index.to_string())
		.collect();
	indexes.reverse();
	indexes.join(".")
}

fn point(doc: &Document, point: Point) -> String {
	format!("{}@{}", path(doc, point.block), point.offset)
}

fn selection(doc: &Document, selection: Selection) -> String {
	match selection {
		Selection::Text { anchor, head } if anchor == head => format!("caret {}", point(doc, head)),
		Selection::Text { anchor, head } => {
			format!("text {} → {}", point(doc, anchor), point(doc, head))
		}
		Selection::Blocks { anchor, head } => {
			format!("blocks {} → {}", path(doc, anchor), path(doc, head))
		}
	}
}

/// Panic unless the document structure, the text runs, and the selection
/// are all valid.
pub fn assert_valid(state: &EditorState) {
	let doc = state.doc();
	let mut reachable = 0;
	let mut stack: Vec<Option<BlockId>> = vec![None];

	while let Some(parent) = stack.pop() {
		let parent_kind = parent
			.and_then(|id| doc.block(id))
			.map(canvist_model::Block::kind);

		for &child in doc.children(parent) {
			let block = doc.block(child).expect("child ids resolve");
			reachable += 1;
			assert_eq!(block.parent(), parent, "parent pointers match");
			assert!(
				doc.schema().allows_child(parent_kind, block.kind()),
				"the schema allows each child"
			);
			stack.push(Some(child));
		}
	}

	assert_eq!(
		reachable,
		doc.block_count(),
		"every block is reachable once"
	);
	assert!(
		!doc.children(None).is_empty(),
		"the document is never empty"
	);

	for text in doc.text_blocks().filter_map(canvist_model::Block::inline) {
		let covered: u32 = text.runs().map(|run| run.range.end - run.range.start).sum();
		assert_eq!(covered, text.len(), "runs cover the text");
	}

	let selection = state.selection();
	assert_eq!(
		canvist_editor::position::valid_selection(doc, selection),
		selection,
		"the selection is valid"
	);
}
