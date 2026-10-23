//! Property tests for details, callouts, and math: random edits that mix
//! their commands with typing keep every invariant and the details shape,
//! undoing everything restores the original document exactly, and random
//! documents of these blocks survive JSON, HTML, and Markdown unchanged.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::KeyPress;
use canvist_core::Slice;
use canvist_core::parse_binding;
use canvist_extensions::Callout;
use canvist_extensions::Details;
use canvist_extensions::Mathematics;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::TreeContent;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;

const TYPES: [&str; 5] = ["info", "warning", "error", "success", "blank"];

fn editor() -> Editor {
	let blocks: Vec<Arc<dyn Extension>> =
		vec![Arc::new(Details), Arc::new(Callout), Arc::new(Mathematics)];
	support::editor_with(blocks)
}

/// One thing a user does.
#[derive(Clone, Debug)]
enum Action {
	/// Type text one character at a time, so input rules apply.
	Type(&'static str),
	/// Press a key binding.
	Press(&'static str),
	/// Run a block command.
	Run(&'static str, Vec<(&'static str, Value)>),
}

fn action() -> impl Strategy<Value = Action> {
	let text = prop::sample::select(vec![
		"a",
		"b",
		" ",
		"$",
		"$$x$$",
		"$$$y$$$",
		":::warning ",
		":::",
		"é",
		"1",
	]);
	let keys = prop::sample::select(vec![
		"Enter",
		"Backspace",
		"Delete",
		"ArrowLeft",
		"ArrowRight",
		"ArrowUp",
		"ArrowDown",
		"Home",
		"End",
		"Mod-a",
		"Mod-z",
		"Mod-Shift-z",
		"Shift-ArrowLeft",
		"Shift-ArrowDown",
		"Tab",
	]);
	let kind = prop::sample::select(TYPES.to_vec());
	let latex = prop::sample::select(vec!["x^2", "\\frac{a}{b}"]);

	prop_oneof![
		5 => text.prop_map(Action::Type),
		5 => keys.prop_map(Action::Press),
		1 => Just(Action::Run("setDetails", Vec::new())),
		1 => Just(Action::Run("unsetDetails", Vec::new())),
		1 => Just(Action::Run("toggleDetailsOpen", Vec::new())),
		1 => kind.clone().prop_map(|kind| Action::Run("toggleCallout", vec![("type", Value::from(kind))])),
		1 => kind.prop_map(|kind| Action::Run("setCalloutType", vec![("type", Value::from(kind))])),
		1 => latex.clone().prop_map(|latex| Action::Run("insertInlineMath", vec![("latex", Value::from(latex))])),
		1 => latex.prop_map(|latex| Action::Run("insertBlockMath", vec![("latex", Value::from(latex))])),
	]
}

/// An editor and the time its user acts at, 10 ms per action.
struct Session {
	editor: Editor,
	now: Duration,
}

impl Session {
	fn perform(&mut self, action: &Action) {
		self.now += Duration::from_millis(10);

		match action {
			Action::Type(text) => {
				for character in text.chars() {
					let mut buffer = [0; 4];
					let mut env = Env::headless(self.now);
					self.editor
						.insert_text(character.encode_utf8(&mut buffer), &mut env)
						.expect("typing never fails");
				}
			}
			Action::Press(chord) => {
				let (key, modifiers) =
					parse_binding(chord, self.editor.platform()).expect("valid chord");
				let mut env = Env::headless(self.now);
				self.editor
					.handle_key(KeyPress::new(&key, modifiers), &mut env)
					.expect("keys never fail");
			}
			Action::Run(name, args) => {
				let args: Attrs = args.iter().cloned().collect();
				let mut env = Env::headless(self.now);
				self.editor
					.run_named(name, &args, &mut env)
					.expect("valid commands never fail");
			}
		}
	}

	/// Undo or redo until there is nothing left, returning how many steps
	/// it took.
	fn unwind(&mut self, redo: bool) -> usize {
		let mut env = Env::headless(self.now);
		let mut steps = 0;

		loop {
			let update = if redo {
				self.editor.redo(&mut env)
			} else {
				self.editor.undo(&mut env)
			};

			if update.expect("history steps apply").is_none() {
				return steps;
			}

			steps += 1;
		}
	}
}

/// Panic unless every details holds a summary first and at most one
/// content block after it, and no summary or content stands anywhere else:
/// the shape every details command and key relies on.
fn assert_details_shape(doc: &Document) {
	let schema = doc.schema();
	let name = |id| {
		doc.block(id)
			.map(|block| schema.block(block.kind()).name.to_string())
			.unwrap_or_default()
	};

	for block in doc.iter() {
		let children = doc.children(Some(block.id()));
		let kind = name(block.id());

		if kind == "details" {
			let names: Vec<String> = children.iter().map(|&id| name(id)).collect();
			assert!(
				names == ["detailsSummary"] || names == ["detailsSummary", "detailsContent"],
				"a details holds a summary and its content, not {names:?}"
			);
		} else {
			for &child in children {
				let child = name(child);
				assert!(
					child != "detailsSummary" && child != "detailsContent",
					"a {child} only stands in a details, not a {kind}"
				);
			}
		}
	}
}

/// The whole document as a slice, the shape the codecs write.
fn whole(doc: &Document) -> Slice {
	Slice::new(
		doc.children(None)
			.iter()
			.filter_map(|&id| doc.tree(id))
			.collect(),
	)
}

/// ProseMirror JSON for a slice the codecs read, without block ids.
fn slice_json(schema: &Arc<Schema>, slice: Slice) -> Json {
	fn fresh(doc: &mut Document, tree: BlockTree) -> BlockTree {
		let id = doc.allocate_id().expect("ids remain");
		let content = match tree.content {
			TreeContent::Blocks(children) => {
				TreeContent::Blocks(
					children
						.into_iter()
						.map(|child| fresh(doc, child))
						.collect(),
				)
			}
			content => content,
		};
		BlockTree {
			id,
			content,
			..tree
		}
	}

	let mut doc = Document::empty(schema.clone(), 1);
	let blocks: Vec<BlockTree> = slice
		.into_blocks()
		.into_iter()
		.map(|tree| fresh(&mut doc, tree))
		.collect();

	if !blocks.is_empty() {
		doc.apply_steps([Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])
		.expect("the codecs read valid blocks");
	}

	without_ids(doc.to_json())
}

fn without_ids(mut json: Json) -> Json {
	fn strip(node: &mut Json) {
		if let Some(attrs) = node.get_mut("attrs").and_then(Json::as_object_mut) {
			attrs.remove("id");

			if attrs.is_empty() {
				node.as_object_mut().map(|object| object.remove("attrs"));
			}
		}

		if let Some(children) = node.get_mut("content").and_then(Json::as_array_mut) {
			children.iter_mut().for_each(strip);
		}
	}

	strip(&mut json);
	json
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn block_edits_keep_invariants_and_history_round_trips(actions in prop::collection::vec(action(), 1..50)) {
		let mut session = Session { editor: editor(), now: Duration::ZERO };
		let initial = session.editor.state().doc().to_json();

		for action in &actions {
			session.perform(action);
			support::assert_valid(session.editor.state());
			assert_details_shape(session.editor.state().doc());
		}

		session.unwind(true);
		let last = session.editor.state().doc().to_json();
		let undone = session.unwind(false);
		prop_assert_eq!(session.editor.state().doc().to_json(), initial, "undo all ({} steps)", undone);

		let redone = session.unwind(true);
		prop_assert_eq!(redone, undone, "redo replays every step");
		prop_assert_eq!(session.editor.state().doc().to_json(), last);
	}

	#[test]
	fn edited_blocks_are_stable_through_html_and_markdown(actions in prop::collection::vec(action(), 1..40)) {
		let mut session = Session { editor: editor(), now: Duration::ZERO };

		for action in &actions {
			session.perform(action);
		}

		let doc = session.editor.state().doc();
		let schema = doc.schema().clone();
		let slice = whole(doc);

		// One pass normalizes what the format cannot say (collapsed
		// whitespace, `$` beside digits); after it, every pass is the same.
		let html = clipboard::to_html(&schema, &clipboard::from_html(&schema, &clipboard::to_html(&schema, &slice)));
		prop_assert_eq!(clipboard::to_html(&schema, &clipboard::from_html(&schema, &html)), html.clone());

		let markdown = clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &clipboard::to_markdown(&schema, &slice)));
		prop_assert_eq!(clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &markdown)), markdown);
	}
}

/// Words of lowercase letters, one space apart, which every format keeps
/// as they are.
fn words() -> impl Strategy<Value = String> {
	"[a-z]{1,6}( [a-z]{1,6}){0,2}"
}

/// A formula Markdown can delimit: no `$`, and no space at either end.
fn latex() -> impl Strategy<Value = String> {
	"[a-z+^={}]{1,4}( [a-z+^=]{1,4})?"
}

fn text(text: &str) -> Json {
	json!({ "type": "text", "text": text })
}

fn paragraph() -> impl Strategy<Value = Json> {
	(
		words(),
		prop::option::of((latex(), prop::option::of(words()))),
	)
		.prop_map(|(lead, math)| {
			let mut content = Vec::new();

			match math {
				None => content.push(text(&lead)),
				Some((latex, tail)) => {
					content.push(text(&format!("{lead} ")));
					content.push(json!({ "type": "inlineMath", "attrs": { "latex": latex } }));

					if let Some(tail) = tail {
						content.push(text(&format!(" {tail}")));
					}
				}
			}

			json!({ "type": "paragraph", "content": content })
		})
}

fn block() -> impl Strategy<Value = Json> {
	let leaf = prop_oneof![
		3 => paragraph(),
		1 => latex().prop_map(|latex| json!({ "type": "blockMath", "attrs": { "latex": latex } })),
	];

	leaf.prop_recursive(2, 12, 3, |inner| {
		let children = prop::collection::vec(inner, 1..3);
		prop_oneof![
			(prop::sample::select(TYPES.to_vec()), children.clone()).prop_map(
				|(kind, children)| {
					json!({ "type": "callout", "attrs": { "type": kind }, "content": children })
				}
			),
			(any::<bool>(), prop::option::of(words()), children).prop_map(
				|(open, summary, children)| {
					let summary = summary.map_or_else(
						|| json!({ "type": "detailsSummary" }),
						|summary| json!({ "type": "detailsSummary", "content": [text(&summary)] }),
					);
					json!({
						"type": "details",
						"attrs": { "open": open },
						"content": [summary, { "type": "detailsContent", "content": children }],
					})
				}
			),
		]
	})
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn block_documents_round_trip_through_every_format(blocks in prop::collection::vec(block(), 1..4)) {
		let json = json!({ "type": "doc", "content": blocks });
		let schema = editor().schema().clone();
		let doc = Document::from_json(schema.clone(), 1, &json).expect("valid JSON");

		prop_assert_eq!(&without_ids(doc.to_json()), &json, "JSON");

		let html = clipboard::to_html(&schema, &whole(&doc));
		prop_assert_eq!(&slice_json(&schema, clipboard::from_html(&schema, &html)), &json, "HTML {}", html);

		let markdown = clipboard::to_markdown(&schema, &whole(&doc));
		prop_assert_eq!(&slice_json(&schema, clipboard::from_markdown(&schema, &markdown)), &json, "Markdown {:?}", markdown);
	}
}
