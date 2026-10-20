//! Names used in a shared scene. See the [module docs](super#the-yjs-shape).

/// The root `Y.Map` from node id to node map.
pub const NODES: &str = "nodes";
/// The node type name, such as `rectangle`.
pub const TYPE: &str = "type";
/// The position register: `[parent, key, clock, client, previousParent]`.
pub const POS: &str = "pos";
/// The `Y.Map` of property values.
pub const PROPS: &str = "props";
/// A text frame's `Y.Array` of block maps.
pub const TEXT: &str = "text";
