//! The editor engine's line navigation, answered by layout.
//!
//! `canvist_core` owns the [`LineNavigator`] trait so the editor never
//! depends on layout. With the `editor` feature, [`LayoutNavigator`]
//! implements it, and the caret types convert both ways.

use canvist_core::LineNavigator;
use canvist_core::NavigationError;
use canvist_model::Document;

use crate::Affinity;
use crate::Caret;
use crate::LayoutNavigator;
use crate::LineEdge;
use crate::VerticalDirection;
use crate::VerticalMotion;

impl From<canvist_core::Affinity> for Affinity {
	fn from(affinity: canvist_core::Affinity) -> Self {
		match affinity {
			canvist_core::Affinity::Upstream => Self::Upstream,
			canvist_core::Affinity::Downstream => Self::Downstream,
		}
	}
}

impl From<Affinity> for canvist_core::Affinity {
	fn from(affinity: Affinity) -> Self {
		match affinity {
			Affinity::Upstream => Self::Upstream,
			Affinity::Downstream => Self::Downstream,
		}
	}
}

impl From<canvist_core::Caret> for Caret {
	fn from(caret: canvist_core::Caret) -> Self {
		Self::with_affinity(caret.point, caret.affinity.into())
	}
}

impl From<Caret> for canvist_core::Caret {
	fn from(caret: Caret) -> Self {
		Self {
			point: caret.point,
			affinity: caret.affinity.into(),
		}
	}
}

impl From<canvist_core::VerticalDirection> for VerticalDirection {
	fn from(direction: canvist_core::VerticalDirection) -> Self {
		match direction {
			canvist_core::VerticalDirection::Up => Self::Up,
			canvist_core::VerticalDirection::Down => Self::Down,
		}
	}
}

impl From<canvist_core::LineEdge> for LineEdge {
	fn from(edge: canvist_core::LineEdge) -> Self {
		match edge {
			canvist_core::LineEdge::Start => Self::Start,
			canvist_core::LineEdge::End => Self::End,
		}
	}
}

impl From<VerticalMotion> for canvist_core::VerticalMotion {
	fn from(motion: VerticalMotion) -> Self {
		Self {
			caret: motion.caret.into(),
			goal_x: motion.goal_x,
		}
	}
}

/// The navigator answers for the document it was created with, so the
/// `doc` each call passes is that same document and goes unused.
impl LineNavigator for LayoutNavigator<'_> {
	fn vertical(
		&mut self,
		_doc: &Document,
		from: canvist_core::Caret,
		direction: canvist_core::VerticalDirection,
		goal_x: Option<f32>,
	) -> Result<Option<canvist_core::VerticalMotion>, NavigationError> {
		let motion = LayoutNavigator::vertical(self, from.into(), direction.into(), goal_x)
			.map_err(|error| NavigationError::new(&error))?;

		Ok(motion.map(Into::into))
	}

	fn line_edge(
		&mut self,
		_doc: &Document,
		at: canvist_core::Caret,
		edge: canvist_core::LineEdge,
	) -> Result<canvist_core::Caret, NavigationError> {
		LayoutNavigator::line_edge(self, at.into(), edge.into())
			.map(Into::into)
			.map_err(|error| NavigationError::new(&error))
	}
}
