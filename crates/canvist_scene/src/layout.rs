//! Constraints and auto-layout: intent stored in the scene. The canvas
//! computes positions from it; the results are never stored.

/// How a node responds on one axis when its parent frame is resized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Constraint {
	/// Keep the distance to the start edge (left or top).
	#[default]
	Start,
	/// Keep the distance to the end edge (right or bottom).
	End,
	/// Keep the distances to both edges, stretching the node.
	Both,
	/// Keep the node's centre at the same relative position.
	Center,
	/// Scale position and size with the parent.
	Scale,
}

impl Constraint {
	/// Every constraint, in declaration order.
	pub const ALL: [Self; 5] = [
		Self::Start,
		Self::End,
		Self::Both,
		Self::Center,
		Self::Scale,
	];

	/// The JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Start => "start",
			Self::End => "end",
			Self::Both => "both",
			Self::Center => "center",
			Self::Scale => "scale",
		}
	}
}

/// A node's constraints on both axes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Constraints {
	/// The horizontal constraint.
	pub horizontal: Constraint,
	/// The vertical constraint.
	pub vertical: Constraint,
}

/// The main axis of an auto-layout frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
	/// Children flow left to right.
	#[default]
	Row,
	/// Children flow top to bottom.
	Column,
}

/// How children are distributed along an axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
	/// Packed at the start.
	#[default]
	Start,
	/// Centred.
	Center,
	/// Packed at the end.
	End,
	/// Main axis only: spread out, with the gap growing to fill the frame.
	SpaceBetween,
	/// Cross axis only: stretched to the frame's inner size.
	Stretch,
}

impl Align {
	/// Every alignment, in declaration order.
	pub const ALL: [Self; 5] = [
		Self::Start,
		Self::Center,
		Self::End,
		Self::SpaceBetween,
		Self::Stretch,
	];

	/// The JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Start => "start",
			Self::Center => "center",
			Self::End => "end",
			Self::SpaceBetween => "space-between",
			Self::Stretch => "stretch",
		}
	}
}

/// Inner spacing of a frame, in the CSS order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Padding {
	/// Top padding.
	pub top: f64,
	/// Right padding.
	pub right: f64,
	/// Bottom padding.
	pub bottom: f64,
	/// Left padding.
	pub left: f64,
}

impl Padding {
	/// The same padding on every side.
	#[must_use]
	pub const fn all(value: f64) -> Self {
		Self {
			top: value,
			right: value,
			bottom: value,
			left: value,
		}
	}
}

/// A frame's auto-layout: a single-line or wrapping flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AutoLayout {
	/// The main axis.
	pub direction: Direction,
	/// The space between children on the main axis (and between lines when
	/// wrapping).
	pub gap: f64,
	/// The frame's inner padding.
	pub padding: Padding,
	/// Distribution on the main axis.
	pub align: Align,
	/// Alignment on the cross axis.
	pub cross_align: Align,
	/// Whether children wrap onto new lines.
	pub wrap: bool,
}

/// How an auto-layout child is sized on one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sizing {
	/// Its own `size`.
	#[default]
	Fixed,
	/// Fill the remaining space (main axis) or the frame (cross axis).
	Fill,
	/// Shrink to its content; for frames with auto-layout.
	Hug,
}

impl Sizing {
	/// Every sizing mode, in declaration order.
	pub const ALL: [Self; 3] = [Self::Fixed, Self::Fill, Self::Hug];

	/// The JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Fixed => "fixed",
			Self::Fill => "fill",
			Self::Hug => "hug",
		}
	}
}

/// A node's behaviour inside an auto-layout parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayoutChild {
	/// Horizontal sizing.
	pub width: Sizing,
	/// Vertical sizing.
	pub height: Sizing,
	/// Whether the node opts out of the flow and keeps its own transform.
	pub absolute: bool,
}
