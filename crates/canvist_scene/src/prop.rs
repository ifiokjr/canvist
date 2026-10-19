//! Node properties: typed fields read on the hot path, plus custom values.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use kurbo::Affine;
use kurbo::Vec2;
use serde_json::Value as Json;

use crate::AutoLayout;
use crate::BlendMode;
use crate::Constraints;
use crate::Effect;
use crate::LayoutChild;
use crate::NodeId;
use crate::Paint;
use crate::Shape;
use crate::StrokeStyle;

/// Names one property: a core field, a custom value, or an instance
/// override. Each is one last-writer-wins register in Yjs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PropKey {
	/// The layer name.
	Name,
	/// Whether the node is drawn.
	Visible,
	/// Whether the node ignores pointer selection.
	Locked,
	/// The transform relative to the parent.
	Transform,
	/// The width and height.
	Size,
	/// Layer opacity.
	Opacity,
	/// Layer blend mode.
	Blend,
	/// Fill paints.
	Fills,
	/// Stroke paints.
	Strokes,
	/// Stroke geometry.
	Stroke,
	/// Effects.
	Effects,
	/// Corner radii.
	Radius,
	/// Whether a frame clips its children.
	Clip,
	/// Resize constraints.
	Constraints,
	/// A frame's auto-layout.
	Layout,
	/// Behaviour inside an auto-layout parent.
	LayoutChild,
	/// Type-specific geometry.
	Shape,
	/// An extension-defined value.
	Custom(Arc<str>),
	/// An instance's override of a property of one node in its component.
	Override(NodeId, Box<PropKey>),
}

impl PropKey {
	/// The core keys, in declaration order.
	pub const CORE: [Self; 17] = [
		Self::Name,
		Self::Visible,
		Self::Locked,
		Self::Transform,
		Self::Size,
		Self::Opacity,
		Self::Blend,
		Self::Fills,
		Self::Strokes,
		Self::Stroke,
		Self::Effects,
		Self::Radius,
		Self::Clip,
		Self::Constraints,
		Self::Layout,
		Self::LayoutChild,
		Self::Shape,
	];

	/// The core key's name in JSON and Yjs, or `None` for custom and
	/// override keys.
	#[must_use]
	pub const fn core_name(&self) -> Option<&'static str> {
		Some(match self {
			Self::Name => "name",
			Self::Visible => "visible",
			Self::Locked => "locked",
			Self::Transform => "transform",
			Self::Size => "size",
			Self::Opacity => "opacity",
			Self::Blend => "blend",
			Self::Fills => "fills",
			Self::Strokes => "strokes",
			Self::Stroke => "stroke",
			Self::Effects => "effects",
			Self::Radius => "radius",
			Self::Clip => "clip",
			Self::Constraints => "constraints",
			Self::Layout => "layout",
			Self::LayoutChild => "layoutChild",
			Self::Shape => "shape",
			Self::Custom(_) | Self::Override(..) => return None,
		})
	}

	/// Parse a key name: a core name, `c:<name>`, or `o:<node id>:<key>`.
	#[must_use]
	pub fn parse(name: &str) -> Option<Self> {
		if let Some(custom) = name.strip_prefix("c:") {
			return (!custom.is_empty()).then(|| Self::Custom(custom.into()));
		}

		if let Some(rest) = name.strip_prefix("o:") {
			let (target, inner) = rest.split_once(':')?;
			let inner = Self::parse(inner)?;

			if matches!(inner, Self::Override(..)) {
				return None;
			}

			return Some(Self::Override(target.parse().ok()?, Box::new(inner)));
		}

		Self::CORE
			.into_iter()
			.find(|key| key.core_name() == Some(name))
	}
}

/// Formats as the name [`PropKey::parse`] reads.
impl fmt::Display for PropKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Custom(name) => write!(f, "c:{name}"),
			Self::Override(target, inner) => write!(f, "o:{target}:{inner}"),
			core => f.write_str(core.core_name().unwrap_or_default()),
		}
	}
}

/// One property with its value: the payload of
/// [`Step::SetProp`](crate::Step::SetProp).
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
	/// The layer name.
	Name(Arc<str>),
	/// Whether the node is drawn.
	Visible(bool),
	/// Whether the node ignores pointer selection.
	Locked(bool),
	/// The transform relative to the parent.
	Transform(Affine),
	/// The width and height, both at least 0.
	Size(Vec2),
	/// Layer opacity, 0 to 1.
	Opacity(f64),
	/// Layer blend mode.
	Blend(BlendMode),
	/// Fill paints, bottom to top.
	Fills(Arc<[Paint]>),
	/// Stroke paints, bottom to top.
	Strokes(Arc<[Paint]>),
	/// Stroke geometry.
	Stroke(StrokeStyle),
	/// Effects.
	Effects(Arc<[Effect]>),
	/// Corner radii: top-left, top-right, bottom-right, bottom-left.
	Radius([f64; 4]),
	/// Whether a frame clips its children.
	Clip(bool),
	/// Resize constraints.
	Constraints(Constraints),
	/// A frame's auto-layout, or `None` for free positioning.
	Layout(Option<AutoLayout>),
	/// Behaviour inside an auto-layout parent.
	LayoutChild(LayoutChild),
	/// Type-specific geometry.
	Shape(Shape),
	/// An extension-defined value; `None` removes it.
	Custom(Arc<str>, Option<Json>),
	/// Override a property of the component node `target` in this instance.
	Override(NodeId, Box<Prop>),
	/// Remove an instance's override.
	ClearOverride(NodeId, Box<PropKey>),
}

impl Prop {
	/// The key this property sets.
	#[must_use]
	pub fn key(&self) -> PropKey {
		match self {
			Self::Name(_) => PropKey::Name,
			Self::Visible(_) => PropKey::Visible,
			Self::Locked(_) => PropKey::Locked,
			Self::Transform(_) => PropKey::Transform,
			Self::Size(_) => PropKey::Size,
			Self::Opacity(_) => PropKey::Opacity,
			Self::Blend(_) => PropKey::Blend,
			Self::Fills(_) => PropKey::Fills,
			Self::Strokes(_) => PropKey::Strokes,
			Self::Stroke(_) => PropKey::Stroke,
			Self::Effects(_) => PropKey::Effects,
			Self::Radius(_) => PropKey::Radius,
			Self::Clip(_) => PropKey::Clip,
			Self::Constraints(_) => PropKey::Constraints,
			Self::Layout(_) => PropKey::Layout,
			Self::LayoutChild(_) => PropKey::LayoutChild,
			Self::Shape(_) => PropKey::Shape,
			Self::Custom(name, _) => PropKey::Custom(name.clone()),
			Self::Override(target, prop) => PropKey::Override(*target, Box::new(prop.key())),
			Self::ClearOverride(target, key) => PropKey::Override(*target, key.clone()),
		}
	}

	/// Whether this property's numbers are usable: finite, in range, and not
	/// nested overrides.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		match self {
			Self::Transform(affine) => affine.is_finite(),
			Self::Size(size) => size.is_finite() && size.x >= 0.0 && size.y >= 0.0,
			Self::Opacity(opacity) => (0.0..=1.0).contains(opacity),
			Self::Radius(radii) => {
				radii
					.iter()
					.all(|radius| radius.is_finite() && *radius >= 0.0)
			}
			Self::Stroke(stroke) => stroke.width.is_finite() && stroke.width >= 0.0,
			Self::Shape(shape) => shape.is_valid(),
			Self::Override(_, prop) => {
				!matches!(**prop, Self::Override(..) | Self::ClearOverride(..)) && prop.is_valid()
			}
			Self::ClearOverride(_, key) => !matches!(**key, PropKey::Override(..)),
			_ => true,
		}
	}
}

/// A node's properties.
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
	/// The layer name; empty means the host shows the type name.
	pub name: Arc<str>,
	/// Whether the node is drawn.
	pub visible: bool,
	/// Whether the node ignores pointer selection.
	pub locked: bool,
	/// The transform relative to the parent. Scale lives in `size`.
	pub transform: Affine,
	/// The width and height.
	pub size: Vec2,
	/// Layer opacity, 0 to 1.
	pub opacity: f64,
	/// Layer blend mode.
	pub blend: BlendMode,
	/// Fill paints, bottom to top.
	pub fills: Arc<[Paint]>,
	/// Stroke paints, bottom to top.
	pub strokes: Arc<[Paint]>,
	/// Stroke geometry.
	pub stroke: StrokeStyle,
	/// Effects.
	pub effects: Arc<[Effect]>,
	/// Corner radii: top-left, top-right, bottom-right, bottom-left.
	pub radius: [f64; 4],
	/// Whether a frame clips its children.
	pub clip: bool,
	/// Resize constraints.
	pub constraints: Constraints,
	/// A frame's auto-layout.
	pub layout: Option<AutoLayout>,
	/// Behaviour inside an auto-layout parent.
	pub layout_child: LayoutChild,
	/// Type-specific geometry.
	pub shape: Shape,
	/// Extension-defined values.
	pub custom: BTreeMap<Arc<str>, Json>,
	/// An instance's overrides, by component node and key.
	pub overrides: BTreeMap<(NodeId, PropKey), Prop>,
}

impl Default for Props {
	fn default() -> Self {
		Self {
			name: "".into(),
			visible: true,
			locked: false,
			transform: Affine::IDENTITY,
			size: Vec2::ZERO,
			opacity: 1.0,
			blend: BlendMode::Normal,
			fills: Arc::new([]),
			strokes: Arc::new([]),
			stroke: StrokeStyle::default(),
			effects: Arc::new([]),
			radius: [0.0; 4],
			clip: false,
			constraints: Constraints::default(),
			layout: None,
			layout_child: LayoutChild::default(),
			shape: Shape::None,
			custom: BTreeMap::new(),
			overrides: BTreeMap::new(),
		}
	}
}

impl Props {
	/// Default props for a new node of `kind`: its default shape, and
	/// clipping for frames.
	#[must_use]
	pub fn for_kind(kind: crate::NodeKind) -> Self {
		Self {
			shape: Shape::default_for(kind),
			clip: kind == crate::NodeKind::Frame,
			..Self::default()
		}
	}

	/// The current value of `key`.
	#[must_use]
	pub fn get(&self, key: &PropKey) -> Prop {
		match key {
			PropKey::Name => Prop::Name(self.name.clone()),
			PropKey::Visible => Prop::Visible(self.visible),
			PropKey::Locked => Prop::Locked(self.locked),
			PropKey::Transform => Prop::Transform(self.transform),
			PropKey::Size => Prop::Size(self.size),
			PropKey::Opacity => Prop::Opacity(self.opacity),
			PropKey::Blend => Prop::Blend(self.blend),
			PropKey::Fills => Prop::Fills(self.fills.clone()),
			PropKey::Strokes => Prop::Strokes(self.strokes.clone()),
			PropKey::Stroke => Prop::Stroke(self.stroke.clone()),
			PropKey::Effects => Prop::Effects(self.effects.clone()),
			PropKey::Radius => Prop::Radius(self.radius),
			PropKey::Clip => Prop::Clip(self.clip),
			PropKey::Constraints => Prop::Constraints(self.constraints),
			PropKey::Layout => Prop::Layout(self.layout),
			PropKey::LayoutChild => Prop::LayoutChild(self.layout_child),
			PropKey::Shape => Prop::Shape(self.shape.clone()),
			PropKey::Custom(name) => Prop::Custom(name.clone(), self.custom.get(name).cloned()),
			PropKey::Override(target, inner) => {
				self.overrides
					.get(&(*target, (**inner).clone()))
					.map_or_else(
						|| Prop::ClearOverride(*target, inner.clone()),
						|prop| Prop::Override(*target, Box::new(prop.clone())),
					)
			}
		}
	}

	/// Set one property, returning its previous value.
	pub fn set(&mut self, prop: Prop) -> Prop {
		let previous = self.get(&prop.key());
		self.assign(prop);
		previous
	}

	fn assign(&mut self, prop: Prop) {
		match prop {
			Prop::Name(value) => self.name = value,
			Prop::Visible(value) => self.visible = value,
			Prop::Locked(value) => self.locked = value,
			Prop::Transform(value) => self.transform = value,
			Prop::Size(value) => self.size = value,
			Prop::Opacity(value) => self.opacity = value,
			Prop::Blend(value) => self.blend = value,
			Prop::Fills(value) => self.fills = value,
			Prop::Strokes(value) => self.strokes = value,
			Prop::Stroke(value) => self.stroke = value,
			Prop::Effects(value) => self.effects = value,
			Prop::Radius(value) => self.radius = value,
			Prop::Clip(value) => self.clip = value,
			Prop::Constraints(value) => self.constraints = value,
			Prop::Layout(value) => self.layout = value,
			Prop::LayoutChild(value) => self.layout_child = value,
			Prop::Shape(value) => self.shape = value,
			Prop::Custom(name, Some(value)) => {
				self.custom.insert(name, value);
			}
			Prop::Custom(name, None) => {
				self.custom.remove(&name);
			}
			Prop::Override(target, prop) => {
				self.overrides.insert((target, prop.key()), *prop);
			}
			Prop::ClearOverride(target, key) => {
				self.overrides.remove(&(target, *key));
			}
		}
	}

	/// Every property that differs from [`Props::default`].
	#[must_use]
	pub fn changed(&self) -> Vec<Prop> {
		self.changed_from(&Self::default())
	}

	/// Every property that differs from `defaults`, for encoding: scene JSON
	/// omits the values [`Props::for_kind`] gives a node anyway.
	#[must_use]
	pub fn changed_from(&self, defaults: &Self) -> Vec<Prop> {
		let core = PropKey::CORE
			.iter()
			.filter(|key| !self.same_field(defaults, key))
			.map(|key| self.get(key));
		let custom = self
			.custom
			.iter()
			.map(|(name, value)| Prop::Custom(name.clone(), Some(value.clone())));
		let overrides = self
			.overrides
			.iter()
			.map(|((target, _), prop)| Prop::Override(*target, Box::new(prop.clone())));

		core.chain(custom).chain(overrides).collect()
	}

	/// Whether a core field is equal in both, without building values.
	fn same_field(&self, other: &Self, key: &PropKey) -> bool {
		match key {
			PropKey::Name => self.name == other.name,
			PropKey::Visible => self.visible == other.visible,
			PropKey::Locked => self.locked == other.locked,
			PropKey::Transform => self.transform == other.transform,
			PropKey::Size => self.size == other.size,
			PropKey::Opacity => self.opacity.to_bits() == other.opacity.to_bits(),
			PropKey::Blend => self.blend == other.blend,
			PropKey::Fills => self.fills == other.fills,
			PropKey::Strokes => self.strokes == other.strokes,
			PropKey::Stroke => self.stroke == other.stroke,
			PropKey::Effects => self.effects == other.effects,
			PropKey::Radius => self.radius.map(f64::to_bits) == other.radius.map(f64::to_bits),
			PropKey::Clip => self.clip == other.clip,
			PropKey::Constraints => self.constraints == other.constraints,
			PropKey::Layout => self.layout == other.layout,
			PropKey::LayoutChild => self.layout_child == other.layout_child,
			PropKey::Shape => self.shape == other.shape,
			PropKey::Custom(_) | PropKey::Override(..) => self.get(key) == other.get(key),
		}
	}

	/// Check every value is usable for a node of `kind`, returning the key
	/// of the first one that is not.
	pub(crate) fn invalid_key(&self, kind: crate::NodeKind) -> Option<PropKey> {
		let core = [
			(PropKey::Transform, self.transform.is_finite()),
			(PropKey::Size, Prop::Size(self.size).is_valid()),
			(PropKey::Opacity, (0.0..=1.0).contains(&self.opacity)),
			(
				PropKey::Radius,
				self.radius
					.iter()
					.all(|radius| radius.is_finite() && *radius >= 0.0),
			),
			(
				PropKey::Stroke,
				self.stroke.width.is_finite() && self.stroke.width >= 0.0,
			),
			(
				PropKey::Shape,
				self.shape.is_valid() && self.shape.fits(kind),
			),
		];

		if let Some((key, _)) = core.into_iter().find(|(_, valid)| !valid) {
			return Some(key);
		}

		let overrides_allowed = kind == crate::NodeKind::Instance || self.overrides.is_empty();

		self.overrides
			.iter()
			.find(|(_, prop)| {
				!overrides_allowed
					|| !prop.is_valid()
					|| matches!(prop, Prop::Override(..) | Prop::ClearOverride(..))
			})
			.map(|((target, key), _)| PropKey::Override(*target, Box::new(key.clone())))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn key_names_round_trip() {
		let target = NodeId::new(3, 9);
		let keys = PropKey::CORE.into_iter().chain([
			PropKey::Custom("seed".into()),
			PropKey::Override(target, Box::new(PropKey::Fills)),
			PropKey::Override(target, Box::new(PropKey::Custom("a:b".into()))),
		]);

		for key in keys {
			assert_eq!(PropKey::parse(&key.to_string()), Some(key));
		}

		assert_eq!(PropKey::parse("o:3.9:o:3.9:fills"), None);
		assert_eq!(PropKey::parse("c:"), None);
		assert_eq!(PropKey::parse("width"), None);
	}

	#[test]
	fn set_returns_the_previous_value() {
		let mut props = Props::default();
		let target = NodeId::new(1, 2);

		assert_eq!(props.set(Prop::Opacity(0.5)), Prop::Opacity(1.0));
		assert_eq!(
			props.set(Prop::Custom("seed".into(), Some(7.into()))),
			Prop::Custom("seed".into(), None)
		);
		assert_eq!(
			props.set(Prop::Override(target, Box::new(Prop::Visible(false)))),
			Prop::ClearOverride(target, Box::new(PropKey::Visible))
		);
		assert_eq!(props.changed().len(), 3);
	}
}
