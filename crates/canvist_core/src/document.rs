//! The core document model for the canvist editor.
//!
//! A [`Document`] is a tree of [`Node`]s that represents the content of a text
//! editor. The tree structure mirrors common rich-text editors:
//!
//! ```text
//! Document
//! └── Paragraph
//!     ├── TextRun { text: "Hello, ", style: normal }
//!     └── TextRun { text: "world!", style: bold }
//! ```
//!
//! # Examples
//!
//! ```
//! use canvist_core::Document;
//! use canvist_core::Position;
//! use canvist_core::Style;
//!
//! let mut doc = Document::new();
//! doc.insert_text(Position::zero(), "Hello, world!");
//!
//! assert_eq!(doc.plain_text(), "Hello, world!");
//! ```

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use serde::Deserialize;
use serde::Serialize;

use crate::Selection;
use crate::Style;
use crate::operation::StyleSnapshot;
use crate::position::Position;

/// A unique identifier for a node within a [`Document`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(u64);

impl NodeId {
	/// The root node identifier.
	pub const ROOT: Self = Self(0);
}

impl std::fmt::Display for NodeId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "node:{}", self.0)
	}
}

/// The kind of content a node holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
	/// The root document container.
	Root,

	/// A paragraph block element.
	Paragraph {
		/// Paragraph-level style overrides.
		style: Style,
	},

	/// A contiguous run of text sharing the same style.
	TextRun {
		/// The text content of this run.
		text: String,
		/// Style applied to this run.
		style: Style,
	},

	/// A horizontal rule / divider.
	HorizontalRule,
}

/// A single node in the document tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
	/// Unique identifier.
	pub id: NodeId,
	/// What kind of content this node represents.
	pub kind: NodeKind,
	/// Ordered list of child node IDs.
	pub children: Vec<NodeId>,
	/// Parent node ID ([`None`] for the root).
	pub parent: Option<NodeId>,
}

/// A rich-text document backed by a node tree.
///
/// The document manages an internal node allocator and provides high-level
/// editing operations. All mutations go through the document so that
/// collaboration and undo/redo can be layered on top.
///
/// # Examples
///
/// ```
/// use canvist_core::Document;
/// use canvist_core::Position;
///
/// let mut doc = Document::new();
/// doc.insert_text(Position::zero(), "First paragraph.");
///
/// assert_eq!(doc.node_count(), 3); // root + paragraph + text run
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
	/// All nodes indexed by their ID.
	nodes: HashMap<NodeId, Node>,
	/// Monotonically increasing counter for generating unique IDs.
	next_id: u64,
	/// Optional document title (metadata, not part of the node tree).
	title: Option<String>,
	/// Cached full plain-text projection of the document.
	plain_text_cache: String,
	/// Incremental index of text runs in document order.
	run_index: Vec<RunIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunIndexEntry {
	run_id: NodeId,
	start_char: usize,
	len_chars: usize,
}

impl Document {
	/// Create a new empty document containing only a root node.
	#[must_use]
	pub fn new() -> Self {
		let root = Node {
			id: NodeId::ROOT,
			kind: NodeKind::Root,
			children: Vec::new(),
			parent: None,
		};

		let mut nodes = HashMap::new();
		nodes.insert(NodeId::ROOT, root);

		let mut document = Self {
			nodes,
			next_id: 1,
			title: None,
			plain_text_cache: String::new(),
			run_index: Vec::new(),
		};
		document.rebuild_indexes();
		document
	}

	/// Set the document title.
	pub fn set_title(&mut self, title: impl Into<String>) {
		self.title = Some(title.into());
	}

	/// Set or clear the document title.
	pub fn set_title_opt(&mut self, title: Option<String>) {
		self.title = title;
	}

	/// Get the document title, if set.
	#[must_use]
	pub fn title(&self) -> Option<&str> {
		self.title.as_deref()
	}

	/// Return the total number of nodes (including the root).
	#[must_use]
	pub fn node_count(&self) -> usize {
		self.nodes.len()
	}

	/// Look up a node by its ID.
	#[must_use]
	pub fn node(&self, id: NodeId) -> Option<&Node> {
		self.nodes.get(&id)
	}

	/// Return the root node.
	#[must_use]
	pub fn root(&self) -> &Node {
		self.nodes
			.get(&NodeId::ROOT)
			.unwrap_or_else(|| panic!("Document is missing root node"))
	}

	/// Return a default cursor at the start of the document.
	#[must_use]
	pub fn cursor(&self) -> Selection {
		Selection::collapsed(Position::zero())
	}

	/// Insert plain text at the given position.
	///
	/// If the position falls inside an existing paragraph, the text is inserted
	/// into (or splits) the appropriate text run. If the document is empty, a
	/// new paragraph and text run are created automatically.
	pub fn insert_text(&mut self, position: Position, text: &str) {
		if self.root().children.is_empty() {
			// Create the first paragraph + text run.
			let run_id = self.alloc_id();
			let para_id = self.alloc_id();

			let run = Node {
				id: run_id,
				kind: NodeKind::TextRun {
					text: text.to_string(),
					style: Style::new(),
				},
				children: Vec::new(),
				parent: Some(para_id),
			};

			let para = Node {
				id: para_id,
				kind: NodeKind::Paragraph {
					style: Style::new(),
				},
				children: vec![run_id],
				parent: Some(NodeId::ROOT),
			};

			self.nodes.insert(run_id, run);
			self.nodes.insert(para_id, para);

			if let Some(root) = self.nodes.get_mut(&NodeId::ROOT) {
				root.children.push(para_id);
			}

			self.rebuild_indexes();
			return;
		}

		// Find the text run at the given offset and splice the text in.
		let offset = position.offset();
		if let Some((run_id, local_offset)) = self.find_run_at_offset(offset)
			&& let Some(node) = self.nodes.get_mut(&run_id)
			&& let NodeKind::TextRun {
				text: ref mut t, ..
			} = node.kind
		{
			let byte_offset = char_to_byte_offset(t, local_offset);
			t.insert_str(byte_offset, text);
			self.rebuild_indexes();
		}
	}

	/// Delete text covered by the given selection.
	pub fn delete(&mut self, selection: &Selection) {
		if selection.is_collapsed() {
			return;
		}

		let start = selection.start().offset();
		let end = selection.end().offset();
		if start >= end {
			return;
		}

		let mut dirty = false;
		for (run_id, local_start, local_end) in self.overlapping_runs(start, end) {
			if local_start >= local_end {
				continue;
			}

			if let Some(node) = self.nodes.get_mut(&run_id)
				&& let NodeKind::TextRun {
					text: ref mut t, ..
				} = node.kind
			{
				let byte_start = char_to_byte_offset(t, local_start);
				let byte_end = char_to_byte_offset(t, local_end);
				t.drain(byte_start..byte_end);
				dirty = true;
			}
		}

		if dirty {
			self.rebuild_indexes();
		}
	}

	/// Apply a style to all text runs overlapping the given selection.
	pub fn apply_style(&mut self, selection: Selection, style: &Style) {
		let start = selection.start().offset();
		let end = selection.end().offset();

		let run_ids = self.ordered_run_ids();
		let mut global_offset = 0usize;

		for run_id in run_ids {
			let run_len = self.run_text_len(run_id);
			let run_start = global_offset;
			let run_end = global_offset + run_len;

			if start < run_end
				&& end > run_start
				&& let Some(node) = self.nodes.get_mut(&run_id)
				&& let NodeKind::TextRun {
					style: ref mut s, ..
				} = node.kind
			{
				*s = s.merge(style);
			}

			global_offset = run_end;
		}
	}

	/// Capture per-run style snapshots for all text runs overlapping a selection.
	///
	/// Each returned [`StyleSnapshot`] records the run's character range and
	/// its current style *before* any format merge. This is used by
	/// [`Operation::inverse`](crate::operation::Operation::inverse) to build a
	/// [`FormatRestore`](crate::operation::Operation::FormatRestore) that can
	/// undo a format operation with full fidelity.
	#[must_use]
	pub fn run_style_snapshots(&self, selection: Selection) -> Vec<StyleSnapshot> {
		let start = selection.start().offset();
		let end = selection.end().offset();

		let mut snapshots = Vec::new();
		let mut global_offset = 0usize;

		for entry in &self.run_index {
			let run_start = entry.start_char;
			let run_end = entry.start_char + entry.len_chars;
			global_offset = run_end;

			if start >= run_end || end <= run_start {
				continue;
			}

			if let Some(node) = self.nodes.get(&entry.run_id)
				&& let NodeKind::TextRun { ref style, .. } = node.kind
			{
				snapshots.push(StyleSnapshot {
					selection: Selection::range(Position::new(run_start), Position::new(run_end)),
					style: style.clone(),
				});
			}
		}

		let _ = global_offset;
		snapshots
	}

	/// Restore text-run styles from per-run snapshots.
	///
	/// Each [`StyleSnapshot`] identifies a run by its character range and
	/// *replaces* (not merges) the run's style with the captured value.
	/// This is the apply-side counterpart of
	/// [`run_style_snapshots`](Self::run_style_snapshots).
	pub fn restore_run_styles(&mut self, snapshots: &[StyleSnapshot]) {
		for snapshot in snapshots {
			let snap_start = snapshot.selection.start().offset();
			let snap_end = snapshot.selection.end().offset();

			// Find the run whose current range matches the snapshot range.
			// In the normal undo path (inverse applied right after the forward
			// op) the run boundaries haven't changed, so an exact match on
			// start_char + len_chars is expected.
			for entry in &self.run_index {
				let run_start = entry.start_char;
				let run_end = entry.start_char + entry.len_chars;

				if run_start == snap_start && run_end == snap_end {
					if let Some(node) = self.nodes.get_mut(&entry.run_id)
						&& let NodeKind::TextRun {
							style: ref mut s, ..
						} = node.kind
					{
						*s = snapshot.style.clone();
					}
					break;
				}
			}
		}
	}

	/// Replace document text content from plain text.
	///
	/// This resets the document tree to a single paragraph with one text run
	/// containing `text` (or empty root-only state if `text` is empty).
	pub fn set_plain_text(&mut self, text: &str) {
		self.nodes.clear();
		self.next_id = 1;

		let root = Node {
			id: NodeId::ROOT,
			kind: NodeKind::Root,
			children: Vec::new(),
			parent: None,
		};
		self.nodes.insert(NodeId::ROOT, root);

		if text.is_empty() {
			self.rebuild_indexes();
		} else {
			self.insert_text(Position::zero(), text);
		}
	}

	/// Return the full plain-text content of the document.
	#[must_use]
	pub fn plain_text(&self) -> String {
		self.plain_text_cache.clone()
	}

	/// Return the total character count across all text runs.
	#[must_use]
	pub fn char_count(&self) -> usize {
		self.plain_text().chars().count()
	}

	/// Return styled text runs in document order.
	///
	/// Each entry is `(text, style, global_char_offset, char_count)` for every
	/// [`NodeKind::TextRun`] in the document.
	#[must_use]
	pub fn styled_runs(&self) -> Vec<(String, Style, usize, usize)> {
		self.run_index
			.iter()
			.filter_map(|entry| {
				let node = self.nodes.get(&entry.run_id)?;
				if let NodeKind::TextRun {
					ref text,
					ref style,
				} = node.kind
				{
					Some((
						text.clone(),
						style.clone(),
						entry.start_char,
						entry.len_chars,
					))
				} else {
					None
				}
			})
			.collect()
	}

	/// Compute a deterministic hash of semantic document state.
	#[must_use]
	pub fn state_hash(&self) -> String {
		let mut hasher = DefaultHasher::new();
		self.title.hash(&mut hasher);
		self.plain_text().hash(&mut hasher);
		format!("{:016x}", hasher.finish())
	}

	/// Upsert a node directly into the document node map.
	///
	/// If a node with the same ID already exists, it is replaced.
	/// Parent/child consistency is the caller's responsibility.
	pub fn upsert_node(&mut self, node: Node) {
		self.nodes.insert(node.id, node);
		self.rebuild_indexes();
	}

	/// Find the previous word boundary from a given character offset.
	///
	/// Uses a 3-class algorithm: whitespace, punctuation, and word characters.
	#[must_use]
	pub fn word_boundary_left(&self, offset: usize) -> usize {
		let text = self.plain_text();
		let chars: Vec<char> = text.chars().collect();
		if offset == 0 || chars.is_empty() {
			return 0;
		}
		let mut pos = offset.min(chars.len());
		while pos > 0 && chars[pos - 1].is_whitespace() {
			pos -= 1;
		}
		if pos == 0 {
			return 0;
		}
		let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
		let target_is_word = is_word(chars[pos - 1]);
		while pos > 0 {
			let ch = chars[pos - 1];
			if ch.is_whitespace() || is_word(ch) != target_is_word {
				break;
			}
			pos -= 1;
		}
		pos
	}

	/// Find the next word boundary from a given character offset.
	#[must_use]
	pub fn word_boundary_right(&self, offset: usize) -> usize {
		let text = self.plain_text();
		let chars: Vec<char> = text.chars().collect();
		let len = chars.len();
		if offset >= len || chars.is_empty() {
			return len;
		}
		let mut pos = offset;
		let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
		let target_is_word = is_word(chars[pos]);
		while pos < len {
			let ch = chars[pos];
			if ch.is_whitespace() || is_word(ch) != target_is_word {
				break;
			}
			pos += 1;
		}
		while pos < len && chars[pos].is_whitespace() {
			pos += 1;
		}
		pos
	}

	/// Find the start and end offsets of the word containing `offset`.
	#[must_use]
	pub fn word_at(&self, offset: usize) -> (usize, usize) {
		let text = self.plain_text();
		let chars: Vec<char> = text.chars().collect();
		let len = chars.len();
		if chars.is_empty() || offset >= len {
			return (len, len);
		}
		let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
		let target_is_word = is_word(chars[offset]);
		let target_is_ws = chars[offset].is_whitespace();
		let class_match = |ch: char| {
			if target_is_ws {
				ch.is_whitespace()
			} else if target_is_word {
				is_word(ch)
			} else {
				!ch.is_whitespace() && !is_word(ch)
			}
		};
		let mut start = offset;
		while start > 0 && class_match(chars[start - 1]) {
			start -= 1;
		}
		let mut end = offset;
		while end < len && class_match(chars[end]) {
			end += 1;
		}
		(start, end)
	}

	/// Remove a node directly from the document node map.
	///
	/// Root node removal is ignored.
	pub fn remove_node(&mut self, id: NodeId) {
		if id == NodeId::ROOT {
			return;
		}

		self.nodes.remove(&id);
		self.rebuild_indexes();
	}

	/// Serialize the document to a JSON string.
	///
	/// # Errors
	///
	/// Returns an error if serialization fails.
	pub fn to_json(&self) -> Result<String, serde_json::Error> {
		serde_json::to_string_pretty(self)
	}

	/// Deserialize a document from a JSON string.
	///
	/// # Errors
	///
	/// Returns an error if the JSON is invalid or doesn't match the expected
	/// schema.
	pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
		serde_json::from_str(json)
	}

	// -- internal helpers ----------------------------------------------------

	fn alloc_id(&mut self) -> NodeId {
		let id = NodeId(self.next_id);
		self.next_id += 1;
		id
	}

	/// Return an ordered list of all text run node IDs in document order.
	fn ordered_run_ids(&self) -> Vec<NodeId> {
		self.run_index.iter().map(|entry| entry.run_id).collect()
	}

	/// Return the character length of a text run.
	fn run_text_len(&self, id: NodeId) -> usize {
		self.nodes
			.get(&id)
			.and_then(|n| {
				if let NodeKind::TextRun { ref text, .. } = n.kind {
					Some(text.chars().count())
				} else {
					None
				}
			})
			.unwrap_or(0)
	}

	/// Find the text run and local character offset for a global character
	/// offset.
	fn find_run_at_offset(&self, global_offset: usize) -> Option<(NodeId, usize)> {
		if self.run_index.is_empty() {
			return None;
		}

		for entry in &self.run_index {
			let run_end = entry.start_char + entry.len_chars;
			if global_offset <= run_end {
				let local = global_offset
					.saturating_sub(entry.start_char)
					.min(entry.len_chars);
				return Some((entry.run_id, local));
			}
		}

		self.run_index
			.last()
			.map(|entry| (entry.run_id, entry.len_chars))
	}

	fn overlapping_runs(&self, start: usize, end: usize) -> Vec<(NodeId, usize, usize)> {
		self.run_index
			.iter()
			.filter_map(|entry| {
				let run_start = entry.start_char;
				let run_end = entry.start_char + entry.len_chars;
				if start >= run_end || end <= run_start {
					return None;
				}
				let local_start = start.saturating_sub(run_start);
				let local_end = end.saturating_sub(run_start).min(entry.len_chars);
				Some((entry.run_id, local_start, local_end))
			})
			.collect()
	}

	fn rebuild_indexes(&mut self) {
		self.run_index.clear();
		self.plain_text_cache.clear();

		let mut global_char = 0usize;
		let paragraphs: Vec<_> = self.root().children.clone();
		for (i, para_id) in paragraphs.iter().enumerate() {
			if i > 0 {
				self.plain_text_cache.push('\n');
				global_char += 1;
			}

			if let Some(para) = self.nodes.get(para_id) {
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& let NodeKind::TextRun { ref text, .. } = run.kind
					{
						let len_chars = text.chars().count();
						self.run_index.push(RunIndexEntry {
							run_id: run.id,
							start_char: global_char,
							len_chars,
						});
						self.plain_text_cache.push_str(text);
						global_char += len_chars;
					}
				}
			}
		}
	}
}

impl Default for Document {
	fn default() -> Self {
		Self::new()
	}
}

/// Convert a character offset to a byte offset within a string.
fn char_to_byte_offset(s: &str, char_offset: usize) -> usize {
	s.char_indices()
		.nth(char_offset)
		.map_or(s.len(), |(byte_idx, _)| byte_idx)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn new_document_has_root() {
		let doc = Document::new();
		assert_eq!(doc.node_count(), 1);
		assert_eq!(doc.root().id, NodeId::ROOT);
	}

	#[test]
	fn insert_text_creates_paragraph_and_run() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");

		assert_eq!(doc.node_count(), 3); // root + paragraph + run
		assert_eq!(doc.plain_text(), "Hello");
	}

	#[test]
	fn insert_at_offset() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello!");
		doc.insert_text(Position::new(5), " world");

		assert_eq!(doc.plain_text(), "Hello world!");
	}

	#[test]
	fn delete_range() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello, world!");

		let sel = Selection::range(Position::new(5), Position::new(7));
		doc.delete(&sel);

		assert_eq!(doc.plain_text(), "Helloworld!");
	}

	#[test]
	fn apply_style_to_selection() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");

		let sel = Selection::all(&doc);
		let style = Style::new().bold();
		doc.apply_style(sel, &style);

		// Verify the run picked up the bold style.
		let run_ids = doc.ordered_run_ids();
		assert_eq!(run_ids.len(), 1);
		let run = doc.node(run_ids[0]).unwrap();
		if let NodeKind::TextRun { ref style, .. } = run.kind {
			assert_eq!(style.font_weight, Some(crate::style::FontWeight::Bold));
		} else {
			panic!("expected text run");
		}
	}

	#[test]
	fn roundtrip_json() {
		let mut doc = Document::new();
		doc.set_title("Test Doc");
		doc.insert_text(Position::zero(), "Hello");

		let json = doc.to_json().unwrap();
		let restored = Document::from_json(&json).unwrap();

		assert_eq!(restored.title(), Some("Test Doc"));
		assert_eq!(restored.plain_text(), "Hello");
	}

	#[test]
	fn char_count() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "café");

		assert_eq!(doc.char_count(), 4);
	}
}
