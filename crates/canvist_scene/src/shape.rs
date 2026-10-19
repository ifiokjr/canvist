//! Type-specific geometry, stored in the `shape` prop.

use std::sync::Arc;

use kurbo::BezPath;

use crate::NodeId;

/// Which points a path's fill covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillRule {
	/// Non-zero winding.
	#[default]
	NonZero,
	/// Even-odd.
	EvenOdd,
}

/// How a boolean node combines its children's outlines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoolOp {
	/// Everything any child covers.
	#[default]
	Union,
	/// The first child minus the others.
	Subtract,
	/// What every child covers.
	Intersect,
	/// What an odd number of children cover.
	Exclude,
}

impl BoolOp {
	/// Every operation, in declaration order.
	pub const ALL: [Self; 4] = [Self::Union, Self::Subtract, Self::Intersect, Self::Exclude];

	/// The JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Union => "union",
			Self::Subtract => "subtract",
			Self::Intersect => "intersect",
			Self::Exclude => "exclude",
		}
	}
}

/// How a text frame takes its size from its content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextSizing {
	/// The frame's `size` is fixed; text may overflow.
	Fixed,
	/// The width is fixed and the height follows the text.
	#[default]
	AutoHeight,
	/// Lines never wrap; the frame is as wide and tall as the text.
	AutoWidth,
}

impl TextSizing {
	/// Every mode, in declaration order.
	pub const ALL: [Self; 3] = [Self::Fixed, Self::AutoHeight, Self::AutoWidth];

	/// The JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Fixed => "fixed",
			Self::AutoHeight => "auto-height",
			Self::AutoWidth => "auto-width",
		}
	}
}

/// A node's type-specific geometry. Which variant is valid depends on the
/// node's [`NodeKind`](crate::NodeKind); [`Shape::fits`] checks it.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
	/// No own geometry: pages, groups, components, and plugin types that
	/// draw from custom props.
	None,
	/// A rectangle filling the node's size, rounded by the `radius` prop.
	/// Rectangles and frames.
	Rect,
	/// An ellipse inscribed in the node's size, optionally an arc or ring.
	Ellipse {
		/// The arc's start angle in degrees.
		start: f64,
		/// The arc's sweep in degrees; 360 for a full ellipse.
		sweep: f64,
		/// The inner radius as a fraction of the outer one; 0 for a disc.
		inner: f64,
	},
	/// A regular polygon or star inscribed in the node's size.
	Polygon {
		/// The number of outer points, at least 3.
		points: u32,
		/// The inner radius as a fraction of the outer one; 1 for a plain
		/// polygon.
		ratio: f64,
	},
	/// A vector path in the node's local coordinates.
	Path {
		/// The outline.
		path: Arc<BezPath>,
		/// The fill rule.
		rule: FillRule,
	},
	/// A boolean combination of the node's children.
	Boolean(BoolOp),
	/// A text frame's sizing. The text itself is a canvist document.
	Text(TextSizing),
	/// An instance of a component.
	Instance {
		/// The component node.
		component: NodeId,
	},
}

impl Shape {
	/// A full ellipse.
	#[must_use]
	pub const fn ellipse() -> Self {
		Self::Ellipse {
			start: 0.0,
			sweep: 360.0,
			inner: 0.0,
		}
	}

	/// A regular polygon with `points` corners.
	#[must_use]
	pub const fn polygon(points: u32) -> Self {
		Self::Polygon { points, ratio: 1.0 }
	}

	/// A path with the non-zero fill rule.
	#[must_use]
	pub fn path(path: BezPath) -> Self {
		Self::Path {
			path: Arc::new(path),
			rule: FillRule::NonZero,
		}
	}

	/// The shape a new node of `kind` starts with.
	#[must_use]
	pub fn default_for(kind: crate::NodeKind) -> Self {
		use crate::NodeKind;

		match kind {
			NodeKind::Frame | NodeKind::Rectangle | NodeKind::Image => Self::Rect,
			NodeKind::Ellipse => Self::ellipse(),
			NodeKind::Polygon => Self::polygon(3),
			NodeKind::Path => Self::path(BezPath::new()),
			NodeKind::Boolean => Self::Boolean(BoolOp::Union),
			NodeKind::Text => Self::Text(TextSizing::AutoHeight),
			NodeKind::Page
			| NodeKind::Group
			| NodeKind::Component
			| NodeKind::Instance
			| NodeKind::Custom => Self::None,
		}
	}

	/// Whether this shape is valid for a node of `kind`. Instances must name
	/// their component.
	#[must_use]
	pub fn fits(&self, kind: crate::NodeKind) -> bool {
		use crate::NodeKind;

		match (self, kind) {
			(Self::Rect, NodeKind::Frame | NodeKind::Rectangle | NodeKind::Image)
			| (Self::Ellipse { .. }, NodeKind::Ellipse)
			| (Self::Polygon { .. }, NodeKind::Polygon)
			| (Self::Path { .. }, NodeKind::Path)
			| (Self::Boolean(_), NodeKind::Boolean)
			| (Self::Text(_), NodeKind::Text)
			| (Self::Instance { .. }, NodeKind::Instance) => true,
			(Self::None, kind) => {
				matches!(
					kind,
					NodeKind::Page | NodeKind::Group | NodeKind::Component | NodeKind::Custom
				)
			}
			_ => false,
		}
	}

	/// Whether the numbers in this shape are usable: finite, with at least
	/// three polygon points.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		match self {
			Self::Ellipse {
				start,
				sweep,
				inner,
			} => start.is_finite() && sweep.is_finite() && (0.0..=1.0).contains(inner),
			Self::Polygon { points, ratio } => *points >= 3 && (0.0..=1.0).contains(ratio),
			_ => true,
		}
	}
}
