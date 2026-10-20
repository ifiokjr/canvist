//! The mouse and trackpad: click to place the caret, double-click for a
//! word, triple-click for a block, Shift-click and drag to extend, and the
//! wheel to scroll.

use std::cmp::Ordering;
use std::time::Duration;

use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_model::InlineText;
use canvist_headless::canvist_model::Point as ModelPoint;
use canvist_headless::canvist_model::Selection;
use kurbo::Point;
use unicode_segmentation::UnicodeSegmentation;

use crate::DesktopEditor;
use crate::DesktopResult;
use crate::Modifiers;
use crate::PointerButton;
use crate::Response;
use crate::ScrollDelta;

/// Presses closer together than this count as one multi-click.
const MULTI_CLICK: Duration = Duration::from_millis(500);
/// Presses further apart than this many logical pixels start a new click
/// count.
const CLICK_SLOP: f64 = 4.0;
/// How far one wheel notch scrolls, in logical pixels.
const LINE_HEIGHT: f32 = 40.0;
/// How far past the viewport's edge a drag must go to scroll a whole
/// viewport per second.
const AUTOSCROLL_REACH: f32 = 120.0;

/// What a pointer drag extends the selection by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
	Character,
	Word,
	Block,
}

/// A press: when and where, and how many quick presses led to it.
#[derive(Clone, Copy, Debug)]
struct Click {
	at: Duration,
	position: Point,
	count: u32,
}

/// A drag in progress: the range the press selected, which every later
/// position extends from.
#[derive(Clone, Copy, Debug)]
struct Drag {
	unit: Unit,
	from: ModelPoint,
	to: ModelPoint,
}

/// The pointer's state between events.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PointerState {
	last_click: Option<Click>,
	drag: Option<Drag>,
	/// Where the pointer last was, in the editor's coordinates.
	position: Option<Point>,
	/// When the last autoscroll step ran.
	autoscrolled: Option<Duration>,
}

impl PointerState {
	/// Forget the drag, as when focus leaves.
	pub(crate) fn release(&mut self) {
		self.drag = None;
		self.autoscrolled = None;
	}

	/// Count a press at `position` and `at`: 1 for a click, 2 for a double
	/// click, and so on.
	fn count(&mut self, position: Point, at: Duration) -> u32 {
		let count = match self.last_click {
			Some(last)
				if at.saturating_sub(last.at) <= MULTI_CLICK
					&& last.position.distance(position) <= CLICK_SLOP =>
			{
				last.count + 1
			}
			_ => 1,
		};

		self.last_click = Some(Click {
			at,
			position,
			count,
		});
		count
	}
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Whether a drag is selecting text.
	#[must_use]
	pub fn is_dragging(&self) -> bool {
		self.pointer.drag.is_some()
	}

	pub(crate) fn pointer_down(
		&mut self,
		position: Point,
		button: PointerButton,
		modifiers: Modifiers,
	) -> DesktopResult<Response> {
		let focused = self.focus_on_press();

		if button != PointerButton::Primary {
			return Ok(Response::changed(focused));
		}

		let clicks = self.pointer.count(position, self.now);
		self.pointer.position = Some(position);

		let Some(point) = self.point_at(position) else {
			return Ok(Response::changed(focused));
		};

		let unit = match clicks {
			1 => Unit::Character,
			2 => Unit::Word,
			_ => Unit::Block,
		};

		if modifiers.shift && unit == Unit::Character {
			let anchor = self.anchor();
			self.pointer.drag = Some(Drag {
				unit,
				from: anchor,
				to: anchor,
			});
			self.extend_to(point)?;

			return Ok(Response::CHANGED);
		}

		let (from, to) = self.unit_around(point, unit);
		self.pointer.drag = Some(Drag { unit, from, to });
		self.select(Selection::text(from, to))?;
		Ok(Response::CHANGED)
	}

	pub(crate) fn pointer_move(&mut self, position: Point) -> DesktopResult<Response> {
		self.pointer.position = Some(position);

		if self.pointer.drag.is_none() {
			return Ok(Response::IGNORED);
		}

		match self.point_at(position) {
			Some(point) => self.extend_to(point).map(Response::changed),
			None => Ok(Response::IGNORED),
		}
	}

	pub(crate) fn pointer_up(&mut self, button: PointerButton) -> Response {
		if button != PointerButton::Primary || self.pointer.drag.is_none() {
			return Response::IGNORED;
		}

		self.pointer.release();
		Response {
			consumed: true,
			repaint: false,
		}
	}

	/// Scroll by a wheel or trackpad delta.
	pub(crate) fn scroll(&mut self, delta: ScrollDelta) -> Response {
		let dy = match delta {
			ScrollDelta::Lines { y, .. } => y * LINE_HEIGHT,
			ScrollDelta::Pixels { y, .. } => y,
		};

		let before = self.scroll;
		self.scroll_to(self.scroll - dy);
		Response::changed(self.scroll.to_bits() != before.to_bits())
	}

	/// While a drag holds the pointer above or below the viewport, scroll
	/// toward it (faster the further out it is) and extend the selection.
	/// Hosts call this every frame; it returns whether it scrolled, in which
	/// case the host should draw another frame soon.
	pub fn autoscroll(&mut self, now: Duration) -> DesktopResult<bool> {
		let (Some(_), Some(position)) = (self.pointer.drag, self.pointer.position) else {
			return Ok(false);
		};
		let y = position.y as f32;
		let outside = if y < 0.0 {
			y
		} else if y > self.viewport.height {
			y - self.viewport.height
		} else {
			self.pointer.autoscrolled = None;
			return Ok(false);
		};

		let elapsed = self
			.pointer
			.autoscrolled
			.map_or(Duration::ZERO, |last| now.saturating_sub(last));
		self.pointer.autoscrolled = Some(now);
		self.now = self.now.max(now);
		let speed = self.viewport.height * (outside / AUTOSCROLL_REACH).clamp(-1.0, 1.0);
		self.scroll_to(self.scroll + speed * elapsed.as_secs_f32());
		self.frame(now)?;

		if let Some(point) = self.point_at(position) {
			self.extend_to(point)?;
		}

		Ok(true)
	}

	/// A press focuses the editor; returns whether it was not focused.
	fn focus_on_press(&mut self) -> bool {
		let was = self.focused;
		self.focused = true;
		self.blink_from = self.now;
		!was
	}

	/// The model point under `position`, clamped to the document so a drag
	/// past the edges keeps selecting.
	fn point_at(&self, position: Point) -> Option<ModelPoint> {
		let (x, y) = self.to_document(position.x as f32, position.y as f32);
		self.layout.hit_test(x, y).map(|caret| caret.point)
	}

	/// The selection's anchor, or the document start for block selections.
	fn anchor(&self) -> ModelPoint {
		match self.doc.selection() {
			Selection::Text { anchor, .. } => anchor,
			selection @ Selection::Blocks { .. } => {
				let doc = self.doc.document();
				position::text_range(doc, selection).map_or_else(
					|| ModelPoint::new(doc.children(None)[0], 0),
					|range| range.from,
				)
			}
		}
	}

	/// The range `unit` covers around `point`.
	fn unit_around(&self, point: ModelPoint, unit: Unit) -> (ModelPoint, ModelPoint) {
		let Some(text) = self
			.doc
			.document()
			.inline(point.block)
			.map(InlineText::as_str)
		else {
			return (point, point);
		};

		match unit {
			Unit::Character => (point, point),
			Unit::Word => {
				let (start, end) = word_around(text, point.offset);
				(
					ModelPoint::new(point.block, start),
					ModelPoint::new(point.block, end),
				)
			}
			Unit::Block => {
				(
					ModelPoint::new(point.block, 0),
					ModelPoint::new(point.block, text.len() as u32),
				)
			}
		}
	}

	/// Extend the dragged selection to cover `point` by the drag's unit.
	fn extend_to(&mut self, point: ModelPoint) -> DesktopResult<bool> {
		let Some(drag) = self.pointer.drag else {
			return Ok(false);
		};
		let (start, end) = self.unit_around(point, drag.unit);
		let before = position::compare_points(self.doc.document(), point, drag.from);
		let selection = if before == Ordering::Less {
			Selection::text(drag.to, start)
		} else {
			Selection::text(drag.from, end)
		};

		if selection == self.doc.selection() {
			return Ok(false);
		}

		self.select(selection)
	}
}

/// The word, run of spaces, or punctuation that `offset` is in or just
/// before, as a byte range.
fn word_around(text: &str, offset: u32) -> (u32, u32) {
	let offset = offset as usize;
	let mut last = (0, 0);

	for (start, segment) in text.split_word_bound_indices() {
		let end = start + segment.len();
		last = (start as u32, end as u32);

		if offset < end {
			return last;
		}
	}

	last
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn words_around_offsets() {
		let text = "hello, wide world";

		assert_eq!(word_around(text, 2), (0, 5));
		assert_eq!(word_around(text, 5), (5, 6), "the comma");
		assert_eq!(word_around(text, 9), (7, 11));
		assert_eq!(word_around(text, text.len() as u32), (12, 17));
		assert_eq!(word_around("", 0), (0, 0));
	}

	#[test]
	fn quick_nearby_presses_count_up() {
		let mut pointer = PointerState::default();
		let here = Point::new(10.0, 10.0);
		let quick = MULTI_CLICK / 2;

		assert_eq!(pointer.count(here, Duration::ZERO), 1);
		assert_eq!(pointer.count(here, quick), 2);
		assert_eq!(pointer.count(here, quick * 2), 3);

		let slow = quick * 2 + MULTI_CLICK * 2;
		assert_eq!(pointer.count(here, slow), 1, "too slow");

		let far = here + kurbo::Vec2::new(CLICK_SLOP * 2.0, 0.0);
		assert_eq!(pointer.count(far, slow + quick), 1, "too far");
	}
}
