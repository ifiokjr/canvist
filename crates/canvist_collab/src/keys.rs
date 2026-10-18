//! Names used in the shared document. See the [crate docs](crate#the-yjs-schema).

/// The root `Y.Array` holding the top-level block maps.
pub const ROOT: &str = "blocks";
/// The block id, a string such as `k3f9a.1c`.
pub const ID: &str = "id";
/// The block type name, such as `paragraph`.
pub const TYPE: &str = "type";
/// The block attributes, a `Y.Map`.
pub const ATTRS: &str = "attrs";
/// The inline content of a text block, a `Y.Text`.
pub const TEXT: &str = "text";
/// The children of a container block, a `Y.Array` of block maps.
pub const CHILDREN: &str = "children";
