#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_scene`
//!
//! The scene model behind canvist's canvas: the design-tool sibling of
//! [`canvist_model`]. Where a canvist document is a tree of blocks, a scene
//! is a tree of nodes: pages, frames, groups, shapes, vector paths, images,
//! components, instances, and text frames that each hold a full canvist
//! text document.
//!
//! - A [`SceneSchema`] declares which node types exist. Several types can
//!   share one built-in [`NodeKind`], so a plugin's `sticky` can be a frame
//!   and its `star` a polygon, and [`NodeKind::Custom`] types draw their own
//!   geometry.
//! - A [`Scene`] stores [`Node`]s by stable [`NodeId`]. Each node has a type,
//!   a parent, a [`FracKey`] ordering it among its siblings, and [`Props`]:
//!   typed core properties plus custom values.
//! - [`Step`]s are the only way to change a scene. Each is invertible and
//!   maps one-to-one onto a Yjs operation, so undo, collaboration, and
//!   rendering share one pipeline, exactly as in the text model.
//! - A [`Transaction`] applies steps atomically and returns a
//!   [`SceneCommit`] naming the touched nodes, which is what caches use to
//!   update only what changed.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_model::BlockSpec;
//! use canvist_model::MarkSet;
//! use canvist_model::Point;
//! use canvist_model::Schema;
//! use canvist_scene::Color;
//! use canvist_scene::NodeId;
//! use canvist_scene::Paint;
//! use canvist_scene::Prop;
//! use canvist_scene::Props;
//! use canvist_scene::Scene;
//! use canvist_scene::SceneSchema;
//! use kurbo::Affine;
//! use kurbo::Vec2;
//!
//! let text = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//! let schema = Arc::new(SceneSchema::standard(text)?);
//! let mut scene = Scene::new(schema, 1)?;
//! let page = scene.pages()[0];
//!
//! let mut transaction = scene.transaction();
//! let card = transaction.insert(page, None, "frame", Props {
//! 	size: Vec2::new(320.0, 200.0),
//! 	fills: Arc::new([Paint::solid(Color::WHITE)]),
//! 	..Props::default()
//! })?;
//! let title = transaction.insert(card, None, "text", Props {
//! 	transform: Affine::translate((16.0, 16.0)),
//! 	size: Vec2::new(288.0, 0.0),
//! 	..Props::default()
//! })?;
//! transaction.text(title, |text| {
//! 	let block = text.doc().first_text_block().expect("text frames start with a paragraph");
//! 	text.insert_text(Point::new(block, 0), "Hello, canvas", MarkSet::empty())
//! })?;
//! transaction.set(card, Prop::Opacity(0.9))?;
//! let commit = transaction.commit();
//!
//! assert_eq!(scene.text(title).map(|doc| doc.plain_text()).as_deref(), Some("Hello, canvas"));
//!
//! // Every commit carries its own undo.
//! scene.apply_steps(commit.undo_steps())?;
//! assert!(!scene.contains(card));
//! assert_eq!(scene.children(page), &[] as &[NodeId]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # The Yjs shape
//!
//! `canvist_collab`'s scene binding stores a scene as one Yjs document:
//!
//! - The root `Y.Map` named `"nodes"` maps each node id (`client.seq` in base
//!   36) to a node `Y.Map`. The tree is flat: nodes point at their parents.
//! - A node map holds `"type"` (the type name), `"pos"` (one atomic value
//!   holding the parent, the key, and the data that breaks move cycles),
//!   `"props"` (a `Y.Map` from property name to its JSON value, one
//!   last-writer-wins register each), and, for text frames, `"text"`: a
//!   `Y.Array` of blocks in `canvist_collab`'s document shape.
//!
//! Each step is one operation on that shape: [`Step::InsertNodes`] and
//! [`Step::RemoveNodes`] set and delete node maps, [`Step::Move`] sets
//! `"pos"`, [`Step::SetProp`] sets one key of `"props"`, and [`Step::Text`]
//! is the text binding's own translation inside `"text"`.
//!
//! # Properties
//!
//! Properties are named in JSON and Yjs by [`PropKey`]'s names: `name`,
//! `visible`, `locked`, `transform` (`[a, b, c, d, e, f]`, kurbo's affine
//! coefficients), `size` (`[w, h]`), `opacity`, `blend` (CSS blend mode
//! names), `fills` and `strokes` (lists of paints such as
//! `{ "type": "solid", "color": "#3b2cf5ff" }`), `stroke`, `effects`,
//! `radius` (`[tl, tr, br, bl]`), `clip`, `constraints`, `layout`,
//! `layoutChild`, and `shape` (for example
//! `{ "type": "path", "d": "M0 0L10 10" }`). Custom values use `c:<name>`
//! and instance overrides `o:<component node id>:<property>`.

mod codec;
mod error;
mod id;
mod json;
mod key;
mod layout;
mod node;
mod paint;
mod prop;
mod scene;
mod schema;
mod shape;
mod step;
mod transaction;

pub use error::SceneError;
pub use id::NodeId;
pub use id::NodeIdGenerator;
pub use id::ParseNodeIdError;
pub use json::SCENE_TYPE;
pub use json::SCENE_VERSION;
pub use json::props_to_json;
pub use key::FracKey;
pub use key::KeyError;
pub use kurbo;
pub use layout::Align;
pub use layout::AutoLayout;
pub use layout::Constraint;
pub use layout::Constraints;
pub use layout::Direction;
pub use layout::LayoutChild;
pub use layout::Padding;
pub use layout::Sizing;
pub use node::Node;
pub use node::NodeRecord;
pub use paint::BlendMode;
pub use paint::Color;
pub use paint::Effect;
pub use paint::EffectKind;
pub use paint::GradientStop;
pub use paint::ImageFit;
pub use paint::Paint;
pub use paint::PaintKind;
pub use paint::Shadow;
pub use paint::StrokeAlign;
pub use paint::StrokeCap;
pub use paint::StrokeJoin;
pub use paint::StrokeStyle;
pub use prop::Prop;
pub use prop::PropKey;
pub use prop::Props;
pub use scene::Scene;
pub use schema::GeometryFn;
pub use schema::NodeKind;
pub use schema::NodeSpec;
pub use schema::NodeTypeId;
pub use schema::SceneSchema;
pub use schema::SceneSchemaBuilder;
pub use schema::SceneSchemaError;
pub use shape::BoolOp;
pub use shape::FillRule;
pub use shape::Shape;
pub use shape::TextSizing;
pub use step::Applied;
pub use step::Step;
pub use transaction::SceneCommit;
pub use transaction::Transaction;
