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
	///
	/// Newlines in `text` create paragraph splits — each `\n` produces a new
	/// `Paragraph` node in the document tree.
	///
	/// # Examples
	///
	/// ```
	/// use canvist_core::{Document, Position};
	///
	/// let mut doc = Document::new();
	/// doc.insert_text(Position::zero(), "Hello, world!");
	/// assert_eq!(doc.plain_text(), "Hello, world!");
	///
	/// // Newlines create paragraphs.
	/// let mut doc2 = Document::new();
	/// doc2.insert_text(Position::zero(), "First\nSecond");
	/// assert_eq!(doc2.paragraph_count(), 2);
	/// ```
	pub fn insert_text(&mut self, position: Position, text: &str) {
		if self.root().children.is_empty() {
			// Create the first paragraph + text run.
			// If the text contains newlines, split into multiple paragraphs.
			let lines: Vec<&str> = text.split('\n').collect();

			for (i, line) in lines.iter().enumerate() {
				let run_id = self.alloc_id();
				let para_id = self.alloc_id();

				let run = Node {
					id: run_id,
					kind: NodeKind::TextRun {
						text: line.to_string(),
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
					parent: Some(para_id),
				};

				self.nodes.insert(run_id, run);
				self.nodes.insert(para_id, para);

				if let Some(root) = self.nodes.get_mut(&NodeId::ROOT) {
					root.children.push(para_id);
				}

				// Fix parent reference.
				if let Some(node) = self.nodes.get_mut(&run_id) {
					node.parent = Some(para_id);
				}

				let _ = i; // used for iteration
			}

			self.rebuild_indexes();
			return;
		}

		// Find the text run at the given offset and splice the text in.
		// If the text contains newlines, we need to split the current
		// paragraph and create new ones.
		let offset = position.offset();

		if !text.contains('\n') {
			// Simple case: no newlines, just insert into the existing run.
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
			return;
		}

		// Complex case: text contains newlines. Split the run at the
		// insertion point, then create new paragraphs for each line.
		let Some((run_id, local_offset)) = self.find_run_at_offset(offset) else {
			return;
		};

		// Get the current run's text, style, and parent paragraph.
		let (run_text, run_style, para_id) = {
			let Some(node) = self.nodes.get(&run_id) else {
				return;
			};
			let NodeKind::TextRun {
				ref text,
				ref style,
			} = node.kind
			else {
				return;
			};
			(text.clone(), style.clone(), node.parent.unwrap_or(NodeId::ROOT))
		};

		let run_chars: Vec<char> = run_text.chars().collect();
		let before: String = run_chars[..local_offset].iter().collect();
		let after: String = run_chars[local_offset..].iter().collect();

		let lines: Vec<&str> = text.split('\n').collect();

		// First line: append to the text before the split point.
		let first_line = format!("{}{}", before, lines[0]);

		// Update the current run with the first line text.
		if let Some(node) = self.nodes.get_mut(&run_id)
			&& let NodeKind::TextRun {
				text: ref mut t, ..
			} = node.kind
			{
				*t = first_line;
			}

		// Find the position of the current paragraph in the root's children.
		let para_pos = self
			.root()
			.children
			.iter()
			.position(|&c| c == para_id)
			.unwrap_or(0);

		// Create new paragraphs for middle lines and the last line.
		let mut insert_after = para_pos;
		for (i, line) in lines.iter().enumerate().skip(1) {
			let new_run_id = self.alloc_id();
			let new_para_id = self.alloc_id();

			// Last line gets the remainder text appended.
			let line_text = if i == lines.len() - 1 {
				format!("{line}{after}")
			} else {
				line.to_string()
			};

			let new_run = Node {
				id: new_run_id,
				kind: NodeKind::TextRun {
					text: line_text,
					style: run_style.clone(),
				},
				children: Vec::new(),
				parent: Some(new_para_id),
			};

			let new_para = Node {
				id: new_para_id,
				kind: NodeKind::Paragraph {
					style: Style::new(),
				},
				children: vec![new_run_id],
				parent: Some(NodeId::ROOT),
			};

			self.nodes.insert(new_run_id, new_run);
			self.nodes.insert(new_para_id, new_para);

			insert_after += 1;
			if let Some(root) = self.nodes.get_mut(&NodeId::ROOT) {
				root.children.insert(insert_after, new_para_id);
			}
		}

		self.rebuild_indexes();
	}

	/// Delete text covered by the given selection.
	///
	/// When the deletion spans across a paragraph boundary (i.e. the `\n`
	/// between two paragraphs), the affected paragraphs are merged: the
	/// second paragraph's runs are appended to the first, and any empty
	/// paragraphs are removed.
	///
	/// # Examples
	///
	/// ```
	/// use canvist_core::{Document, Position, Selection};
	///
	/// let mut doc = Document::new();
	/// doc.insert_text(Position::zero(), "Hello, world!");
	/// doc.delete(&Selection::range(Position::new(5), Position::new(7)));
	/// assert_eq!(doc.plain_text(), "Helloworld!");
	/// ```
	pub fn delete(&mut self, selection: &Selection) {
		if selection.is_collapsed() {
			return;
		}

		let start = selection.start().offset();
		let end = selection.end().offset();
		if start >= end {
			return;
		}

		// Determine which paragraphs are affected by the deletion.
		// The plain text has `\n` between paragraphs. If the deletion
		// range crosses any `\n`, we need to merge paragraphs.
		let plain = self.plain_text();
		let deleted: String = plain.chars().skip(start).take(end - start).collect();
		let crosses_paragraph = deleted.contains('\n');

		// Delete text from overlapping runs.
		let runs_to_delete = self.overlapping_runs(start, end);
		let dirty = !runs_to_delete.is_empty() || crosses_paragraph;
		for (run_id, local_start, local_end) in runs_to_delete {
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
			}
		}

		if dirty {
			// If the deletion crossed paragraph boundaries, merge the
			// affected paragraphs and clean up empty ones.
			if crosses_paragraph {
				self.merge_empty_paragraphs();
			}
			self.rebuild_indexes();
		}
	}

	/// Merge adjacent paragraphs when one has been emptied by deletion.
	///
	/// After text deletion across a `\n` boundary, the second paragraph
	/// may be empty or the split may be unnecessary. This method:
	/// 1. Removes completely empty paragraphs (no runs or all-empty runs).
	/// 2. Merges a paragraph that lost its trailing content with the next
	///    paragraph by moving the next paragraph's runs into it.
	fn merge_empty_paragraphs(&mut self) {
		let para_ids: Vec<NodeId> = self.root().children.clone();
		let mut to_remove: Vec<NodeId> = Vec::new();
		let mut to_merge: Vec<(NodeId, NodeId)> = Vec::new(); // (keep, absorb)

		for (i, &pid) in para_ids.iter().enumerate() {
			let is_empty = self
				.nodes
				.get(&pid)
				.is_none_or(|p| {
					p.children.iter().all(|cid| {
						self.nodes
							.get(cid)
							.is_none_or(|n| {
								if let NodeKind::TextRun { ref text, .. } = n.kind {
									text.is_empty()
								} else {
									false
								}
							})
					})
				});

			if is_empty && para_ids.len() > 1 {
				// If this empty paragraph has a non-empty predecessor,
				// merge them (move nothing — the empty one just gets removed).
				// If the NEXT paragraph has content and this one is empty,
				// just remove the empty one.
				to_remove.push(pid);
			} else if !is_empty && i + 1 < para_ids.len() {
				// Check if the next paragraph should be merged into this one.
				let next_pid = para_ids[i + 1];
				let next_empty = self
					.nodes
					.get(&next_pid)
					.is_none_or(|p| {
						p.children.iter().all(|cid| {
							self.nodes
								.get(cid)
								.is_none_or(|n| {
									if let NodeKind::TextRun { ref text, .. } = n.kind {
										text.is_empty()
									} else {
										false
									}
								})
						})
					});

				if !next_empty && !to_remove.contains(&next_pid) {
					// Both non-empty — they were split by a newline that's now
					// been deleted. Merge the next paragraph's runs into this one.
					to_merge.push((pid, next_pid));
				}
			}
		}

		// Perform merges: move runs from the absorbed paragraph into the keeper.
		for (keep_id, absorb_id) in &to_merge {
			let absorbed_children: Vec<NodeId> = self
				.nodes
				.get(absorb_id)
				.map(|p| p.children.clone())
				.unwrap_or_default();

			// Reparent the absorbed runs.
			for &cid in &absorbed_children {
				if let Some(node) = self.nodes.get_mut(&cid) {
					node.parent = Some(*keep_id);
				}
			}

			// Append to keeper's children.
			if let Some(keeper) = self.nodes.get_mut(keep_id) {
				keeper.children.extend(absorbed_children);
			}

			// Mark absorbed paragraph for removal.
			to_remove.push(*absorb_id);
		}

		// Remove marked paragraphs.
		if !to_remove.is_empty() {
			if let Some(root) = self.nodes.get_mut(&NodeId::ROOT) {
				root.children.retain(|c| !to_remove.contains(c));
			}
			for pid in &to_remove {
				// Remove the paragraph node itself (but not its children,
				// which may have been reparented).
				self.nodes.remove(pid);
			}
		}
	}

	/// Apply a style to all text runs overlapping the given selection.
	///
	/// When the selection partially overlaps a run, the run is split at the
	/// selection boundaries so that only the selected portion receives the
	/// new style.
	///
	/// # Examples
	///
	/// ```
	/// use canvist_core::{Document, Position, Selection, Style};
	///
	/// let mut doc = Document::new();
	/// doc.insert_text(Position::zero(), "Hello world");
	///
	/// // Bold only "world".
	/// let sel = Selection::range(Position::new(6), Position::new(11));
	/// doc.apply_style(sel, &Style::new().bold());
	///
	/// let runs = doc.styled_runs();
	/// assert_eq!(runs.len(), 2);
	/// assert_eq!(runs[0].0, "Hello ");
	/// assert_eq!(runs[1].0, "world");
	/// ```
	pub fn apply_style(&mut self, selection: Selection, style: &Style) {
		let start = selection.start().offset();
		let end = selection.end().offset();

		if start >= end {
			return;
		}

		// Collect run info before mutating to avoid borrow conflicts.
		let run_infos: Vec<(NodeId, usize, usize, String, Style, Option<NodeId>)> = self
			.run_index
			.iter()
			.filter_map(|entry| {
				let run_start = entry.start_char;
				let run_end = entry.start_char + entry.len_chars;
				// Only process runs that overlap the selection.
				if start >= run_end || end <= run_start {
					return None;
				}
				let node = self.nodes.get(&entry.run_id)?;
				if let NodeKind::TextRun {
					ref text,
					ref style,
				} = node.kind
				{
					Some((
						entry.run_id,
						run_start,
						run_end,
						text.clone(),
						style.clone(),
						node.parent,
					))
				} else {
					None
				}
			})
			.collect();

		let mut dirty = false;

		for (run_id, run_start, run_end, run_text, run_style, parent_id) in run_infos {
			let sel_start_in_run = start.saturating_sub(run_start);
			let sel_end_in_run = (end - run_start).min(run_end - run_start);

			let fully_covers = sel_start_in_run == 0 && sel_end_in_run >= run_end - run_start;

			if fully_covers {
				// Selection covers the entire run — merge in place.
				if let Some(node) = self.nodes.get_mut(&run_id)
					&& let NodeKind::TextRun {
						style: ref mut s, ..
					} = node.kind
				{
					*s = s.merge(style);
					dirty = true;
				}
			} else {
				// Partial overlap — need to split the run.
				let run_chars: Vec<char> = run_text.chars().collect();
				let parent = parent_id.unwrap_or(NodeId::ROOT);

				// Find the position of this run in its parent's children list.
				let child_pos = self
					.nodes
					.get(&parent)
					.map_or(0, |p| {
						p.children
							.iter()
							.position(|&c| c == run_id)
							.unwrap_or(0)
					});

				// Build up to 3 split pieces.
				let mut new_ids: Vec<NodeId> = Vec::new();

				// Before-selection portion.
				if sel_start_in_run > 0 {
					let before_text: String =
						run_chars[..sel_start_in_run].iter().collect();
					let before_id = self.alloc_id();
					self.nodes.insert(
						before_id,
						Node {
							id: before_id,
							kind: NodeKind::TextRun {
								text: before_text,
								style: run_style.clone(),
							},
							children: Vec::new(),
							parent: Some(parent),
						},
					);
					new_ids.push(before_id);
				}

				// Selected portion (gets merged style).
				{
					let mid_text: String = run_chars
						[sel_start_in_run..sel_end_in_run]
						.iter()
						.collect();
					let mid_id = self.alloc_id();
					self.nodes.insert(
						mid_id,
						Node {
							id: mid_id,
							kind: NodeKind::TextRun {
								text: mid_text,
								style: run_style.clone().merge(style),
							},
							children: Vec::new(),
							parent: Some(parent),
						},
					);
					new_ids.push(mid_id);
				}

				// After-selection portion.
				if sel_end_in_run < run_chars.len() {
					let after_text: String =
						run_chars[sel_end_in_run..].iter().collect();
					let after_id = self.alloc_id();
					self.nodes.insert(
						after_id,
						Node {
							id: after_id,
							kind: NodeKind::TextRun {
								text: after_text,
								style: run_style,
							},
							children: Vec::new(),
							parent: Some(parent),
						},
					);
					new_ids.push(after_id);
				}

				// Replace the old run in the parent's children list.
				if let Some(parent_node) = self.nodes.get_mut(&parent) {
					parent_node.children.splice(
						child_pos..=child_pos,
						new_ids,
					);
				}

				// Remove the old run node.
				self.nodes.remove(&run_id);
				dirty = true;
			}
		}

		if dirty {
			self.rebuild_indexes();
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
	///
	/// When a format operation has split a run into multiple pieces, this
	/// method merges them back into a single run with the snapshot's style
	/// before restoring, so that undo fully reverses the split.
	pub fn restore_run_styles(&mut self, snapshots: &[StyleSnapshot]) {
		let mut needs_rebuild = false;

		for snapshot in snapshots {
			let snap_start = snapshot.selection.start().offset();
			let snap_end = snapshot.selection.end().offset();

			// Try exact match first (fast path when run wasn't split).
			let exact = self.run_index.iter().find(|entry| {
				entry.start_char == snap_start
					&& entry.start_char + entry.len_chars == snap_end
			});

			if let Some(entry) = exact {
				if let Some(node) = self.nodes.get_mut(&entry.run_id)
					&& let NodeKind::TextRun {
						style: ref mut s, ..
					} = node.kind
				{
					*s = snapshot.style.clone();
				}
				continue;
			}

			// Slow path: the run was split by apply_style. Merge all runs
			// that fall within [snap_start, snap_end) back into one run.
			let overlapping: Vec<_> = self
				.run_index
				.iter()
				.filter(|entry| {
					let rs = entry.start_char;
					let re = entry.start_char + entry.len_chars;
					rs >= snap_start && re <= snap_end
				})
				.map(|e| e.run_id)
				.collect();

			if overlapping.is_empty() {
				continue;
			}

			// Collect text from all pieces and determine the parent.
			let mut merged_text = String::new();
			let mut parent_id = None;
			for &rid in &overlapping {
				if let Some(node) = self.nodes.get(&rid) {
					if parent_id.is_none() {
						parent_id = node.parent;
					}
					if let NodeKind::TextRun { ref text, .. } = node.kind {
						merged_text.push_str(text);
					}
				}
			}

			let parent = parent_id.unwrap_or(NodeId::ROOT);

			// Create the merged run.
			let merged_id = self.alloc_id();
			self.nodes.insert(
				merged_id,
				Node {
					id: merged_id,
					kind: NodeKind::TextRun {
						text: merged_text,
						style: snapshot.style.clone(),
					},
					children: Vec::new(),
					parent: Some(parent),
				},
			);

			// Replace the split runs in the parent's children list.
			if let Some(parent_node) = self.nodes.get_mut(&parent)
				&& let Some(first_pos) = parent_node
					.children
					.iter()
					.position(|c| overlapping.contains(c))
				{
					// Remove all overlapping children and insert the merged one.
					parent_node
						.children
						.retain(|c| !overlapping.contains(c));
					parent_node.children.insert(first_pos, merged_id);
				}

			// Remove the old split nodes.
			for rid in &overlapping {
				self.nodes.remove(rid);
			}
			needs_rebuild = true;
		}

		if needs_rebuild {
			self.rebuild_indexes();
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

	/// Count the number of words in the document.
	///
	/// Words are defined as contiguous sequences of non-whitespace characters.
	/// An empty document returns 0.
	#[must_use]
	pub fn word_count(&self) -> usize {
		self.plain_text().split_whitespace().count()
	}

	/// Count the number of paragraphs in the document tree.
	///
	/// Returns the number of `Paragraph` child nodes under the root.
	/// An empty document (no paragraphs) returns 1 (implicit empty paragraph).
	#[must_use]
	pub fn paragraph_count(&self) -> usize {
		self.root().children.len().max(1)
	}

	/// Return the leading whitespace (tabs/spaces) of the line containing
	/// the given character `offset`.
	///
	/// This is used for auto-indent: when pressing Enter, the new line starts
	/// with the same indentation as the current line.
	#[must_use]
	pub fn leading_whitespace_at(&self, offset: usize) -> String {
		let text = self.plain_text();
		let chars: Vec<char> = text.chars().collect();
		// Walk backwards to find the start of the current line.
		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		// Collect leading whitespace.
		let mut ws = String::new();
		for &ch in &chars[line_start..] {
			if ch == ' ' || ch == '\t' {
				ws.push(ch);
			} else {
				break;
			}
		}
		ws
	}

	/// Find all occurrences of `needle` in the document plain text.
	///
	/// Returns a list of `(start_offset, end_offset)` for each match.
	/// When `case_sensitive` is false, matching is done on lowercased text.
	#[must_use]
	pub fn find_all(&self, needle: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
		if needle.is_empty() {
			return Vec::new();
		}
		let text = self.plain_text();
		let chars: Vec<char> = text.chars().collect();
		let haystack: String = if case_sensitive {
			text.clone()
		} else {
			text.to_lowercase()
		};
		let search: String = if case_sensitive {
			needle.to_string()
		} else {
			needle.to_lowercase()
		};
		let needle_len = search.chars().count();
		let mut results = Vec::new();
		// Walk by character offset, not byte offset, for Unicode correctness.
		let hay_chars: Vec<char> = haystack.chars().collect();
		let search_chars: Vec<char> = search.chars().collect();
		if needle_len > hay_chars.len() {
			return results;
		}
		let _ = &chars; // suppress unused
		for start in 0..=hay_chars.len() - needle_len {
			if hay_chars[start..start + needle_len] == search_chars[..] {
				results.push((start, start + needle_len));
			}
		}
		results
	}

	/// Find the next occurrence of `needle` at or after `from_offset`.
	///
	/// Wraps around to the beginning if no match is found after `from_offset`.
	/// Returns `None` if there are no matches at all.
	#[must_use]
	pub fn find_next(
		&self,
		needle: &str,
		from_offset: usize,
		case_sensitive: bool,
	) -> Option<(usize, usize)> {
		let matches = self.find_all(needle, case_sensitive);
		if matches.is_empty() {
			return None;
		}
		// Find first match at or after from_offset.
		for &(s, e) in &matches {
			if s >= from_offset {
				return Some((s, e));
			}
		}
		// Wrap around.
		Some(matches[0])
	}

	/// Find the previous occurrence of `needle` before `from_offset`.
	///
	/// Wraps around to the end if no match is found before `from_offset`.
	#[must_use]
	pub fn find_prev(
		&self,
		needle: &str,
		from_offset: usize,
		case_sensitive: bool,
	) -> Option<(usize, usize)> {
		let matches = self.find_all(needle, case_sensitive);
		if matches.is_empty() {
			return None;
		}
		// Find last match before from_offset.
		for &(s, e) in matches.iter().rev() {
			if s < from_offset {
				return Some((s, e));
			}
		}
		// Wrap around.
		matches.last().copied()
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

	/// Return the resolved style at a given character offset.
	///
	/// Finds which text run the offset falls in and returns its style. Returns
	/// the default style if no run covers the offset.
	#[must_use]
	pub fn style_at_offset(&self, offset: usize) -> Style {
		for entry in &self.run_index {
			if offset >= entry.start_char
				&& offset < entry.start_char + entry.len_chars
				&& let Some(node) = self.nodes.get(&entry.run_id)
				&& let NodeKind::TextRun { ref style, .. } = node.kind
			{
				return style.clone();
			}
		}
		Style::new()
	}

	/// Check whether **all** characters in the given range have bold applied.
	#[must_use]
	pub fn is_bold_in_range(&self, start: usize, end: usize) -> bool {
		if start >= end {
			return false;
		}
		let runs = self.styled_runs();
		for (_, style, run_start, run_len) in &runs {
			let run_end = run_start + run_len;
			if run_end <= start || *run_start >= end {
				continue;
			}
			let resolved = style.resolve();
			if resolved.font_weight.as_u16() < 700 {
				return false;
			}
		}
		true
	}

	/// Check whether **all** characters in the given range have italic applied.
	#[must_use]
	pub fn is_italic_in_range(&self, start: usize, end: usize) -> bool {
		if start >= end {
			return false;
		}
		let runs = self.styled_runs();
		for (_, style, run_start, run_len) in &runs {
			let run_end = run_start + run_len;
			if run_end <= start || *run_start >= end {
				continue;
			}
			if !style.resolve().italic {
				return false;
			}
		}
		true
	}

	/// Check whether **all** characters in the given range have underline
	/// applied.
	#[must_use]
	pub fn is_underline_in_range(&self, start: usize, end: usize) -> bool {
		if start >= end {
			return false;
		}
		let runs = self.styled_runs();
		for (_, style, run_start, run_len) in &runs {
			let run_end = run_start + run_len;
			if run_end <= start || *run_start >= end {
				continue;
			}
			if !style.resolve().underline {
				return false;
			}
		}
		true
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

	/// Import HTML content, replacing the current document.
	///
	/// Supports basic inline elements: `<strong>`/`<b>`, `<em>`/`<i>`,
	/// `<u>`, `<s>`/`<del>`, `<br>`, `<p>`, `<div>`. HTML entities
	/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&nbsp;` are decoded.
	pub fn from_html(&mut self, html: &str) {
		let segments = parse_simple_html(html);
		self.set_plain_text("");
		let mut offset = 0usize;
		for (text, bold, italic, underline, strike) in &segments {
			self.insert_text(Position::new(offset), text);
			let len = text.chars().count();
			if *bold || *italic || *underline || *strike {
				let mut style = Style::new();
				if *bold {
					style = style.bold();
				}
				if *italic {
					style = style.italic();
				}
				if *underline {
					style = style.underline();
				}
				if *strike {
					style = style.strikethrough();
				}
				let sel = Selection::range(Position::new(offset), Position::new(offset + len));
				self.apply_style(sel, &style);
			}
			offset += len;
		}
	}

	/// Export the document as HTML.
	///
	/// Each paragraph becomes a `<p>` element. Text runs with styles are
	/// wrapped in appropriate inline elements (`<strong>`, `<em>`, `<u>`,
	/// `<s>`, `<span>`).
	#[must_use]
	pub fn to_html(&self) -> String {
		let para_ids = self.root().children.clone();
		if para_ids.is_empty() {
			return String::from("<p></p>");
		}

		let mut html = String::new();

		for para_id in &para_ids {
			html.push_str("<p>");

			if let Some(para) = self.nodes.get(para_id) {
				let mut has_content = false;
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& let NodeKind::TextRun {
							ref text,
							ref style,
						} = run.kind
					{
						if text.is_empty() {
							continue;
						}
						has_content = true;
						let resolved = style.resolve();

						let escaped = html_escape(text);
						let mut content = escaped;

						if resolved.strikethrough {
							content = format!("<s>{content}</s>");
						}
						if resolved.underline {
					content = format!("<u>{content}</u>");
				}
				if resolved.italic {
					content = format!("<em>{content}</em>");
				}
				if resolved.font_weight == crate::style::FontWeight::Bold {
					content = format!("<strong>{content}</strong>");
				}

						// Font size and color via <span> if non-default.
						let mut span_styles = Vec::new();
						if (resolved.font_size - 16.0).abs() > 0.01 {
							span_styles.push(format!("font-size:{}px", resolved.font_size));
						}
						if resolved.color != crate::style::Color::BLACK {
							span_styles.push(format!(
								"color:{}",
								resolved.color.to_css()
							));
						}

						if !span_styles.is_empty() {
							content = format!(
								"<span style=\"{}\">{content}</span>",
								span_styles.join(";")
							);
						}

						html.push_str(&content);
					}
				}
				let _ = has_content;
			}

			html.push_str("</p>");
		}

		if html.is_empty() {
			String::from("<p></p>")
		} else {
			html
		}
	}

	/// Export the document as Markdown.
	///
	/// Bold text is wrapped in `**`, italic in `*`, strikethrough in `~~`.
	/// Underline has no standard Markdown equivalent and is ignored.
	#[must_use]
	pub fn to_markdown(&self) -> String {
		let para_ids = self.root().children.clone();
		if para_ids.is_empty() {
			return String::new();
		}

		let mut md = String::new();

		for (pi, para_id) in para_ids.iter().enumerate() {
			if pi > 0 {
				md.push_str("\n\n");
			}

			if let Some(para) = self.nodes.get(para_id) {
				for run_id in &para.children {
					if let Some(run) = self.nodes.get(run_id)
						&& let NodeKind::TextRun {
							ref text,
							ref style,
						} = run.kind
					{
						if text.is_empty() {
							continue;
						}
						let resolved = style.resolve();

						let mut content = text.clone();

						if resolved.strikethrough {
							content = format!("~~{content}~~");
						}
						if resolved.italic {
							content = format!("*{content}*");
						}
						if resolved.font_weight == crate::style::FontWeight::Bold {
							content = format!("**{content}**");
						}

						md.push_str(&content);
					}
				}
			}
		}

		md
	}

	/// Import a document from Markdown text.
	///
	/// Supports `**bold**`, `*italic*`, `~~strikethrough~~`, and paragraph
	/// breaks (double newline). Resets the current document content.
	///
	/// # Examples
	///
	/// ```
	/// use canvist_core::Document;
	///
	/// let mut doc = Document::new();
	/// doc.from_markdown("**Bold** and *italic*");
	///
	/// let runs = doc.styled_runs();
	/// assert!(runs.iter().any(|(t, _, _, _)| t == "Bold"));
	/// ```
	pub fn from_markdown(&mut self, md: &str) {
		let segments = parse_simple_markdown(md);

		// Collect all plain text first, then apply formatting.
		let full_text: String = segments.iter().map(|(t, _, _, _)| t.as_str()).collect();
		self.set_plain_text(&full_text);

		// Now apply formatting to each segment.
		let mut offset = 0usize;
		for (text, bold, italic, strike) in &segments {
			let len = text.chars().count();
			if *bold || *italic || *strike {
				let mut style = Style::new();
				if *bold {
					style = style.bold();
				}
				if *italic {
					style = style.italic();
				}
				if *strike {
					style = style.strikethrough();
				}
				let sel =
					Selection::range(Position::new(offset), Position::new(offset + len));
				self.apply_style(sel, &style);
			}
			offset += len;
		}
	}

	// -- internal helpers ----------------------------------------------------

	fn alloc_id(&mut self) -> NodeId {
		let id = NodeId(self.next_id);
		self.next_id += 1;
		id
	}

	/// Return an ordered list of all text run node IDs in document order.
	#[cfg_attr(not(test), allow(dead_code))]
	fn ordered_run_ids(&self) -> Vec<NodeId> {
		self.run_index.iter().map(|entry| entry.run_id).collect()
	}

	/// Return the character length of a text run.
	#[allow(dead_code)]
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
		// Merge adjacent runs with identical styles before rebuilding,
		// preventing fragmentation from repeated format/undo cycles.
		self.compact_runs();

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

	/// Merge adjacent text runs within each paragraph when they share
	/// identical styles. This prevents run fragmentation from accumulating
	/// over repeated format/undo cycles.
	fn compact_runs(&mut self) {
		let para_ids: Vec<NodeId> = self.root().children.clone();

		for para_id in para_ids {
			let children: Vec<NodeId> = self
				.nodes
				.get(&para_id)
				.map(|p| p.children.clone())
				.unwrap_or_default();

			if children.len() < 2 {
				continue;
			}

			let mut new_children: Vec<NodeId> = Vec::with_capacity(children.len());
			let mut i = 0;

			while i < children.len() {
				let current_id = children[i];

				// Collect the style and text of the current run.
				let (mut merged_text, current_style) = if let Some(Node {
						kind: NodeKind::TextRun { text, style },
						..
					}) = self.nodes.get(&current_id) { (text.clone(), style.clone()) } else {
    						new_children.push(current_id);
    						i += 1;
    						continue;
    					};

				// Look ahead and merge consecutive runs with the same style.
				let mut j = i + 1;
				let mut absorbed: Vec<NodeId> = Vec::new();

				while j < children.len() {
					let next_id = children[j];
					let same_style = self
						.nodes
						.get(&next_id)
						.is_some_and(|n| {
							if let NodeKind::TextRun { style, .. } = &n.kind {
								*style == current_style
							} else {
								false
							}
						});

					if !same_style {
						break;
					}

					// Absorb the next run's text.
					if let Some(Node {
						kind: NodeKind::TextRun { text, .. },
						..
					}) = self.nodes.get(&next_id)
					{
						merged_text.push_str(text);
						absorbed.push(next_id);
					}
					j += 1;
				}

				if absorbed.is_empty() {
					// No merge needed — keep original.
					new_children.push(current_id);
				} else {
					// Update the current run with the merged text.
					if let Some(node) = self.nodes.get_mut(&current_id)
						&& let NodeKind::TextRun { text, .. } = &mut node.kind {
							*text = merged_text;
						}
					new_children.push(current_id);

					// Remove absorbed nodes.
					for rid in &absorbed {
						self.nodes.remove(rid);
					}
				}

				i = j;
			}

			// Update the paragraph's children list.
			if let Some(para) = self.nodes.get_mut(&para_id) {
				para.children = new_children;
			}
		}

		// Remove empty text runs, but keep at least one run per paragraph
		// so that insert operations can still find a target.
		let para_ids2: Vec<NodeId> = self.root().children.clone();
		for para_id in para_ids2 {
			let children: Vec<NodeId> = self
				.nodes
				.get(&para_id)
				.map(|p| p.children.clone())
				.unwrap_or_default();

			if children.len() <= 1 {
				// Keep at least one run (even if empty) per paragraph.
				continue;
			}

			let mut non_empty: Vec<NodeId> = Vec::new();
			for cid in &children {
				let is_empty = self
					.nodes
					.get(cid)
					.is_some_and(|n| {
						if let NodeKind::TextRun { text, .. } = &n.kind {
							text.is_empty()
						} else {
							false
						}
					});

				if is_empty {
					self.nodes.remove(cid);
				} else {
					non_empty.push(*cid);
				}
			}

			// Ensure at least one child remains.
			if non_empty.is_empty() {
				// Shouldn't happen since children.len() > 1 and we'd need
				// all to be empty, but guard anyway.
				continue;
			}

			if non_empty.len() != children.len()
				&& let Some(para) = self.nodes.get_mut(&para_id) {
					para.children = non_empty;
				}
		}
	}
}

impl Default for Document {
	fn default() -> Self {
		Self::new()
	}
}

/// Strip HTML tags and extract plain text with basic formatting detection.
///
/// Returns a list of `(text, bold, italic, underline, strikethrough)` tuples.
/// This is a simple state-machine parser — not a full HTML engine.
pub fn parse_simple_html(html: &str) -> Vec<(String, bool, bool, bool, bool)> {
	let mut result = Vec::new();
	let mut bold = false;
	let mut italic = false;
	let mut underline = false;
	let mut strike = false;
	let mut buf = String::new();
	let mut chars = html.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '<' {
			// Flush current buffer.
			if !buf.is_empty() {
				result.push((std::mem::take(&mut buf), bold, italic, underline, strike));
			}
			// Read tag name.
			let mut tag = String::new();
			for tc in chars.by_ref() {
				if tc == '>' {
					break;
				}
				tag.push(tc);
			}
			let tag_lower = tag.to_lowercase();
			let tag_name = tag_lower.split_whitespace().next().unwrap_or("");
			match tag_name {
				"strong" | "b" => bold = true,
				"/strong" | "/b" => bold = false,
				"em" | "i" => italic = true,
				"/em" | "/i" => italic = false,
				"u" => underline = true,
				"/u" => underline = false,
				"s" | "del" | "strike" => strike = true,
				"/s" | "/del" | "/strike" => strike = false,
				"br" | "br/" | "br /" | "/p" | "/div" | "/li" => buf.push('\n'),
				_ => {}
			}
		} else if ch == '&' {
			// Decode HTML entities.
			let mut entity = String::new();
			for ec in chars.by_ref() {
				if ec == ';' {
					break;
				}
				entity.push(ec);
				if entity.len() > 10 {
					break;
				}
			}
			match entity.as_str() {
				"amp" => buf.push('&'),
				"lt" => buf.push('<'),
				"gt" => buf.push('>'),
				"quot" => buf.push('"'),
				"apos" => buf.push('\''),
				"nbsp" => buf.push(' '),
				_ => {
					buf.push('&');
					buf.push_str(&entity);
					buf.push(';');
				}
			}
		} else {
			buf.push(ch);
		}
	}

	if !buf.is_empty() {
		result.push((buf, bold, italic, underline, strike));
	}

	result
}

/// Parse simple Markdown into styled text segments.
///
/// Returns `(text, bold, italic, strikethrough)` tuples.
/// Supports `**bold**`, `*italic*`, `~~strikethrough~~`, and paragraph
/// breaks (double newline → single `\n`).
pub fn parse_simple_markdown(md: &str) -> Vec<(String, bool, bool, bool)> {
	let mut result = Vec::new();
	let mut bold = false;
	let mut italic = false;
	let mut strike = false;
	let mut buf = String::new();
	let mut chars = md.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '~' && chars.peek() == Some(&'~') {
			// Flush buffer.
			if !buf.is_empty() {
				result.push((std::mem::take(&mut buf), bold, italic, strike));
			}
			chars.next(); // consume second ~
			strike = !strike;
		} else if ch == '*' && chars.peek() == Some(&'*') {
			// Bold marker.
			if !buf.is_empty() {
				result.push((std::mem::take(&mut buf), bold, italic, strike));
			}
			chars.next(); // consume second *
			bold = !bold;
		} else if ch == '*' {
			// Italic marker.
			if !buf.is_empty() {
				result.push((std::mem::take(&mut buf), bold, italic, strike));
			}
			italic = !italic;
		} else if ch == '\n' && chars.peek() == Some(&'\n') {
			// Double newline → paragraph break (single \n).
			if !buf.is_empty() {
				result.push((std::mem::take(&mut buf), bold, italic, strike));
			}
			chars.next(); // consume second \n
			result.push(("\n".to_string(), false, false, false));
		} else {
			buf.push(ch);
		}
	}

	if !buf.is_empty() {
		result.push((buf, bold, italic, strike));
	}

	result
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
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

	#[test]
	fn word_boundary_left_basic() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world");
		// From end of "world" (offset 11).
		assert_eq!(doc.word_boundary_left(11), 6);
		// From start of "world" (offset 6).
		assert_eq!(doc.word_boundary_left(6), 0);
		// From middle of "hello" (offset 3).
		assert_eq!(doc.word_boundary_left(3), 0);
		// At start.
		assert_eq!(doc.word_boundary_left(0), 0);
	}

	#[test]
	fn word_boundary_right_basic() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world");
		// From start (offset 0).
		assert_eq!(doc.word_boundary_right(0), 6);
		// From offset 6 (start of "world").
		assert_eq!(doc.word_boundary_right(6), 11);
		// At end.
		assert_eq!(doc.word_boundary_right(11), 11);
	}

	#[test]
	fn word_at_selects_correct_word() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world");
		assert_eq!(doc.word_at(2), (0, 5)); // "hello"
		assert_eq!(doc.word_at(7), (6, 11)); // "world"
		assert_eq!(doc.word_at(5), (5, 6)); // whitespace
	}

	#[test]
	fn word_boundary_with_punctuation() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "foo.bar baz");
		// "foo" and "." are different classes.
		assert_eq!(doc.word_boundary_left(4), 3); // from "b" in "bar" back to "."
		assert_eq!(doc.word_boundary_right(0), 3); // "foo" then whitespace
	}

	#[test]
	fn is_bold_in_range_returns_false_for_unstyled() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		assert!(!doc.is_bold_in_range(0, 5));
	}

	#[test]
	fn is_bold_in_range_returns_true_for_bold() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.apply_style(sel, &Style::new().bold());
		assert!(doc.is_bold_in_range(0, 5));
	}

	#[test]
	fn style_at_offset_returns_correct_style() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.apply_style(sel, &Style::new().bold());
		let s = doc.style_at_offset(2);
		assert_eq!(s.font_weight, Some(crate::style::FontWeight::Bold));
	}

	#[test]
	fn word_count_empty() {
		let doc = Document::new();
		assert_eq!(doc.word_count(), 0);
	}

	#[test]
	fn word_count_single_word() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello");
		assert_eq!(doc.word_count(), 1);
	}

	#[test]
	fn word_count_multiple_words() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world foo bar");
		assert_eq!(doc.word_count(), 4);
	}

	#[test]
	fn word_count_with_newlines() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello\nworld\nfoo");
		assert_eq!(doc.word_count(), 3);
	}

	#[test]
	fn word_count_extra_whitespace() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "  hello   world  ");
		assert_eq!(doc.word_count(), 2);
	}

	#[test]
	fn paragraph_count_empty() {
		let doc = Document::new();
		assert_eq!(doc.paragraph_count(), 1);
	}

	#[test]
	fn paragraph_count_single_line() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world");
		assert_eq!(doc.paragraph_count(), 1);
	}

	#[test]
	fn paragraph_count_multiple_lines() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "line one\nline two\nline three");
		assert_eq!(doc.paragraph_count(), 3);
	}

	#[test]
	fn find_all_case_sensitive() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello hello HELLO");
		let results = doc.find_all("hello", true);
		assert_eq!(results, vec![(6, 11)]);
	}

	#[test]
	fn find_all_case_insensitive() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello hello HELLO");
		let results = doc.find_all("hello", false);
		assert_eq!(results, vec![(0, 5), (6, 11), (12, 17)]);
	}

	#[test]
	fn find_all_empty_needle_returns_empty() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		assert!(doc.find_all("", true).is_empty());
	}

	#[test]
	fn find_next_wraps_around() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "abc abc abc");
		// From offset 5 → finds "abc" at 8.
		assert_eq!(doc.find_next("abc", 5, true), Some((8, 11)));
		// From offset 9 → wraps around to (0, 3).
		assert_eq!(doc.find_next("abc", 9, true), Some((0, 3)));
	}

	#[test]
	fn find_prev_wraps_around() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "abc abc abc");
		// From offset 5 → finds "abc" at 4.
		assert_eq!(doc.find_prev("abc", 5, true), Some((4, 7)));
		// From offset 0 → wraps around to last match.
		assert_eq!(doc.find_prev("abc", 0, true), Some((8, 11)));
	}

	#[test]
	fn find_no_matches() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		assert!(doc.find_all("xyz", true).is_empty());
		assert_eq!(doc.find_next("xyz", 0, true), None);
		assert_eq!(doc.find_prev("xyz", 0, true), None);
	}

	// ── HTML export ──────────────────────────────────────────────────

	#[test]
	fn to_html_empty_document() {
		let doc = Document::new();
		assert_eq!(doc.to_html(), "<p></p>");
	}

	#[test]
	fn to_html_plain_text() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello, world!");
		assert_eq!(doc.to_html(), "<p>Hello, world!</p>");
	}

	#[test]
	fn to_html_bold_text() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "bold");
		let sel = Selection::range(Position::new(0), Position::new(4));
		doc.apply_style(sel, &Style::new().bold());
		let html = doc.to_html();
		assert!(html.contains("<strong>bold</strong>"), "got: {html}");
	}

	#[test]
	fn to_html_italic_and_bold() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "text");
		let sel = Selection::range(Position::new(0), Position::new(4));
		doc.apply_style(sel, &Style::new().bold().italic());
		let html = doc.to_html();
		assert!(html.contains("<strong>"), "got: {html}");
		assert!(html.contains("<em>"), "got: {html}");
	}

	#[test]
	fn to_html_multi_paragraph() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "para one\npara two");
		let html = doc.to_html();
		assert!(
			html.contains("</p><p>"),
			"paragraphs should be separated: {html}"
		);
	}

	#[test]
	fn to_html_escapes_special_chars() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "<script>alert('xss')</script>");
		let html = doc.to_html();
		assert!(!html.contains("<script>"), "should escape: {html}");
		assert!(html.contains("&lt;script&gt;"), "got: {html}");
	}

	#[test]
	fn to_html_font_size() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "big");
		let sel = Selection::range(Position::new(0), Position::new(3));
		doc.apply_style(sel, &Style::new().font_size(32.0));
		let html = doc.to_html();
		assert!(html.contains("font-size:32px"), "got: {html}");
	}

	// ── Markdown export ──────────────────────────────────────────────

	#[test]
	fn to_markdown_empty_document() {
		let doc = Document::new();
		assert_eq!(doc.to_markdown(), "");
	}

	#[test]
	fn to_markdown_plain_text() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello, world!");
		assert_eq!(doc.to_markdown(), "Hello, world!");
	}

	#[test]
	fn to_markdown_bold_text() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "bold");
		let sel = Selection::range(Position::new(0), Position::new(4));
		doc.apply_style(sel, &Style::new().bold());
		assert_eq!(doc.to_markdown(), "**bold**");
	}

	#[test]
	fn to_markdown_italic_text() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "italic");
		let sel = Selection::range(Position::new(0), Position::new(6));
		doc.apply_style(sel, &Style::new().italic());
		assert_eq!(doc.to_markdown(), "*italic*");
	}

	#[test]
	fn to_markdown_strikethrough() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "done");
		let sel = Selection::range(Position::new(0), Position::new(4));
		doc.apply_style(sel, &Style::new().strikethrough());
		assert_eq!(doc.to_markdown(), "~~done~~");
	}

	#[test]
	fn to_markdown_multi_paragraph() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "first\nsecond");
		let md = doc.to_markdown();
		assert!(md.contains("first\n\nsecond"), "got: {md}");
	}

	// ── HTML import ─────────────────────────────────────────────────

	#[test]
	fn from_html_plain_text() {
		let mut doc = Document::new();
		doc.from_html("<p>Hello, world!</p>");
		assert_eq!(doc.plain_text(), "Hello, world!\n");
	}

	#[test]
	fn from_html_bold() {
		let mut doc = Document::new();
		doc.from_html("<strong>bold</strong> plain");
		let text = doc.plain_text();
		assert!(text.starts_with("bold"), "got: '{text}'");
		assert!(doc.is_bold_in_range(0, 4), "bold range should be bold");
		// The " plain" segment starts at offset 4.
		let runs = doc.styled_runs();
		// Verify bold only applies to first 4 chars.
		for (run_text, style, offset, _len) in &runs {
			if *offset >= 4 {
				assert!(
					style.font_weight != Some(crate::style::FontWeight::Bold),
					"run at offset {offset} ('{run_text}') should not be bold"
				);
			}
		}
	}

	#[test]
	fn from_html_mixed_styles() {
		let mut doc = Document::new();
		doc.from_html("<p><b>bold</b> <em>italic</em> <u>underline</u></p>");
		let text = doc.plain_text();
		assert!(text.contains("bold"), "got: {text}");
		assert!(text.contains("italic"), "got: {text}");
	}

	#[test]
	fn from_html_entities() {
		let mut doc = Document::new();
		doc.from_html("<p>&lt;script&gt; &amp; &quot;test&quot;</p>");
		let text = doc.plain_text();
		assert!(
			text.contains("<script>"),
			"entities should be decoded: {text}"
		);
		assert!(text.contains('&'), "got: {text}");
		assert!(text.contains("\"test\""), "got: {text}");
	}

	#[test]
	fn from_html_multi_paragraph() {
		let mut doc = Document::new();
		doc.from_html("<p>first</p><p>second</p>");
		let text = doc.plain_text();
		assert!(
			text.contains("first\nsecond"),
			"paragraphs should be separated by newline: {text}"
		);
	}

	#[test]
	fn from_html_br_creates_newline() {
		let mut doc = Document::new();
		doc.from_html("line one<br>line two");
		let text = doc.plain_text();
		assert!(text.contains("line one\nline two"), "got: {text}");
	}

	#[test]
	fn parse_simple_html_roundtrip() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello bold world");
		let sel = Selection::range(Position::new(6), Position::new(10));
		doc.apply_style(sel, &Style::new().bold());
		let html = doc.to_html();

		let mut doc2 = Document::new();
		doc2.from_html(&html);
		let text2 = doc2.plain_text();
		assert!(
			text2.trim() == "Hello bold world"
				|| text2.contains("Hello") && text2.contains("bold") && text2.contains("world"),
			"roundtrip text mismatch: {text2}"
		);
	}

	// ── Leading whitespace / auto-indent ────────────────────────────

	#[test]
	fn leading_whitespace_no_indent() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello world");
		assert_eq!(doc.leading_whitespace_at(5), "");
	}

	#[test]
	fn leading_whitespace_with_tab() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "\thello world");
		assert_eq!(doc.leading_whitespace_at(5), "\t");
	}

	#[test]
	fn leading_whitespace_second_line() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "first\n    second");
		assert_eq!(doc.leading_whitespace_at(10), "    ");
	}

	#[test]
	fn leading_whitespace_at_start() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "  hello");
		assert_eq!(doc.leading_whitespace_at(0), "  ");
	}

	#[test]
	fn apply_style_partial_selection_splits_run() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello world");

		// Bold only "world" (offsets 6..11)
		let sel = Selection::range(Position::new(6), Position::new(11));
		doc.apply_style(sel, &Style::new().bold());

		// Should now have 2 runs: "Hello " (normal) + "world" (bold)
		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 2, "expected 2 runs after partial format");
		assert_eq!(runs[0].0, "Hello ");
		assert_eq!(runs[0].1.font_weight, None); // normal
		assert_eq!(runs[1].0, "world");
		assert_eq!(
			runs[1].1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);

		// Plain text must be preserved.
		assert_eq!(doc.plain_text(), "Hello world");
	}

	#[test]
	fn apply_style_middle_of_run_creates_three_splits() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "abcdefgh");

		// Bold only "cdef" (offsets 2..6)
		let sel = Selection::range(Position::new(2), Position::new(6));
		doc.apply_style(sel, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 3, "expected 3 runs: before, middle, after");
		assert_eq!(runs[0].0, "ab");
		assert_eq!(runs[0].1.font_weight, None);
		assert_eq!(runs[1].0, "cdef");
		assert_eq!(
			runs[1].1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);
		assert_eq!(runs[2].0, "gh");
		assert_eq!(runs[2].1.font_weight, None);

		assert_eq!(doc.plain_text(), "abcdefgh");
	}

	#[test]
	fn apply_style_full_run_no_split() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");

		// Bold the entire run
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.apply_style(sel, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 1, "full coverage should not split");
		assert_eq!(runs[0].0, "Hello");
		assert_eq!(
			runs[0].1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);
	}

	#[test]
	fn apply_style_start_of_run_creates_two_splits() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello world");

		// Bold "Hello" (offsets 0..5)
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.apply_style(sel, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 2);
		assert_eq!(runs[0].0, "Hello");
		assert_eq!(
			runs[0].1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);
		assert_eq!(runs[1].0, " world");
		assert_eq!(runs[1].1.font_weight, None);
	}

	#[test]
	fn apply_style_then_undo_restores_original() {
		use crate::EditorRuntime;

		let mut runtime = EditorRuntime::new(
			Document::new(),
			Selection::collapsed(Position::zero()),
			"test:user",
		);

		// Insert text via runtime for proper undo tracking.
		runtime
			.handle_event(crate::EditorEvent::TextInsert {
				text: "Hello world".to_string(),
			})
			.unwrap();

		// Select "world" and bold it.
		let _ = runtime.handle_event(crate::EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(6), Position::new(11)),
		});
		runtime.apply_operation(crate::operation::Operation::format(
			Selection::range(Position::new(6), Position::new(11)),
			Style::new().bold(),
		));

		// Should have 2 runs now.
		assert_eq!(runtime.document().styled_runs().len(), 2);
		assert_eq!(runtime.document().plain_text(), "Hello world");

		// Undo should restore to 1 run.
		assert!(runtime.undo());
		assert_eq!(runtime.document().plain_text(), "Hello world");

		let runs_after_undo = runtime.document().styled_runs();
		// After undo, the run should be unstyled.
		for (text, style, _, _) in &runs_after_undo {
			assert_eq!(
				style.font_weight, None,
				"after undo, run '{text}' should not be bold"
			);
		}
	}

	#[test]
	fn compact_runs_merges_adjacent_same_style() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello world");

		// Bold the whole thing then un-bold a middle section, creating
		// 3 runs: bold "Hello" | normal " wor" | bold "ld".
		let sel_all = Selection::range(Position::new(0), Position::new(11));
		doc.apply_style(sel_all, &Style::new().bold());

		// Un-bold " wor" (offsets 5..9) — creates split.
		let sel_mid = Selection::range(Position::new(5), Position::new(9));
		doc.apply_style(
			sel_mid,
			&Style::new().font_weight(crate::style::FontWeight::Normal),
		);

		// Now re-bold " wor" — all 3 runs should have bold, so
		// compact_runs should merge them back into 1.
		doc.apply_style(sel_mid, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(
			runs.len(),
			1,
			"all runs with identical bold style should merge into 1, got {} runs",
			runs.len()
		);
		assert_eq!(runs[0].0, "Hello world");
		assert_eq!(doc.plain_text(), "Hello world");
	}

	#[test]
	fn compact_runs_preserves_different_styles() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "abcdef");

		// Bold "abc", leave "def" normal.
		let sel = Selection::range(Position::new(0), Position::new(3));
		doc.apply_style(sel, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 2, "different styles should stay separate");
		assert_eq!(runs[0].0, "abc");
		assert_eq!(runs[1].0, "def");
		assert_eq!(doc.plain_text(), "abcdef");
	}

	#[test]
	fn insert_newline_creates_two_paragraphs() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello\nworld");

		assert_eq!(doc.paragraph_count(), 2);
		assert_eq!(doc.plain_text(), "hello\nworld");
	}

	#[test]
	fn insert_multiple_newlines_creates_multiple_paragraphs() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "a\nb\nc");

		assert_eq!(doc.paragraph_count(), 3);
		assert_eq!(doc.plain_text(), "a\nb\nc");
	}

	#[test]
	fn delete_across_paragraph_boundary_merges() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "hello\nworld");

		assert_eq!(doc.paragraph_count(), 2);

		// Delete the newline (offset 5..6).
		let sel = Selection::range(Position::new(5), Position::new(6));
		doc.delete(&sel);

		assert_eq!(doc.plain_text(), "helloworld");
		// Should merge back to 1 paragraph.
		assert_eq!(doc.paragraph_count(), 1);
	}

	#[test]
	fn paragraph_roundtrip_insert_delete_newline() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "ab");
		assert_eq!(doc.paragraph_count(), 1);

		// Insert newline to split.
		doc.insert_text(Position::new(1), "\n");
		assert_eq!(doc.paragraph_count(), 2);
		assert_eq!(doc.plain_text(), "a\nb");

		// Delete the newline to merge.
		let sel = Selection::range(Position::new(1), Position::new(2));
		doc.delete(&sel);
		assert_eq!(doc.paragraph_count(), 1);
		assert_eq!(doc.plain_text(), "ab");
	}

	#[test]
	fn insert_newline_in_styled_run_preserves_style() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "abcd");

		// Bold the entire run.
		let sel = Selection::range(Position::new(0), Position::new(4));
		doc.apply_style(sel, &Style::new().bold());

		// Insert newline in the middle of the bold run.
		doc.insert_text(Position::new(2), "\n");

		assert_eq!(doc.plain_text(), "ab\ncd");
		assert_eq!(doc.paragraph_count(), 2);

		// Both runs should retain bold.
		let runs = doc.styled_runs();
		assert!(runs.len() >= 2, "expected at least 2 runs, got {}", runs.len());
		for (text, style, _, _) in &runs {
			if !text.is_empty() {
				assert_eq!(
					style.font_weight,
					Some(crate::style::FontWeight::Bold),
					"run '{text}' should be bold after paragraph split"
				);
			}
		}
	}

	#[test]
	fn json_roundtrip_multi_paragraph() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello\nWorld");
		doc.apply_style(
			Selection::range(Position::new(0), Position::new(5)),
			&Style::new().bold(),
		);

		let json = doc.to_json().unwrap();
		let restored = Document::from_json(&json).unwrap();

		assert_eq!(restored.plain_text(), "Hello\nWorld");
		assert_eq!(restored.paragraph_count(), 2);

		// Bold should survive the roundtrip.
		let runs = restored.styled_runs();
		let bold_run = runs.iter().find(|(t, _, _, _)| t == "Hello");
		assert!(
			bold_run.is_some(),
			"should find 'Hello' run after JSON roundtrip"
		);
		if let Some((_, style, _, _)) = bold_run {
			assert_eq!(
				style.font_weight,
				Some(crate::style::FontWeight::Bold),
				"'Hello' should be bold after JSON roundtrip"
			);
		}
	}

	#[test]
	fn html_roundtrip_multi_paragraph_with_style() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "First\nSecond");
		doc.apply_style(
			Selection::range(Position::new(0), Position::new(5)),
			&Style::new().italic(),
		);

		let html = doc.to_html();
		assert!(html.contains("<em>"), "should have <em> tag: {html}");
		assert!(html.contains("</p><p>"), "should have paragraph break: {html}");

		// Roundtrip through from_html.
		let mut doc2 = Document::new();
		doc2.from_html(&html);
		let text = doc2.plain_text();
		assert!(
			text.contains("First") && text.contains("Second"),
			"roundtrip text: {text}"
		);
	}

	#[test]
	fn collab_sync_multi_paragraph() {
		use crate::collaboration::CollaborationSession;

		let session = CollaborationSession::new();
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Para one\nPara two\nPara three");

		session.sync_from_document(&doc);
		assert_eq!(session.text(), "Para one\nPara two\nPara three");

		// Roundtrip back to a new document.
		let mut doc2 = Document::new();
		session.sync_to_document(&mut doc2);
		assert_eq!(doc2.plain_text(), "Para one\nPara two\nPara three");
	}

	#[test]
	fn delete_entire_paragraph_content() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello\nWorld\nEnd");

		assert_eq!(doc.paragraph_count(), 3);

		// Delete "World\n" (offsets 6..12).
		let sel = Selection::range(Position::new(6), Position::new(12));
		doc.delete(&sel);

		assert_eq!(doc.plain_text(), "Hello\nEnd");
		assert_eq!(doc.paragraph_count(), 2);
	}

	#[test]
	fn format_across_paragraph_boundary() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello\nWorld");

		// Bold across the paragraph boundary (offsets 3..8 = "lo\nWo").
		let sel = Selection::range(Position::new(3), Position::new(8));
		doc.apply_style(sel, &Style::new().bold());

		// Should create styled runs that span the boundary.
		let runs = doc.styled_runs();
		let bold_runs: Vec<_> = runs
			.iter()
			.filter(|(_, s, _, _)| s.font_weight == Some(crate::style::FontWeight::Bold))
			.collect();
		assert!(
			!bold_runs.is_empty(),
			"should have bold runs spanning paragraph boundary"
		);
		assert_eq!(doc.plain_text(), "Hello\nWorld");
	}

	#[test]
	fn delete_across_multiple_paragraph_boundaries() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "AAA\nBBB\nCCC\nDDD");

		assert_eq!(doc.paragraph_count(), 4);

		// Delete "BBB\nCCC\n" (offsets 4..12).
		let sel = Selection::range(Position::new(4), Position::new(12));
		doc.delete(&sel);

		assert_eq!(doc.plain_text(), "AAA\nDDD");
		assert_eq!(doc.paragraph_count(), 2);
	}

	#[test]
	fn insert_text_at_paragraph_boundary() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello\nWorld");

		// Insert at offset 6 (start of "World").
		doc.insert_text(Position::new(6), "Beautiful ");

		assert_eq!(doc.plain_text(), "Hello\nBeautiful World");
		assert_eq!(doc.paragraph_count(), 2);
	}

	#[test]
	fn parse_markdown_basic() {
		let segments = parse_simple_markdown("Hello **bold** and *italic* text");
		let texts: Vec<_> = segments.iter().map(|(t, _, _, _)| t.as_str()).collect();
		assert!(texts.contains(&"Hello "), "should have plain text: {texts:?}");
		assert!(texts.contains(&"bold"), "should have bold text: {texts:?}");
		assert!(texts.contains(&"italic"), "should have italic text: {texts:?}");

		let bold_seg = segments.iter().find(|(t, _, _, _)| t == "bold");
		assert!(bold_seg.unwrap().1, "bold segment should have bold=true");

		let italic_seg = segments.iter().find(|(t, _, _, _)| t == "italic");
		assert!(italic_seg.unwrap().2, "italic segment should have italic=true");
	}

	#[test]
	fn from_markdown_creates_styled_document() {
		let mut doc = Document::new();
		doc.from_markdown("**Bold** and *italic*");

		let runs = doc.styled_runs();
		let bold_run = runs.iter().find(|(t, _, _, _)| t == "Bold");
		assert!(bold_run.is_some(), "should have 'Bold' run");
		assert_eq!(
			bold_run.unwrap().1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);

		let italic_run = runs.iter().find(|(t, _, _, _)| t == "italic");
		assert!(italic_run.is_some(), "should have 'italic' run");
		assert_eq!(italic_run.unwrap().1.italic, Some(true));
	}

	#[test]
	fn markdown_roundtrip() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello World");
		doc.apply_style(
			Selection::range(Position::new(0), Position::new(5)),
			&Style::new().bold(),
		);
		doc.apply_style(
			Selection::range(Position::new(6), Position::new(11)),
			&Style::new().italic(),
		);

		let md = doc.to_markdown();
		assert!(md.contains("**Hello**"), "markdown: {md}");
		assert!(md.contains("*World*"), "markdown: {md}");

		let mut doc2 = Document::new();
		doc2.from_markdown(&md);
		let text = doc2.plain_text();
		assert!(
			text.contains("Hello") && text.contains("World"),
			"roundtrip text: {text}"
		);
	}

	#[test]
	fn from_markdown_with_paragraphs() {
		let mut doc = Document::new();
		doc.from_markdown("First paragraph\n\nSecond paragraph");

		assert_eq!(doc.paragraph_count(), 2);
		assert!(doc.plain_text().contains("First paragraph"));
		assert!(doc.plain_text().contains("Second paragraph"));
	}

	#[test]
	fn empty_paragraphs_after_multiple_newlines() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "A\n\nB");

		assert_eq!(doc.paragraph_count(), 3);
		assert_eq!(doc.plain_text(), "A\n\nB");

		// The middle paragraph should be empty but present.
		let runs = doc.styled_runs();
		let texts: Vec<_> = runs.iter().map(|(t, _, _, _)| t.as_str()).collect();
		assert!(texts.contains(&"A"), "should have 'A': {texts:?}");
		assert!(texts.contains(&"B"), "should have 'B': {texts:?}");
	}

	// ── Stress / edge-case tests ────────────────────────────────────

	#[test]
	fn insert_at_out_of_bounds_offset() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");
		// Insert past end — should not panic.
		doc.insert_text(Position::new(999), " world");
		assert!(doc.plain_text().contains("Hello"));
	}

	#[test]
	fn delete_empty_document() {
		let mut doc = Document::new();
		// Should not panic on empty doc.
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.delete(&sel);
		assert_eq!(doc.plain_text(), "");
	}

	#[test]
	fn delete_past_end() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hi");
		let sel = Selection::range(Position::new(0), Position::new(100));
		doc.delete(&sel);
		assert_eq!(doc.plain_text(), "");
	}

	#[test]
	fn apply_style_to_empty_document() {
		let mut doc = Document::new();
		// Should not panic.
		let sel = Selection::range(Position::new(0), Position::new(5));
		doc.apply_style(sel, &Style::new().bold());
		assert_eq!(doc.plain_text(), "");
	}

	#[test]
	fn apply_style_past_end() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hi");
		let sel = Selection::range(Position::new(0), Position::new(100));
		doc.apply_style(sel, &Style::new().bold());
		assert_eq!(doc.plain_text(), "Hi");
	}

	#[test]
	fn rapid_insert_delete_cycles() {
		let mut doc = Document::new();
		for i in 0..100 {
			doc.insert_text(Position::zero(), &format!("Line {i}\n"));
		}
		let text = doc.plain_text();
		assert!(text.contains("Line 0"));
		assert!(text.contains("Line 99"));

		// Delete everything character by character from the end.
		for _ in 0..text.chars().count() {
			let len = doc.char_count();
			if len == 0 {
				break;
			}
			doc.delete(&Selection::range(
				Position::new(len - 1),
				Position::new(len),
			));
		}
		assert_eq!(doc.plain_text(), "");
	}

	#[test]
	fn unicode_emoji_handling() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello 🌍🎉 world");

		assert_eq!(doc.plain_text(), "Hello 🌍🎉 world");

		// Delete the emoji range (offsets 6..8).
		let sel = Selection::range(Position::new(6), Position::new(8));
		doc.delete(&sel);
		assert_eq!(doc.plain_text(), "Hello  world");
	}

	#[test]
	fn format_single_character() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Hello");

		// Bold just "H".
		let sel = Selection::range(Position::new(0), Position::new(1));
		doc.apply_style(sel, &Style::new().bold());

		let runs = doc.styled_runs();
		assert_eq!(runs.len(), 2);
		assert_eq!(runs[0].0, "H");
		assert_eq!(
			runs[0].1.font_weight,
			Some(crate::style::FontWeight::Bold)
		);
		assert_eq!(runs[1].0, "ello");
	}

	#[test]
	fn many_paragraph_splits_and_merges() {
		let mut doc = Document::new();
		// Create 50 paragraphs.
		let text: String = (0..50).map(|i| format!("Para {i}")).collect::<Vec<_>>().join("\n");
		doc.insert_text(Position::zero(), &text);
		assert_eq!(doc.paragraph_count(), 50);

		// Delete all newlines to merge into 1.
		while doc.plain_text().contains('\n') {
			if let Some(pos) = doc.plain_text().find('\n') {
				let char_offset = doc.plain_text().chars().take(pos).count();
				doc.delete(&Selection::range(
					Position::new(char_offset),
					Position::new(char_offset + 1),
				));
			}
		}
		assert_eq!(doc.paragraph_count(), 1);
		assert!(!doc.plain_text().contains('\n'));
	}

	#[test]
	fn concurrent_format_and_delete() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "ABCDEFGHIJ");

		// Bold "CDE" then delete "BCD".
		doc.apply_style(
			Selection::range(Position::new(2), Position::new(5)),
			&Style::new().bold(),
		);
		doc.delete(&Selection::range(Position::new(1), Position::new(4)));

		// Should have "A" + some styled remainder.
		assert_eq!(doc.char_count(), 7);
		assert!(doc.plain_text().starts_with('A'));
	}

	#[test]
	fn stress_html_export_empty() {
		let doc = Document::new();
		assert_eq!(doc.to_html(), "<p></p>");
	}

	#[test]
	fn stress_markdown_export_empty() {
		let doc = Document::new();
		assert_eq!(doc.to_markdown(), "");
	}

	#[test]
	fn stress_json_roundtrip_empty() {
		let doc = Document::new();
		let json = doc.to_json().unwrap();
		let restored = Document::from_json(&json).unwrap();
		assert_eq!(restored.plain_text(), "");
		assert_eq!(restored.paragraph_count(), 1);
	}
}
