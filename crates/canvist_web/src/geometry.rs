//! Where layout put everything, as JSON, so tests can hold the canvas to
//! the browser's own layout of the server-rendered HTML.

use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Content;
use canvist_headless::canvist_model::Document;
use canvist_headless::canvist_model::Point;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DocumentLayout;
use canvist_layout::Rect;
use serde_json::Value as Json;
use serde_json::json;
use unicode_segmentation::UnicodeSegmentation;

fn rect(rect: Rect) -> Json {
	json!([rect.x, rect.y, rect.width, rect.height])
}

/// Every block's border box, every text block's caret stops, and every
/// text run, in document coordinates.
///
/// - `blocks`: `{ id, rect, text?, stops? }`. `text` is the block's text in
///   UTF-16 terms the DOM uses, and `stops` holds one `[utf16, downstream x,
///   upstream x, top, height]` per grapheme boundary: where a caret at that
///   offset is drawn with each affinity, and the downstream caret's line.
/// - `runs`: `{ block, start, text, x, width, baseline, ascent, font, rtl }`,
///   where `ascent` is the content area's height above the baseline and
///   `start` is the run's UTF-16 offset in its block. List markers, which
///   CSS generates rather than lays out from the text, are left out.
pub(crate) fn geometry(doc: &Document, layout: &DocumentLayout, list: &DisplayList) -> Json {
	let mut blocks = Vec::new();
	collect_blocks(doc, layout, None, &mut blocks);

	let runs: Vec<Json> = list
		.items
		.iter()
		.filter_map(|item| {
			let DisplayItem::Text(run) = item else {
				return None;
			};
			let font = list.fonts.get(run.font.index())?;
			// List markers are drawn beside their block but are not its text.
			let own_text = doc
				.inline(run.block)
				.is_some_and(|inline| inline.as_str() == &*run.source);

			if !own_text {
				return None;
			}

			Some(json!({
				"block": run.block.to_string(),
				"start": run.source[..run.range.start as usize].encode_utf16().count(),
				"text": run.text(),
				"x": run.x,
				"width": run.width,
				"baseline": run.baseline,
				"ascent": layout.font_metrics(run.font).map_or(0.0, |metrics| metrics.ascent),
				"font": font.css(),
				"rtl": run.rtl,
			}))
		})
		.collect();

	json!({ "height": layout.height(), "blocks": blocks, "runs": runs })
}

fn collect_blocks(
	doc: &Document,
	layout: &DocumentLayout,
	parent: Option<BlockId>,
	out: &mut Vec<Json>,
) {
	for &id in doc.children(parent) {
		let Some(block) = doc.block(id) else {
			continue;
		};
		let mut entry = json!({
			"id": id.to_string(),
			"rect": layout.block_rect(id).map_or(Json::Null, rect),
		});

		match block.content() {
			Content::Inline(inline) => {
				let text = inline.as_str();
				entry["text"] = Json::from(text);
				entry["stops"] = Json::Array(stops(layout, id, text));
			}
			Content::Blocks(_) => collect_blocks(doc, layout, Some(id), out),
			Content::Leaf => {}
		}

		out.push(entry);
	}
}

/// A downstream caret at every grapheme boundary of `text`, keyed by its
/// UTF-16 offset.
fn stops(layout: &DocumentLayout, block: BlockId, text: &str) -> Vec<Json> {
	let mut utf16 = 0;
	let mut previous = 0;

	text.grapheme_indices(true)
		.map(|(offset, _)| offset)
		.chain(std::iter::once(text.len()))
		.filter_map(|offset| {
			utf16 += text[previous..offset].encode_utf16().count();
			previous = offset;
			let point = Point::new(block, offset as u32);
			let down = layout.caret_rect(Caret::with_affinity(point, Affinity::Downstream))?;
			let up = layout.caret_rect(Caret::with_affinity(point, Affinity::Upstream))?;
			let centre = |rect: Rect| rect.x + rect.width / 2.0;

			Some(json!([
				utf16,
				centre(down),
				centre(up),
				down.y,
				down.height
			]))
		})
		.collect()
}
