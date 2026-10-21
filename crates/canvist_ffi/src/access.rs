//! What screen readers need: the blocks and lines on screen, what each
//! block is, and where its text is.

use canvist_headless::canvist_model::Block;
use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Document;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_headless::canvist_model::Value;
use canvist_layout::Rect;

use crate::BlockInfo;
use crate::BlockRole;
use crate::Bounds;
use crate::Editor;
use crate::LineInfo;
use crate::OffsetRange;
use crate::session::Session;
use crate::text::utf16_offset;

#[uniffi::export]
impl Editor {
	/// The text blocks with rows between `top` and `bottom`, in document
	/// order. Lay the area out with a frame first; blocks not laid out yet
	/// have estimated bounds.
	pub fn blocks(&self, top: f32, bottom: f32) -> Vec<BlockInfo> {
		let session = self.lock();
		let doc = session.doc.document();

		session
			.layout
			.blocks_between(top, bottom)
			.into_iter()
			.filter_map(|block| {
				Some(BlockInfo {
					range: session.text.range_of(block)?,
					bounds: session.layout.block_rect(block)?.into(),
					role: role(doc, block),
				})
			})
			.collect()
	}

	/// The visual lines of text between `top` and `bottom`, in document
	/// order, for reading line by line.
	pub fn lines(&self, top: f32, bottom: f32) -> Vec<LineInfo> {
		let session = self.lock();
		let mut lines = Vec::new();

		for block in session.layout.blocks_between(top, bottom) {
			block_lines(&session, block, &mut lines);
		}

		lines
	}
}

/// Push the lines of text block `block`.
fn block_lines(session: &Session, block: BlockId, out: &mut Vec<LineInfo>) {
	let doc = session.doc.document();
	let (Some(text), Some(range)) = (doc.inline(block), session.text.range_of(block)) else {
		return;
	};
	let text = text.as_str();
	let Some(ranges) = session.layout.line_ranges(block) else {
		return;
	};

	for line in ranges {
		let selection = Selection::text(Point::new(block, line.start), Point::new(block, line.end));
		let bounds = session
			.layout
			.selection_rects(&selection)
			.into_iter()
			.reduce(|a, b| a.union(&b))
			.or_else(|| {
				let caret = canvist_layout::Caret::new(Point::new(block, line.start));
				session.layout.caret_rect(caret)
			});
		let Some(bounds) = bounds else {
			continue;
		};

		out.push(LineInfo {
			range: OffsetRange {
				start: range.start + utf16_offset(text, line.start),
				end: range.start + utf16_offset(text, line.end),
			},
			bounds: Bounds::from(bounds as Rect),
		});
	}
}

/// What text block `block` is, from its type and the containers around it.
fn role(doc: &Document, block: BlockId) -> BlockRole {
	let schema = doc.schema();
	let name = |id: BlockId| {
		doc.block(id)
			.map(|block| schema.block(block.kind()).name.clone())
	};
	let Some(node) = doc.block(block) else {
		return BlockRole::Paragraph;
	};

	match name(block).as_deref() {
		Some("heading") => {
			let level = node
				.attrs()
				.get("level")
				.and_then(Value::as_int)
				.unwrap_or(1);
			return BlockRole::Heading {
				level: level.clamp(1, 6) as u8,
			};
		}
		Some("codeBlock") => return BlockRole::Code,
		_ => {}
	}

	let mut current = node.parent();

	while let Some(parent) = current {
		let parent_block = doc.block(parent);

		match name(parent).as_deref() {
			Some("listItem" | "taskItem") => {
				let checked = parent_block
					.and_then(|item| item.attrs().get("checked"))
					.and_then(Value::as_bool);
				let list = parent_block.and_then(Block::parent).and_then(name);
				return BlockRole::ListItem {
					ordered: list.as_deref() == Some("orderedList"),
					checked,
				};
			}
			Some("blockquote") => return BlockRole::Quote,
			_ => current = parent_block.and_then(Block::parent),
		}
	}

	BlockRole::Paragraph
}
