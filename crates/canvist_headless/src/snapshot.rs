//! Snapshots: a document's content and selection as plain JSON.

use serde_json::Value as Json;
use serde_json::json;

use crate::HeadlessError;
use crate::Position;

/// The snapshot format this version writes and reads.
const VERSION: u64 = 1;

/// A document's content, with its block ids, and its selection, taken by
/// [`Doc::snapshot`](crate::Doc::snapshot) and turned back into a document
/// by [`Engine::restore`](crate::Engine::restore).
///
/// [`Snapshot::to_json`] gives a self-describing JSON value to store in a
/// database or send to another process; every binding reads the same
/// shape:
///
/// ```json
/// { "version": 1, "content": { "type": "doc", ... }, "selection": {
///   "anchor": { "path": [0], "offset": 2 }, "head": { "path": [0], "offset": 2 } } }
/// ```
///
/// A snapshot has no collaboration history. To persist a shared document,
/// store its Yjs state instead, which peers can still merge with.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
	content: Json,
	selection: Option<(Position, Position)>,
}

impl Snapshot {
	pub(crate) fn new(content: Json, selection: Option<(Position, Position)>) -> Self {
		Self { content, selection }
	}

	/// The ProseMirror JSON content, with block ids.
	#[must_use]
	pub fn content(&self) -> &Json {
		&self.content
	}

	/// The anchor and head of the selection, if it was a text selection.
	#[must_use]
	pub fn selection(&self) -> Option<(&Position, &Position)> {
		self.selection.as_ref().map(|(anchor, head)| (anchor, head))
	}

	/// The snapshot as JSON.
	#[must_use]
	pub fn to_json(&self) -> Json {
		let selection = self.selection.as_ref().map(
			|(anchor, head)| json!({ "anchor": position_to_json(anchor), "head": position_to_json(head) }),
		);

		json!({ "version": VERSION, "content": self.content, "selection": selection })
	}

	/// Read a snapshot written by [`Snapshot::to_json`].
	pub fn from_json(json: &Json) -> Result<Self, HeadlessError> {
		let invalid = |reason: &str| HeadlessError::Snapshot(reason.to_owned());

		if json.get("version").and_then(Json::as_u64) != Some(VERSION) {
			return Err(invalid("expected version 1"));
		}

		let content = json
			.get("content")
			.filter(|content| content.is_object())
			.ok_or_else(|| invalid("expected a `content` document"))?;
		let selection = match json.get("selection") {
			None | Some(Json::Null) => None,
			Some(selection) => {
				let side = |name| {
					selection
						.get(name)
						.and_then(position_from_json)
						.ok_or_else(|| invalid("expected `anchor` and `head` positions"))
				};
				Some((side("anchor")?, side("head")?))
			}
		};

		Ok(Self {
			content: content.clone(),
			selection,
		})
	}
}

/// A position as `{ "path": [...], "offset": n }`.
pub(crate) fn position_to_json(position: &Position) -> Json {
	json!({ "path": position.path, "offset": position.offset })
}

/// Read `{ "path": [...], "offset": n }`.
pub(crate) fn position_from_json(json: &Json) -> Option<Position> {
	let path = json
		.get("path")?
		.as_array()?
		.iter()
		.map(|index| index.as_u64().and_then(|index| u32::try_from(index).ok()))
		.collect::<Option<Vec<u32>>>()?;
	let offset = u32::try_from(json.get("offset")?.as_u64()?).ok()?;

	Some(Position { path, offset })
}
