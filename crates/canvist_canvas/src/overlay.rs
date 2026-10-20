//! The overlay: canvas chrome as data. Hosts draw it however they like, or
//! hand it to the default painter. Everything is in screen pixels.

use kurbo::BezPath;
use kurbo::Point;
use kurbo::Rect;

use crate::Handle;

/// What an outline marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineKind {
	/// A selected node.
	Selection,
	/// The node under the pointer.
	Hover,
	/// The selection's frame, which the handles surround.
	Frame,
}

/// Whether a pen point is an anchor or a curve handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointKind {
	/// A path anchor.
	Anchor,
	/// A curve control point.
	Control,
}

/// One thing to draw over the canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayItem {
	/// An outline around a node or the selection.
	Outline {
		/// The outline.
		path: BezPath,
		/// What it marks.
		kind: OutlineKind,
	},
	/// A transform handle square.
	Handle {
		/// The square.
		rect: Rect,
		/// Which handle.
		handle: Handle,
	},
	/// The marquee rectangle.
	Marquee {
		/// The rectangle.
		rect: Rect,
	},
	/// A snapping guide.
	Guide {
		/// One end.
		from: Point,
		/// The other end.
		to: Point,
	},
	/// A path being drawn, such as the pen tool's.
	Path {
		/// The path.
		path: BezPath,
	},
	/// A path point.
	Point {
		/// Where it is.
		at: Point,
		/// What it is.
		kind: PointKind,
	},
	/// A small text label, such as a size readout.
	Label {
		/// Its anchor point.
		at: Point,
		/// The text.
		text: String,
	},
}

/// The pointer cursor the canvas wants.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Cursor {
	/// The default arrow.
	#[default]
	Default,
	/// Something can be grabbed.
	Grab,
	/// Something is being dragged.
	Grabbing,
	/// Something will be drawn.
	Crosshair,
	/// Text will be placed.
	Text,
	/// A move.
	Move,
	/// A resize along `angle` radians in screen space.
	Resize(f64),
	/// A rotation near the corner at `angle` radians.
	Rotate(f64),
}
