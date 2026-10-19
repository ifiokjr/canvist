//! The scene schema: which node types exist.

use std::fmt;
use std::sync::Arc;

use canvist_model::Schema;
use kurbo::BezPath;
use rustc_hash::FxHashMap;

use crate::Node;

/// The built-in behaviour a node type has. Several types may share a kind:
/// a plugin's `sticky` type can be a [`NodeKind::Frame`] and its `star` a
/// [`NodeKind::Polygon`], with their own names and default props.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
	/// A top-level page: a child of [`NodeId::ROOT`](crate::NodeId::ROOT).
	Page,
	/// A frame: a rectangle that can clip, lay out, and constrain children.
	Frame,
	/// A group: its bounds are its children's.
	Group,
	/// A rectangle, rounded by the `radius` prop.
	Rectangle,
	/// An ellipse, arc, or ring.
	Ellipse,
	/// A regular polygon or star.
	Polygon,
	/// A vector path.
	Path,
	/// A boolean combination of its children.
	Boolean,
	/// A rectangle meant for an image fill.
	Image,
	/// A text frame holding a canvist text document.
	Text,
	/// A reusable component: a frame-like container instances copy.
	Component,
	/// An instance of a component, with per-property overrides.
	Instance,
	/// A plugin type whose geometry comes from its spec's geometry function.
	Custom,
}

/// Builds a custom node type's outline from the node, in local coordinates.
pub type GeometryFn = Arc<dyn Fn(&Node) -> BezPath + Send + Sync>;

/// The declaration of one node type.
#[derive(Clone)]
pub struct NodeSpec {
	name: Arc<str>,
	kind: NodeKind,
	children: bool,
	geometry: Option<GeometryFn>,
}

impl NodeSpec {
	/// A type named `name` with the behaviour of `kind`.
	#[must_use]
	pub fn new(name: &str, kind: NodeKind) -> Self {
		let children = matches!(
			kind,
			NodeKind::Page
				| NodeKind::Frame
				| NodeKind::Group
				| NodeKind::Boolean
				| NodeKind::Component
		);

		Self {
			name: name.into(),
			kind,
			children,
			geometry: None,
		}
	}

	/// A plugin type drawn by `geometry`, which may hold children if
	/// `children` is true.
	#[must_use]
	pub fn custom(
		name: &str,
		children: bool,
		geometry: impl Fn(&Node) -> BezPath + Send + Sync + 'static,
	) -> Self {
		Self {
			name: name.into(),
			kind: NodeKind::Custom,
			children,
			geometry: Some(Arc::new(geometry)),
		}
	}

	/// The type name, as stored in JSON and Yjs.
	#[must_use]
	pub fn name(&self) -> &str {
		&self.name
	}

	/// The built-in behaviour.
	#[must_use]
	pub fn kind(&self) -> NodeKind {
		self.kind
	}

	/// Whether nodes of this type hold children.
	#[must_use]
	pub fn has_children(&self) -> bool {
		self.children
	}

	/// The geometry function of a custom type.
	#[must_use]
	pub fn geometry(&self) -> Option<&GeometryFn> {
		self.geometry.as_ref()
	}
}

impl fmt::Debug for NodeSpec {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NodeSpec")
			.field("name", &self.name)
			.field("kind", &self.kind)
			.field("children", &self.children)
			.field("geometry", &self.geometry.is_some())
			.finish()
	}
}

/// An index into a [`SceneSchema`]'s node types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeTypeId(u16);

impl NodeTypeId {
	/// The index.
	#[must_use]
	pub const fn index(self) -> usize {
		self.0 as usize
	}
}

/// Why a schema could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SceneSchemaError {
	/// Two node types share a name.
	#[error("node type `{0}` is registered twice")]
	Duplicate(String),
	/// A text node type was registered without a text schema.
	#[error("node type `{0}` holds text, but no text schema was given")]
	MissingTextSchema(String),
	/// A custom type has no geometry function.
	#[error("custom node type `{0}` has no geometry")]
	MissingGeometry(String),
	/// More than 65,536 node types.
	#[error("too many node types")]
	TooMany,
}

/// The node types a scene may contain, and the text schema of its text
/// frames.
#[derive(Clone, Debug)]
pub struct SceneSchema {
	types: Vec<NodeSpec>,
	by_name: FxHashMap<Arc<str>, NodeTypeId>,
	page: NodeTypeId,
	text: Option<Arc<Schema>>,
}

impl SceneSchema {
	/// Start a schema. It always contains a `page` type.
	#[must_use]
	pub fn builder() -> SceneSchemaBuilder {
		SceneSchemaBuilder::default()
	}

	/// Every built-in type under its usual name: `page`, `frame`, `group`,
	/// `rectangle`, `ellipse`, `polygon`, `path`, `boolean`, `image`, `text`,
	/// `component`, and `instance`.
	pub fn standard(text: Arc<Schema>) -> Result<Self, SceneSchemaError> {
		let mut builder = Self::builder().text(text);

		for (name, kind) in STANDARD {
			builder = builder.node(NodeSpec::new(name, kind));
		}

		builder.build()
	}

	/// The type with this name.
	#[must_use]
	pub fn node_type(&self, name: &str) -> Option<NodeTypeId> {
		self.by_name.get(name).copied()
	}

	/// The declaration of `id`.
	///
	/// # Panics
	///
	/// Panics when `id` came from another schema.
	#[must_use]
	pub fn spec(&self, id: NodeTypeId) -> &NodeSpec {
		&self.types[id.index()]
	}

	/// The `page` type.
	#[must_use]
	pub fn page(&self) -> NodeTypeId {
		self.page
	}

	/// The schema of text frames' documents, if text frames are allowed.
	#[must_use]
	pub fn text(&self) -> Option<&Arc<Schema>> {
		self.text.as_ref()
	}

	/// Every node type.
	pub fn types(&self) -> impl Iterator<Item = (NodeTypeId, &NodeSpec)> {
		self.types
			.iter()
			.enumerate()
			.map(|(index, spec)| (NodeTypeId(index as u16), spec))
	}

	/// Whether a node of type `child` may be a child of a node of type
	/// `parent` (`None` for the root).
	#[must_use]
	pub fn accepts(&self, parent: Option<NodeTypeId>, child: NodeTypeId) -> bool {
		let is_page = self.spec(child).kind == NodeKind::Page;

		match parent {
			None => is_page,
			Some(parent) => !is_page && self.spec(parent).children,
		}
	}
}

const STANDARD: [(&str, NodeKind); 11] = [
	("frame", NodeKind::Frame),
	("group", NodeKind::Group),
	("rectangle", NodeKind::Rectangle),
	("ellipse", NodeKind::Ellipse),
	("polygon", NodeKind::Polygon),
	("path", NodeKind::Path),
	("boolean", NodeKind::Boolean),
	("image", NodeKind::Image),
	("text", NodeKind::Text),
	("component", NodeKind::Component),
	("instance", NodeKind::Instance),
];

/// Builds a [`SceneSchema`].
#[derive(Clone, Debug, Default)]
pub struct SceneSchemaBuilder {
	types: Vec<NodeSpec>,
	text: Option<Arc<Schema>>,
}

impl SceneSchemaBuilder {
	/// Register a node type.
	#[must_use]
	pub fn node(mut self, spec: NodeSpec) -> Self {
		self.types.push(spec);
		self
	}

	/// The schema of text frames' documents.
	#[must_use]
	pub fn text(mut self, schema: Arc<Schema>) -> Self {
		self.text = Some(schema);
		self
	}

	/// Whether a type with this name has been registered.
	#[must_use]
	pub fn has(&self, name: &str) -> bool {
		self.types.iter().any(|spec| &*spec.name == name)
	}

	/// Build the schema.
	pub fn build(self) -> Result<SceneSchema, SceneSchemaError> {
		let mut types = vec![NodeSpec::new("page", NodeKind::Page)];
		types.extend(self.types);

		if types.len() > usize::from(u16::MAX) {
			return Err(SceneSchemaError::TooMany);
		}

		let mut by_name = FxHashMap::default();

		for (index, spec) in types.iter().enumerate() {
			check_spec(spec, self.text.is_some())?;

			if by_name
				.insert(spec.name.clone(), NodeTypeId(index as u16))
				.is_some()
			{
				return Err(SceneSchemaError::Duplicate(spec.name.to_string()));
			}
		}

		Ok(SceneSchema {
			types,
			by_name,
			page: NodeTypeId(0),
			text: self.text,
		})
	}
}

fn check_spec(spec: &NodeSpec, has_text: bool) -> Result<(), SceneSchemaError> {
	if spec.kind == NodeKind::Text && !has_text {
		return Err(SceneSchemaError::MissingTextSchema(spec.name.to_string()));
	}

	if spec.kind == NodeKind::Custom && spec.geometry.is_none() {
		return Err(SceneSchemaError::MissingGeometry(spec.name.to_string()));
	}

	Ok(())
}
