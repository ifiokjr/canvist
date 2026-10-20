#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_collab`
//!
//! Real-time collaboration for [`canvist_model`] documents over Yjs, using
//! [`yrs`]. A [`Binding`] mirrors a [`Document`](canvist_model::Document)
//! into a yrs document and back, translating each model
//! [`Step`](canvist_model::Step) into yrs operations and each remote yrs
//! event into steps. It never diffs documents, so applying a remote
//! keystroke costs the same in a ten-block document as in a ten-thousand-block
//! one.
//!
//! The shared document is plain Yjs, so JavaScript Yjs peers, y-websocket
//! servers, and Hocuspocus servers take part without knowing about canvist.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_collab::Binding;
//! use canvist_collab::Encoding;
//! use canvist_collab::Track;
//! use canvist_model::BlockSpec;
//! use canvist_model::Document;
//! use canvist_model::MarkSet;
//! use canvist_model::Point;
//! use canvist_model::Schema;
//!
//! let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//!
//! // The first peer seeds the shared document from its own.
//! let mut alice_doc = Document::new(schema.clone(), 1);
//! let mut alice = Binding::with_client_id(schema.clone(), 1)?;
//! alice.seed(&alice_doc)?;
//!
//! // A second peer joins from the first peer's state.
//! let mut bob = Binding::with_client_id(schema.clone(), 2)?;
//! let state = alice.encode_state(Encoding::V1);
//! let mut bob_doc = Document::empty(schema, 2);
//! bob.apply_update(&mut bob_doc, &state, Encoding::V1)?;
//!
//! // Alice types; her binding turns the commit into a Yjs update.
//! let block = alice_doc.first_text_block().expect("seeded with one paragraph");
//! let mut transaction = alice_doc.transaction();
//! transaction.insert_text(Point::new(block, 0), "Hello", MarkSet::empty())?;
//! let commit = transaction.commit();
//! alice.apply_local(&mut alice_doc, &commit, Track::Step)?;
//!
//! // Bob applies Alice's update and gets an ordinary commit back.
//! for update in alice.take_updates() {
//! 	bob.apply_update(&mut bob_doc, &update, Encoding::V1)?;
//! }
//!
//! assert_eq!(bob_doc.plain_text(), "Hello");
//! assert_eq!(bob_doc.to_json(), alice_doc.to_json());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # The Yjs schema
//!
//! Any Yjs client can read and write canvist documents by following this
//! shape:
//!
//! - The root is a `Y.Array` named `"blocks"` ([`keys::ROOT`]) holding one
//!   `Y.Map` per top-level block, in document order.
//! - Each block `Y.Map` has:
//!   - `"id"`: the block id as a string, `client.seq` in base 36
//!     (for example `"k3f9a.1c"`), as [`BlockId`](canvist_model::BlockId)
//!     formats it.
//!   - `"type"`: the block type name from the schema, such as `"paragraph"`.
//!   - `"attrs"`: a `Y.Map` of block attributes (strings, numbers, booleans,
//!     null, and arrays of those).
//!   - For text blocks, `"text"`: a `Y.Text`. Its formatting attributes are
//!     the marks: the key is the mark name and the value is `true`, or a map
//!     of the mark's attributes (`{ "href": "…" }` for a link). `null` or
//!     `false` means the mark is absent.
//!     Inline embeds are embedded values of the form
//!     `{ "<embed type>": { …attrs } }`, for example
//!     `{ "mention": { "id": "ifiok" } }`.
//!   - For containers, `"children"`: a `Y.Array` of child block maps.
//!   - Leaf blocks, such as dividers, have neither.
//!
//! Integers are Yjs integers and floats are Yjs floats, matching JavaScript
//! `Number`s. Text indices are UTF-16 code units, as in JavaScript.
//!
//! Blocks that break the schema are hidden from the model but kept in the
//! shared document, never deleted: maps with an unknown type, an id that
//! does not parse, the wrong content for their type, or a type their parent
//! does not allow (which can follow from concurrent edits). They reappear
//! when they become valid, for example when a concurrent type change is
//! reverted. Inline content the model cannot show (embeds of unknown types
//! or shared types embedded in text) appears as U+FFFD. Formatting keys
//! that are not marks are ignored and preserved.
//!
//! # Concurrency
//!
//! Yjs guarantees every peer converges on the same shared document, and the
//! model is a deterministic function of it, so models converge too. Two
//! consequences follow from Yjs having no move or split operation:
//!
//! - **Moving a block** deletes its map and writes a copy with the same id.
//!   Text typed into the block concurrently goes to the deleted map and is
//!   lost. When two peers move the same block at once, both copies survive
//!   in Yjs; of the copies the model can show, the one with the smallest
//!   yrs item id keeps the block and the others are deleted (see
//!   [`Binding::apply_update`]). A hidden copy claims nothing until it
//!   becomes valid.
//! - **Splitting a block** moves its tail into a new `Y.Text`. Edits a peer
//!   makes to that tail concurrently target the deleted original: inserted
//!   text survives but lands at the end of the first block, while deletions
//!   and formatting are lost because the copy still has the old text. Edits
//!   before the split point are unaffected. Every Yjs-based editor shares
//!   this limitation; the `concurrent_split` tests measure it.
//!
//! Updates may arrive out of order and more than once. The binding holds an
//! update back until the updates it builds on have arrived, because yrs 0.27
//! and 0.28 can otherwise integrate it early and diverge
//! ([y-crdt#670](https://github.com/y-crdt/y-crdt/issues/670),
//! [y-crdt#677](https://github.com/y-crdt/y-crdt/issues/677)).
//!
//! # Undo
//!
//! [`Binding::undo`] and [`Binding::redo`] use a yrs `UndoManager` that only
//! tracks local commits, so undoing never reverts a collaborator's work.
//! [`Track`] controls how commits group into undo steps. The binding also
//! implements [`History`], the trait an editor uses to swap local history for
//! shared history.
//!
//! # Syncing and presence
//!
//! A [`SyncSession`] speaks the y-sync protocol (sync step 1, step 2, and
//! updates) plus awareness, over any transport: the host passes it the
//! binary messages it receives and sends the ones it returns. Use
//! [`Framing::YWebsocket`] for y-websocket servers and
//! [`Framing::Hocuspocus`] for Hocuspocus, which adds a document name and an
//! authentication step.
//!
//! [`Presence`] publishes this peer's [`User`] and selection through Yjs
//! awareness, encoding carets as Yjs relative positions so they stay on
//! their characters while others type, and resolves other peers' carets into
//! [`Collaborator`]s for rendering. The state uses y-prosemirror's shape
//! (`user` and `cursor: { anchor, head }`), so JavaScript cursor plugins read
//! it too.

mod binding;
mod claims;
mod error;
mod history;
pub mod keys;
mod local;
mod mirror;
mod presence;
mod registry;
mod remote;
mod reorder;
#[cfg(feature = "scene")]
pub mod scene;
mod snapshot;
mod sync;

mod text;
mod tree;
mod values;

pub use binding::Binding;
pub use binding::Encoding;
pub use binding::Track;
pub use error::CollabError;
pub use history::History;
pub use presence::Collaborator;
pub use presence::Presence;
pub use presence::User;
pub use reorder::Reorder;
#[cfg(feature = "scene")]
pub use scene::LentDocument;
#[cfg(feature = "scene")]
pub use scene::LentText;
#[cfg(feature = "scene")]
pub use scene::SceneBinding;
pub use sync::Framing;
pub use sync::Received;
pub use sync::SyncSession;
pub use yrs;
