//! Accessibility through AccessKit: the document as a multiline text input
//! whose children are its text blocks, each split into one text run per
//! line with character positions, so screen readers read, navigate, and
//! select text as in a native text view.
//!
//! The tree covers the blocks on screen plus the blocks holding the
//! selection's ends, not the whole document: a long document would
//! otherwise rebuild thousands of nodes on every keystroke. Moving the caret
//! scrolls the view, which brings the next blocks into the tree.

use std::ops::Range;

use accesskit::Action;
use accesskit::ActionData;
use accesskit::ActionRequest;
use accesskit::Node;
use accesskit::NodeId;
use accesskit::Role;
use accesskit::TextPosition;
use accesskit::TextSelection;
use accesskit::Toggled;
use accesskit::TreeId;
use accesskit::TreeInfo;
use accesskit::TreeUpdate;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_headless::canvist_model::Value;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::Rect;
use kurbo::Affine;
use rustc_hash::FxHashMap;
use unicode_segmentation::UnicodeSegmentation;

use crate::DesktopEditor;
use crate::DesktopResult;
use crate::Response;

/// The window node of [`DesktopEditor::accessibility_tree`].
pub const WINDOW_ID: NodeId = NodeId(1);
/// The editor's own node, the document.
pub const EDITOR_ID: NodeId = NodeId(2);
/// Ids from here up are block and line nodes.
const FIRST_DYNAMIC_ID: u64 = 1 << 32;

/// A line's text run as it was last described: which block and byte offsets
/// its characters start at, so selections and actions can be mapped back.
#[derive(Clone, Debug)]
struct RunInfo {
	block: BlockId,
	/// The byte offset of each character, plus the end.
	offsets: Vec<u32>,
}

/// One visual line of a text block, to describe as a text run.
struct Line<'a> {
	block: BlockId,
	/// The whole block's text.
	text: &'a str,
	/// The line's byte range of `text`.
	range: Range<u32>,
	/// Whether the line ends the block.
	last: bool,
}

/// Each grapheme's byte offset, UTF-8 length, position, and width, as
/// AccessKit describes a text run's characters.
#[derive(Default)]
struct Graphemes {
	offsets: Vec<u32>,
	lengths: Vec<u8>,
	positions: Vec<f32>,
	widths: Vec<f32>,
}

impl Graphemes {
	/// Add the line break that ends a block: one more character, of no
	/// width, where the last one ends.
	fn push_break(&mut self) {
		let end = match (self.positions.last(), self.widths.last()) {
			(Some(position), Some(width)) => position + width,
			_ => 0.0,
		};
		self.lengths.push(1);
		self.positions.push(end);
		self.widths.push(0.0);
	}
}

/// Stable node ids for blocks and lines, and the runs last described.
#[derive(Clone, Debug, Default)]
pub(crate) struct Accessibility {
	ids: FxHashMap<(BlockId, u32), NodeId>,
	next: u64,
	runs: FxHashMap<NodeId, RunInfo>,
}

impl Accessibility {
	/// The node id of `block` (`line` 0) or of its line `line - 1`.
	fn id(&mut self, block: BlockId, line: u32) -> NodeId {
		let next = &mut self.next;
		*self.ids.entry((block, line)).or_insert_with(|| {
			*next += 1;
			NodeId(FIRST_DYNAMIC_ID + *next)
		})
	}

	/// The run and character index of `point`. At a soft wrap, the offset
	/// that ends one line also starts the next; the next line wins, as the
	/// caret is drawn there.
	fn position(&self, point: Point) -> Option<TextPosition> {
		let mut end_of_line = None;

		for (&node, run) in &self.runs {
			let (Some(&first), Some(&last)) = (run.offsets.first(), run.offsets.last()) else {
				continue;
			};

			if run.block != point.block || point.offset < first || point.offset > last {
				continue;
			}

			let position = TextPosition {
				node,
				character_index: run.offsets.partition_point(|&offset| offset < point.offset),
			};

			if point.offset < last || first == last {
				return Some(position);
			}

			end_of_line = Some(position);
		}

		end_of_line
	}

	/// The model point of `position`.
	fn point(&self, position: TextPosition) -> Option<Point> {
		let run = self.runs.get(&position.node)?;
		let index = position.character_index.min(run.offsets.len() - 1);
		Some(Point::new(run.block, run.offsets[index]))
	}
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// A whole accessibility tree for a window that holds only this editor:
	/// a window node named `title` whose one child is the document.
	/// `to_window` maps the editor's logical coordinates to the window's
	/// physical pixels, as for painting.
	pub fn accessibility_tree(&mut self, title: &str, to_window: Affine) -> TreeUpdate {
		let mut nodes = Vec::new();
		let editor = self.accessibility_nodes(to_window, &mut nodes);
		let mut window = Node::new(Role::Window);
		window.set_label(title);
		window.set_children(vec![editor]);
		nodes.push((WINDOW_ID, window));

		TreeUpdate {
			nodes,
			tree: Some(TreeInfo::new(WINDOW_ID)),
			tree_id: TreeId::ROOT,
			focus: if self.focused { editor } else { WINDOW_ID },
		}
	}

	/// The editor's accessibility nodes, appended to `nodes`, for a host
	/// that builds its own tree: add the returned id to a parent's children
	/// and make it the focus while the editor has focus. Every call
	/// describes the current view in full.
	pub fn accessibility_nodes(
		&mut self,
		to_window: Affine,
		nodes: &mut Vec<(NodeId, Node)>,
	) -> NodeId {
		let mut editor = Node::new(Role::MultilineTextInput);
		editor.set_label(self.options.label.as_str());
		editor.set_bounds(bounds(
			to_window,
			Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height),
		));
		editor.add_action(Action::Focus);
		editor.add_action(Action::SetTextSelection);
		editor.add_action(Action::ReplaceSelectedText);

		self.a11y.runs.clear();
		let mut children = Vec::new();

		for block in self.described_blocks() {
			if let Some(id) = self.describe_block(block, to_window, nodes) {
				children.push(id);
			}
		}

		editor.set_children(children);

		if let Some(selection) = self.text_selection() {
			editor.set_text_selection(selection);
		}

		nodes.push((EDITOR_ID, editor));
		EDITOR_ID
	}

	/// Handle an action a screen reader asked for: focus, select text,
	/// replace the selection, or scroll a block into view.
	pub fn accessibility_action(&mut self, request: &ActionRequest) -> DesktopResult<Response> {
		match (request.action, &request.data) {
			(Action::Focus | Action::Click, _) => {
				self.focused = true;
				self.blink_from = self.now;
				Ok(Response::CHANGED)
			}
			(Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
				self.select_runs(*selection)
			}
			(Action::ReplaceSelectedText, Some(ActionData::Value(text))) => {
				self.type_text(text).map(Response::changed)
			}
			(Action::ScrollIntoView, _) => Ok(self.scroll_node_into_view(request.target_node)),
			_ => Ok(Response::IGNORED),
		}
	}

	/// Select the text between two positions in described runs.
	fn select_runs(&mut self, selection: TextSelection) -> DesktopResult<Response> {
		let anchor = self.a11y.point(selection.anchor);
		let focus = self.a11y.point(selection.focus);
		let (Some(anchor), Some(focus)) = (anchor, focus) else {
			return Ok(Response::IGNORED);
		};

		self.select(Selection::text(anchor, focus))
			.map(Response::changed)
	}

	/// The text blocks to describe: those on screen, then the selection's
	/// ends.
	fn described_blocks(&self) -> Vec<BlockId> {
		let doc = self.doc.document();
		let (top, bottom) = self.visible_range();
		let mut blocks: Vec<BlockId> = self
			.layout
			.blocks_between(top, bottom)
			.into_iter()
			.filter(|&id| doc.inline(id).is_some())
			.collect();

		if let Selection::Text { anchor, head } = self.doc.selection() {
			blocks.extend([anchor.block, head.block]);
		}

		blocks.sort_by(|&first, &second| position::compare_blocks(doc, first, second));
		blocks.dedup();
		blocks
	}

	/// Describe one text block and its lines, returning the block's id.
	fn describe_block(
		&mut self,
		block: BlockId,
		to_window: Affine,
		nodes: &mut Vec<(NodeId, Node)>,
	) -> Option<NodeId> {
		let text = self.doc.document().inline(block)?.as_str().to_owned();
		let mut node = self.block_node(block, to_window)?;
		let whole = 0..text.len() as u32;
		let lines = self
			.layout
			.line_ranges(block)
			.unwrap_or_else(|| std::iter::once(whole).collect());
		let count = lines.len();
		let mut children = Vec::with_capacity(count);

		for (index, range) in lines.into_iter().enumerate() {
			let id = self.a11y.id(block, index as u32 + 1);
			let line = Line {
				block,
				text: &text,
				range,
				last: index + 1 == count,
			};
			let (run, info) = self.describe_line(&line, to_window);
			nodes.push((id, run));
			self.a11y.runs.insert(id, info);
			children.push(id);
		}

		node.set_children(children);
		let id = self.a11y.id(block, 0);
		nodes.push((id, node));
		Some(id)
	}

	/// A text block's own node: its role (paragraph, heading with its level,
	/// code, or a task's checkbox) and bounds.
	fn block_node(&self, block: BlockId, to_window: Affine) -> Option<Node> {
		let doc = self.doc.document();
		let data = doc.block(block)?;
		let mut node = Node::new(Role::Paragraph);

		match doc.schema().block(data.kind()).name.as_ref() {
			"heading" => {
				node.set_role(Role::Heading);
				let level = data.attrs().get("level").and_then(Value::as_int);
				node.set_level(level.map_or(1, |level| level as usize));
			}
			"codeBlock" => node.set_role(Role::Code),
			_ => {}
		}

		if let Some(checked) = self.task_checked(block) {
			node.set_role(Role::CheckBox);
			node.set_toggled(Toggled::from(checked));
		}

		if let Some(rect) = self.layout.block_rect(block) {
			node.set_bounds(bounds(to_window, self.to_view_rect(rect)));
		}

		Some(node)
	}

	/// One line's text run: its text, grapheme lengths, word starts, and
	/// each grapheme's position and width from layout.
	fn describe_line(&self, line: &Line<'_>, to_window: Affine) -> (Node, RunInfo) {
		let text = line
			.text
			.get(line.range.start as usize..line.range.end as usize)
			.unwrap_or_default();
		let mut graphemes = self.graphemes(line.block, text, line.range.start);
		graphemes.offsets.push(line.range.end);
		let mut value = text.to_owned();

		// A block ends in a hard break, which counts as one more character.
		if line.last {
			value.push('\n');
			graphemes.push_break();
		}

		let mut node = Node::new(Role::TextRun);
		node.set_word_starts(word_starts(text));
		node.set_value(value);
		node.set_character_lengths(graphemes.lengths);
		node.set_character_positions(graphemes.positions);
		node.set_character_widths(graphemes.widths);

		if let Some(rect) = self.line_rect(line.block, &line.range) {
			node.set_bounds(bounds(to_window, self.to_view_rect(rect)));
		}

		let info = RunInfo {
			block: line.block,
			offsets: graphemes.offsets,
		};
		(node, info)
	}

	/// Where each grapheme of `text`, starting at byte `start` of `block`,
	/// is drawn, relative to the first.
	fn graphemes(&self, block: BlockId, text: &str, start: u32) -> Graphemes {
		let mut graphemes = Graphemes::default();
		let left_edge = self.caret_x(block, start, Affinity::Downstream);

		for (offset, grapheme) in text.grapheme_indices(true) {
			let from = start + offset as u32;
			let to = from + grapheme.len() as u32;
			// Upstream keeps a character that ends the line on this line.
			let left = self.caret_x(block, from, Affinity::Downstream);
			let right = self.caret_x(block, to, Affinity::Upstream);
			graphemes.offsets.push(from);
			graphemes
				.lengths
				.push(grapheme.len().min(usize::from(u8::MAX)) as u8);
			graphemes.positions.push(left.min(right) - left_edge);
			graphemes.widths.push((right - left).abs());
		}

		graphemes
	}

	/// The x position of the caret at byte `offset` of `block`.
	fn caret_x(&self, block: BlockId, offset: u32, affinity: Affinity) -> f32 {
		self.layout
			.caret_rect(Caret::with_affinity(Point::new(block, offset), affinity))
			.map_or(0.0, |rect| rect.x)
	}

	/// The rectangle a line covers, from the carets at its ends.
	fn line_rect(&self, block: BlockId, range: &Range<u32>) -> Option<Rect> {
		let start = self
			.layout
			.caret_rect(Caret::new(Point::new(block, range.start)))?;
		let end = self.layout.caret_rect(Caret::with_affinity(
			Point::new(block, range.end),
			Affinity::Upstream,
		))?;
		let left = start.x.min(end.x);
		Some(Rect::new(
			left,
			start.y,
			(end.x - start.x).abs().max(start.width),
			start.height,
		))
	}

	/// Whether `block` is the text of a task item, and if so whether the
	/// item is checked.
	fn task_checked(&self, block: BlockId) -> Option<bool> {
		let doc = self.doc.document();
		let parent = doc.block(doc.block(block)?.parent()?)?;
		let name = &doc.schema().block(parent.kind()).name;

		(name.as_ref() == "taskItem")
			.then(|| parent.attrs().get("checked").and_then(Value::as_bool))
			.map(|checked| checked.unwrap_or(false))
	}

	/// The selection as text positions in the runs just described.
	fn text_selection(&self) -> Option<TextSelection> {
		let Selection::Text { anchor, head } = self.doc.selection() else {
			return None;
		};

		Some(TextSelection {
			anchor: self.a11y.position(anchor)?,
			focus: self.a11y.position(head)?,
		})
	}

	fn scroll_node_into_view(&mut self, node: NodeId) -> Response {
		let block = self
			.a11y
			.ids
			.iter()
			.find_map(|(&(block, _), &id)| (id == node).then_some(block));
		let Some(rect) = block.and_then(|block| self.layout.block_rect(block)) else {
			return Response::IGNORED;
		};
		let before = self.scroll;
		self.scroll_to(rect.y);
		Response::changed(before.to_bits() != self.scroll.to_bits())
	}
}

/// The character indexes where words start in `line`.
fn word_starts(line: &str) -> Vec<u8> {
	let mut starts = Vec::new();
	let mut characters = 0usize;

	for (offset, word) in line.split_word_bound_indices() {
		if word.chars().any(char::is_alphanumeric) {
			starts.push(characters.min(usize::from(u8::MAX)) as u8);
		}

		characters += line[offset..offset + word.len()].graphemes(true).count();
	}

	starts
}

/// `rect` in window coordinates, as AccessKit wants bounds.
fn bounds(to_window: Affine, rect: Rect) -> accesskit::Rect {
	let rect = to_window.transform_rect_bbox(kurbo::Rect::new(
		f64::from(rect.x),
		f64::from(rect.y),
		f64::from(rect.x + rect.width),
		f64::from(rect.y + rect.height),
	));
	accesskit::Rect::new(rect.x0, rect.y0, rect.x1, rect.y1)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn words_start_at_letters() {
		assert_eq!(word_starts("Hello, wide world"), vec![0, 7, 12]);
		assert_eq!(word_starts("  "), Vec::<u8>::new());
		assert_eq!(word_starts("é b"), vec![0, 2], "counts graphemes");
	}
}
