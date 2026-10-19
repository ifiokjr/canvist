//! Auto-layout: where a frame's flow children go. The scene stores the
//! intent ([`AutoLayout`], [`LayoutChild`]); positions are computed here and
//! never stored, so peers never write conflicting results.

use canvist_scene::Align;
use canvist_scene::AutoLayout;
use canvist_scene::Direction;
use canvist_scene::LayoutChild;
use canvist_scene::Sizing;
use kurbo::Point;
use kurbo::Vec2;

/// One child as auto-layout sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowItem {
	/// The child's own (or intrinsic) size.
	pub size: Vec2,
	/// How it sizes itself in the flow.
	pub child: LayoutChild,
}

/// Where one child goes, in the frame's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
	/// The top-left corner.
	pub origin: Point,
	/// The size, after fill and stretch.
	pub size: Vec2,
}

/// The result of laying out a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowResult {
	/// One placement per item, in order. Absolute items keep their size and
	/// get `None`: they keep their own transform.
	pub placed: Vec<Option<Placed>>,
	/// The size the frame takes after hugging its content.
	pub frame: Vec2,
}

/// Lay out `items` in a frame of `frame` size with `layout`. `hug` says per
/// axis (x, y) whether the frame shrinks to its content.
#[must_use]
pub fn flow(layout: &AutoLayout, frame: Vec2, hug: (bool, bool), items: &[FlowItem]) -> FlowResult {
	let axes = Axes::new(layout.direction);
	let padding = layout.padding;
	let inset = Vec2::new(padding.left + padding.right, padding.top + padding.bottom);
	let inner = frame - inset;

	let flow: Vec<usize> = (0..items.len())
		.filter(|&index| !items[index].child.absolute)
		.collect();
	let lines = if layout.wrap {
		wrap_lines(layout, &axes, axes.main(inner), items, &flow)
	} else {
		vec![flow]
	};

	let mut placed = vec![None; items.len()];
	let mut cross_cursor = 0.0;
	let mut content = Vec2::ZERO;

	for line in &lines {
		let measured = place_line(layout, &axes, inner, items, line);
		let line_cross = measured.cross;

		for (index, mut item) in measured.items {
			item.origin += Vec2::new(padding.left, padding.top) + axes.vec(0.0, cross_cursor);
			placed[index] = Some(item);
		}

		content = axes.vec(
			axes.main(content).max(measured.main),
			cross_cursor + line_cross,
		);
		cross_cursor += line_cross + layout.gap;
	}

	let hugged = Vec2::new(
		if hug.0 { content.x + inset.x } else { frame.x },
		if hug.1 { content.y + inset.y } else { frame.y },
	);

	FlowResult {
		placed,
		frame: hugged,
	}
}

/// Main and cross axis helpers for one direction.
struct Axes {
	row: bool,
}

impl Axes {
	fn new(direction: Direction) -> Self {
		Self {
			row: direction == Direction::Row,
		}
	}

	fn main(&self, value: Vec2) -> f64 {
		if self.row { value.x } else { value.y }
	}

	fn cross(&self, value: Vec2) -> f64 {
		if self.row { value.y } else { value.x }
	}

	fn vec(&self, main: f64, cross: f64) -> Vec2 {
		if self.row {
			Vec2::new(main, cross)
		} else {
			Vec2::new(cross, main)
		}
	}

	fn main_sizing(&self, child: LayoutChild) -> Sizing {
		if self.row { child.width } else { child.height }
	}

	fn cross_sizing(&self, child: LayoutChild) -> Sizing {
		if self.row { child.height } else { child.width }
	}
}

/// Break flow items into lines no longer than `limit` on the main axis.
fn wrap_lines(
	layout: &AutoLayout,
	axes: &Axes,
	limit: f64,
	items: &[FlowItem],
	flow: &[usize],
) -> Vec<Vec<usize>> {
	let mut lines: Vec<Vec<usize>> = Vec::new();
	let mut used = 0.0;

	for &index in flow {
		let length = axes.main(items[index].size);
		let fits = lines
			.last()
			.is_some_and(|line| !line.is_empty() && used + layout.gap + length <= limit);

		if fits {
			used += layout.gap + length;

			if let Some(line) = lines.last_mut() {
				line.push(index);
			}
		} else {
			used = length;
			lines.push(vec![index]);
		}
	}

	lines
}

/// One laid-out line, before it is moved to its cross position.
struct Line {
	items: Vec<(usize, Placed)>,
	main: f64,
	cross: f64,
}

/// Size and place one line's items along the main axis, and align each on
/// the cross axis.
fn place_line(
	layout: &AutoLayout,
	axes: &Axes,
	inner: Vec2,
	items: &[FlowItem],
	line: &[usize],
) -> Line {
	let gaps = layout.gap * line.len().saturating_sub(1) as f64;
	let fill_length = fill_length(axes, axes.main(inner) - gaps, items, line);

	let sizes: Vec<Vec2> = line
		.iter()
		.map(|&index| item_size(layout, axes, inner, items[index], fill_length))
		.collect();
	let total = sizes.iter().map(|size| axes.main(*size)).sum::<f64>() + gaps;
	let cross = sizes
		.iter()
		.map(|size| axes.cross(*size))
		.fold(0.0, f64::max);
	let (mut cursor, spacing) = distribute(
		layout.align,
		axes.main(inner),
		total,
		line.len(),
		layout.gap,
	);
	let mut placed = Vec::with_capacity(line.len());

	for (&index, size) in line.iter().zip(sizes) {
		let offset = align_offset(
			layout.cross_align,
			axes.cross(inner).max(cross),
			axes.cross(size),
		);
		placed.push((
			index,
			Placed {
				origin: axes.vec(cursor, offset).to_point(),
				size,
			},
		));
		cursor += axes.main(size) + spacing;
	}

	Line {
		items: placed,
		main: total,
		cross,
	}
}

/// The main-axis length each fill child gets: an equal share of `room`
/// after the fixed-size children.
fn fill_length(axes: &Axes, room: f64, items: &[FlowItem], line: &[usize]) -> f64 {
	let (fills, fixed): (Vec<usize>, Vec<usize>) = line
		.iter()
		.partition(|&&index| axes.main_sizing(items[index].child) == Sizing::Fill);

	if fills.is_empty() {
		return 0.0;
	}

	let fixed_length: f64 = fixed
		.iter()
		.map(|&index| axes.main(items[index].size))
		.sum();
	((room - fixed_length) / fills.len() as f64).max(0.0)
}

fn item_size(
	layout: &AutoLayout,
	axes: &Axes,
	inner: Vec2,
	item: FlowItem,
	fill_length: f64,
) -> Vec2 {
	let main = if axes.main_sizing(item.child) == Sizing::Fill {
		fill_length
	} else {
		axes.main(item.size)
	};

	let stretch =
		layout.cross_align == Align::Stretch || axes.cross_sizing(item.child) == Sizing::Fill;
	let cross = if stretch {
		axes.cross(inner)
	} else {
		axes.cross(item.size)
	};

	axes.vec(main, cross.max(0.0))
}

/// The starting offset and the space between items on the main axis.
fn distribute(align: Align, room: f64, used: f64, count: usize, gap: f64) -> (f64, f64) {
	let free = room - used;

	match align {
		Align::Center => (free / 2.0, gap),
		Align::End => (free, gap),

		Align::SpaceBetween if count > 1 => (0.0, gap + free.max(0.0) / (count - 1) as f64),
		_ => (0.0, gap),
	}
}

fn align_offset(align: Align, room: f64, size: f64) -> f64 {
	match align {
		Align::Center => (room - size) / 2.0,
		Align::End => room - size,
		_ => 0.0,
	}
}

#[cfg(test)]
mod tests {
	use canvist_scene::Padding;

	use super::*;

	fn item(width: f64, height: f64) -> FlowItem {
		FlowItem {
			size: Vec2::new(width, height),
			child: LayoutChild::default(),
		}
	}

	fn origins(result: &FlowResult) -> Vec<(f64, f64)> {
		result
			.placed
			.iter()
			.flatten()
			.map(|placed| (placed.origin.x, placed.origin.y))
			.collect()
	}

	/// A short child's size.
	const SHORT: Vec2 = Vec2::new(20.0, 10.0);
	/// A taller, wider child's size.
	const TALL: Vec2 = Vec2::new(30.0, 40.0);

	/// A short child and a taller, wider one.
	fn pair() -> [FlowItem; 2] {
		[item(SHORT.x, SHORT.y), item(TALL.x, TALL.y)]
	}

	#[test]
	fn a_row_places_children_with_gaps_and_padding() {
		/// Wider and taller than the children, so cross-centring shows.
		const FRAME: Vec2 = Vec2::new(200.0, 50.0);
		/// Padding on every side.
		const PADDING: f64 = 5.0;

		let layout = AutoLayout {
			gap: 10.0,
			padding: Padding::all(PADDING),
			cross_align: Align::Center,
			..AutoLayout::default()
		};
		let result = flow(&layout, FRAME, (false, false), &pair());

		assert_eq!(origins(&result), [(5.0, 20.0), (35.0, 5.0)]);
	}

	#[test]
	fn fill_children_share_the_remaining_space() {
		/// A column with 100 units of height to share.
		const FRAME: Vec2 = Vec2::new(50.0, 100.0);
		/// The fixed child's height.
		const FIXED: f64 = 20.0;

		let layout = AutoLayout {
			direction: Direction::Column,
			gap: 4.0,
			..AutoLayout::default()
		};
		let mut fill = item(10.0, 0.0);
		fill.child.height = Sizing::Fill;
		let result = flow(
			&layout,
			FRAME,
			(false, false),
			&[item(10.0, FIXED), fill, fill],
		);
		let heights: Vec<f64> = result
			.placed
			.iter()
			.flatten()
			.map(|placed| placed.size.y)
			.collect();

		assert_eq!(heights, [20.0, 36.0, 36.0]);
	}

	#[test]
	fn hugging_frames_shrink_to_their_content() {
		/// Far larger than the children; hugging ignores it.
		const FRAME: Vec2 = Vec2::new(500.0, 500.0);
		/// Padding on every side.
		const PADDING: f64 = 8.0;

		let layout = AutoLayout {
			gap: 10.0,
			padding: Padding::all(PADDING),
			..AutoLayout::default()
		};
		let result = flow(&layout, FRAME, (true, true), &pair());

		assert_eq!(result.frame, Vec2::new(76.0, 56.0));
	}

	#[test]
	fn wrapping_starts_new_lines() {
		let layout = AutoLayout {
			gap: 10.0,
			wrap: true,
			..AutoLayout::default()
		};
		let items = [item(40.0, 10.0), item(40.0, 20.0), item(40.0, 10.0)];
		let result = flow(&layout, Vec2::new(100.0, 100.0), (false, false), &items);

		assert_eq!(origins(&result), [(0.0, 0.0), (50.0, 0.0), (0.0, 30.0)]);
	}

	#[test]
	fn space_between_spreads_children() {
		let layout = AutoLayout {
			align: Align::SpaceBetween,
			..AutoLayout::default()
		};
		let result = flow(
			&layout,
			Vec2::new(100.0, 10.0),
			(false, false),
			&[item(10.0, 10.0), item(10.0, 10.0), item(10.0, 10.0)],
		);

		assert_eq!(origins(&result), [(0.0, 0.0), (45.0, 0.0), (90.0, 0.0)]);
	}
}
