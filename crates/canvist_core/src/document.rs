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

use serde::Deserialize;
use serde::Serialize;

use crate::Selection;
use crate::Style;
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

		Self {
			nodes,
			next_id: 1,
			title: None,
		}
	}

	/// Set the document title.
	pub fn set_title(&mut self, title: impl Into<String>) {
		self.title = Some(title.into());
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

			return;
		}

		// Find the text run at the given offset and splice the text in.
		let offset = position.offset();
		if let Some((run_id, local_offset)) = self.find_run_at_offset(offset)
			&& let Some(node) = self.nodes.get_mut(&run_id)
			&& let NodeKind::TextRun { text: ref mut t, .. } = node.kind
		{
			let clamped = local_offset.min(t.len());
			t.insert_str(clamped, text);
		}
	}

	/// Delete text covered by the given selection.
	pub fn delete(&mut self, selection: &Selection) {
		if selection.is_collapsed() {
			return;
		}

		let start = selection.start().offset();
		let end = selection.end().offset();

		// Walk through runs, removing the range [start..end).
		let mut remaining_start = start;
		let mut remaining_end = end;
		let run_ids = self.ordered_run_ids();

		let mut global_offset = 0usize;
		for run_id in run_ids {
			let run_len = self.run_text_len(run_id);
			let run_start = global_offset;
			let run_end = global_offset + run_len;

			if remaining_start >= run_end || remaining_end <= run_start {
				global_offset = run_end;
				continue;
			}

			let local_start = remaining_start.saturating_sub(run_start);
			let local_end = (remaining_end - run_start).min(run_len);

			if let Some(node) = self.nodes.get_mut(&run_id)
				&& let NodeKind::TextRun { text: ref mut t, .. } = node.kind
			{
				let byte_start = char_to_byte_offset(t, local_start);
				let byte_end = char_to_byte_offset(t, local_end);
				t.drain(byte_start..byte_end);
			}

			let deleted = local_end - local_start;
			remaining_start = remaining_start.max(run_end) - deleted;
			remaining_end -= deleted;
			global_offset = run_end - deleted;
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
				&& let NodeKind::TextRun { style: ref mut s, .. } = node.kind
			{
				*s = s.merge(style);
			}

			global_offset = run_end;
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

		if !text.is_empty() {
			self.insert_text(Position::zero(), text);
		}
	}

	/// Return the full plain-text content of the document.
	#[must_use]
	pub fn plain_text(&self) -> String {
		let mut out = String::new();
		let paragraphs: Vec<_> = self.root().children.clone();

		for (i, para_id) in paragraphs.iter().enumerate() {
			if i > 0 {
				out.push('\n');
			}

			if let Some(para) = self.nodes.get(para_id) {
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& let NodeKind::TextRun { ref text, .. } = run.kind
					{
						out.push_str(text);
					}
				}
			}
		}

		out
	}

	/// Return the total character count across all text runs.
	#[must_use]
	pub fn char_count(&self) -> usize {
		self.plain_text().chars().count()
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
		let mut ids = Vec::new();
		for para_id in &self.root().children.clone() {
			if let Some(para) = self.nodes.get(para_id) {
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& matches!(run.kind, NodeKind::TextRun { .. })
					{
						ids.push(run.id);
					}
				}
			}
		}
		ids
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
		let mut remaining = global_offset;

		for para_id in &self.root().children.clone() {
			if let Some(para) = self.nodes.get(para_id) {
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& let NodeKind::TextRun { ref text, .. } = run.kind
					{
						let len = text.chars().count();
						if remaining <= len {
							return Some((run.id, remaining));
						}
						remaining -= len;
					}
				}
			}
		}

		// If offset is past end, return the last run at its end.
		let ids = self.ordered_run_ids();
		ids.last().map(|&id| (id, self.run_text_len(id)))
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
