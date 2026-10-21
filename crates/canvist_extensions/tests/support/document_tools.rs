//! Helpers shared by the document tools' tests: an editor with the five
//! tools' typed handles, and readers for its blocks.

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;

use super::Typist;

pub fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// The plain text of every top-level block.
pub fn texts(typist: &Typist) -> Vec<String> {
	let doc = typist.editor.state().doc();
	doc.children(None)
		.iter()
		.map(|&id| {
			canvist_editor::position::selected_text(
				doc,
				Selection::text(
					Point::new(first_text(doc, id), 0),
					canvist_editor::position::end_of(doc, last_text(doc, id)),
				),
			)
			.iter()
			.filter_map(|(block, range)| {
				doc.inline(*block)
					.map(|text| text.as_str()[range.start as usize..range.end as usize].to_owned())
			})
			.collect::<Vec<_>>()
			.join("/")
		})
		.collect()
}

pub fn first_text(
	doc: &canvist_model::Document,
	id: canvist_model::BlockId,
) -> canvist_model::BlockId {
	let mut current = id;

	while let Some(&child) = doc.children(Some(current)).first() {
		current = child;
	}

	current
}

pub fn last_text(
	doc: &canvist_model::Document,
	id: canvist_model::BlockId,
) -> canvist_model::BlockId {
	let mut current = id;

	while let Some(&child) = doc.children(Some(current)).last() {
		current = child;
	}

	current
}

/// The five document tools' typed handles.
pub struct Tools {
	pub find: canvist_extensions::FindAndReplaceHandle,
	pub toc: canvist_extensions::TableOfContentsHandle,
	pub comments: canvist_extensions::CommentsHandle,
	pub drag: canvist_extensions::DragHandleHandle,
	pub clear: canvist_extensions::ClearFormattingHandle,
}

/// A StarterKit editor on `platform` with the document tools, holding
/// `doc` (ProseMirror JSON) or one empty paragraph.
pub fn tools_on(platform: canvist_editor::Platform, doc: Option<&Json>) -> (Typist, Tools) {
	let mut builder = Editor::builder();
	canvist_extensions::StarterKit::add_all(&mut builder);
	let tools = Tools {
		find: builder.add(canvist_extensions::FindAndReplace),
		toc: builder.add(canvist_extensions::TableOfContents),
		comments: builder.add(canvist_extensions::Comments),
		drag: builder.add(canvist_extensions::DragHandle),
		clear: builder.add(canvist_extensions::ClearFormatting),
	};
	let editor = builder
		.platform(platform)
		.client(1)
		.build_with(|schema, client| {
			Ok(match doc {
				Some(json) => canvist_model::Document::from_json(schema, client, json)?,
				None => canvist_model::Document::new(schema, client),
			})
		})
		.expect("the document tools build");

	(Typist::with(editor), tools)
}

pub fn tools(doc: Option<&Json>) -> (Typist, Tools) {
	tools_on(canvist_editor::Platform::Other, doc)
}

/// A document of one paragraph per entry of `texts`.
pub fn paragraphs(texts: &[&str]) -> Json {
	let content: Vec<Json> = texts
		.iter()
		.map(|text| {
			if text.is_empty() {
				json!({ "type": "paragraph" })
			} else {
				json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
			}
		})
		.collect();
	json!({ "type": "doc", "content": content })
}

pub fn apply(typist: &mut Typist, command: &canvist_editor::Command) -> bool {
	typist
		.editor
		.run(command, &mut env())
		.expect("commands run")
		.is_some()
}

pub fn can(typist: &mut Typist, command: &canvist_editor::Command) -> bool {
	typist
		.editor
		.can(command, &mut env())
		.expect("dry runs run")
}

pub fn select(typist: &mut Typist, selection: Selection) {
	typist
		.editor
		.select(selection, &mut env())
		.expect("a valid selection");
}

/// The id of top-level block `index`.
pub fn block(typist: &Typist, index: usize) -> canvist_model::BlockId {
	typist.editor.state().doc().children(None)[index]
}
