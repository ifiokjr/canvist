//! Who else is editing, and where their carets are.
//!
//! Presence rides on the Yjs awareness protocol. Each peer publishes a JSON
//! state shaped like y-prosemirror's, so JavaScript cursor plugins can read
//! it:
//!
//! ```json
//! {
//!   "user": { "name": "Ifiok", "color": "#f97316" },
//!   "cursor": { "anchor": { "item": { "client": 1, "clock": 4 }, "assoc": 0 },
//!               "head": { "item": { "client": 1, "clock": 4 }, "assoc": 0 } }
//! }
//! ```
//!
//! Carets are yrs sticky indexes (Yjs relative positions) inside a block's
//! `Y.Text`, so they stay attached to the same characters while others edit.
//! `cursor` is `null` when the selection is not a text selection.

use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::Value as Json;
use serde_json::json;
use yrs::Assoc;
use yrs::BranchID;
use yrs::IndexedSequence;
use yrs::StickyIndex;
use yrs::Transact;
use yrs::sync::Awareness;
use yrs::sync::AwarenessUpdate;
use yrs::sync::time::Clock;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::Binding;
use crate::CollabError;
use crate::local::map_ref;
use crate::registry::Owner;
use crate::text::Chunks;
use crate::text::Cursor;
use crate::tree;

/// How a collaborator appears to others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
	/// The display name.
	pub name: String,
	/// A CSS color for their caret and selection, such as `#f97316`.
	pub color: String,
}

/// Another peer currently connected to the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collaborator {
	/// Their awareness client id (their yrs client id).
	pub client_id: u64,
	/// Who they are, if they said.
	pub user: Option<User>,
	/// Their selection in this peer's document, if it is a text selection in
	/// blocks this peer shows.
	pub selection: Option<Selection>,
}

/// This peer's awareness state and everyone else's.
pub struct Presence {
	awareness: Awareness,
	changed: bool,
}

impl std::fmt::Debug for Presence {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Presence")
			.field("client_id", &self.awareness.client_id())
			.field("changed", &self.changed)
			.finish_non_exhaustive()
	}
}

impl Presence {
	/// Presence for `binding`'s shared document, timed by the system clock.
	#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
	pub fn new(binding: &Binding) -> Self {
		Self::from_awareness(Awareness::new(binding.yrs().clone()))
	}

	/// Presence timed by `clock`, which returns milliseconds since the Unix
	/// epoch. WebAssembly hosts pass `Date.now`.
	pub fn with_clock(binding: &Binding, clock: impl Clock + 'static) -> Self {
		Self::from_awareness(Awareness::with_clock(binding.yrs().clone(), clock))
	}

	fn from_awareness(awareness: Awareness) -> Self {
		Self {
			awareness,
			changed: false,
		}
	}

	/// This peer's awareness client id.
	pub fn client_id(&self) -> u64 {
		self.awareness.client_id().get()
	}

	/// Publish who this peer is and where its selection is in `doc`.
	pub fn set_local(
		&mut self,
		(binding, doc): (&Binding, &Document),
		user: &User,
		selection: Option<Selection>,
	) {
		let cursor = selection.and_then(|selection| cursor_json(binding, doc, selection));
		let state = json!({
			"user": { "name": user.name, "color": user.color },
			"cursor": cursor,
		});
		self.awareness.set_local_state_raw(state.to_string());
		self.changed = true;
	}

	/// Withdraw this peer's state, as when leaving the document.
	pub fn clear_local(&mut self) {
		self.awareness.clean_local_state();
		self.changed = true;
	}

	/// Every other connected peer, with selections resolved against `doc`.
	pub fn collaborators(&self, binding: &Binding, doc: &Document) -> Vec<Collaborator> {
		let own = self.awareness.client_id();
		let mut collaborators: Vec<Collaborator> = self
			.awareness
			.iter()
			.filter(|(client, _)| *client != own)
			.filter_map(|(client, state)| {
				let state: Json = serde_json::from_str(state.data.as_deref()?).ok()?;
				Some(Collaborator {
					client_id: client.get(),
					user: user_of(&state),
					selection: selection_of(binding, doc, &state),
				})
			})
			.collect();
		collaborators.sort_by_key(|collaborator| collaborator.client_id);
		collaborators
	}

	/// Every peer's state, v1 encoded, for a newly connected peer.
	pub fn encode(&self) -> Result<Vec<u8>, CollabError> {
		Ok(self.awareness.update()?.encode_v1())
	}

	/// This peer's state if it changed since the last call, v1 encoded.
	pub fn take_local_change(&mut self) -> Result<Option<AwarenessUpdate>, CollabError> {
		if !std::mem::take(&mut self.changed) {
			return Ok(None);
		}

		let own = self.awareness.client_id();
		Ok(Some(self.awareness.update_with_clients([own])?))
	}

	/// Merge another peer's v1 encoded awareness update.
	pub fn apply(&mut self, update: &[u8]) -> Result<(), CollabError> {
		self.apply_update(AwarenessUpdate::decode_v1(update)?)
	}

	pub(crate) fn apply_update(&mut self, update: AwarenessUpdate) -> Result<(), CollabError> {
		Ok(self.awareness.apply_update(update)?)
	}

	pub(crate) fn update(&self) -> Result<AwarenessUpdate, CollabError> {
		Ok(self.awareness.update()?)
	}
}

fn cursor_json(binding: &Binding, doc: &Document, selection: Selection) -> Option<Json> {
	let Selection::Text { anchor, head } = selection else {
		return None;
	};
	let anchor = serde_json::to_value(sticky(binding, doc, anchor)?).ok()?;
	let head = serde_json::to_value(sticky(binding, doc, head)?).ok()?;
	Some(json!({ "anchor": anchor, "head": head }))
}

fn user_of(state: &Json) -> Option<User> {
	let user = state.get("user")?;
	Some(User {
		name: user.get("name")?.as_str()?.to_owned(),
		color: user.get("color")?.as_str().unwrap_or_default().to_owned(),
	})
}

fn selection_of(binding: &Binding, doc: &Document, state: &Json) -> Option<Selection> {
	let cursor = state.get("cursor")?;
	let anchor: StickyIndex = serde_json::from_value(cursor.get("anchor")?.clone()).ok()?;
	let head: StickyIndex = serde_json::from_value(cursor.get("head")?.clone()).ok()?;
	Some(Selection::text(
		point(binding, doc, &anchor)?,
		point(binding, doc, &head)?,
	))
}

/// A sticky index for `point`, attached to the character after it.
pub(crate) fn sticky(binding: &Binding, doc: &Document, point: Point) -> Option<StickyIndex> {
	let txn = binding.yrs().transact();
	let block = binding.registry().block(point.block)?;
	let text = tree::text_of(&map_ref(&txn, &block.map)?, &txn)?;
	doc.inline(point.block)?.check_offset(point.offset).ok()?;
	let index = Chunks::read(&text, &txn).utf16_index(point.offset)?;

	// The end of a text has no character after it; Yjs then anchors to the
	// text itself, which `from_type` does.
	text.sticky_index(&txn, index, Assoc::After)
		.or_else(|| Some(StickyIndex::from_type(&txn, &text, Assoc::After)))
}

/// Where `sticky` points in `doc` now.
pub(crate) fn point(binding: &Binding, doc: &Document, sticky: &StickyIndex) -> Option<Point> {
	let txn = binding.yrs().transact();
	let offset = sticky.get_offset(&txn)?;
	let BranchID::Nested(text) = offset.branch.id() else {
		return None;
	};
	let Some(Owner::Text(block)) = binding.registry().owner(&text) else {
		return None;
	};
	let mut cursor = Cursor::default();
	cursor.advance(doc.inline(block)?.as_str(), offset.index)?;
	Some(Point::new(block, cursor.utf8))
}
