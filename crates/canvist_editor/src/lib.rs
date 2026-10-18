//! # `canvist_editor`
//!
//! The platform-agnostic editing engine behind canvist. It sits between the
//! document model ([`canvist_model`]) and the platform hosts, and owns
//! everything about *editing* that does not depend on pixels:
//!
//! - [`EditorState`]: the document, the selection, stored marks, and IME
//!   composition, changed only through [`EditorState::change`] and
//!   [`EditorState::apply_steps`].
//! - [`commands`]: plain functions such as typing, deleting by grapheme or
//!   word, splitting and joining blocks, lists, marks, and paste.
//! - [`motion`]: caret movement by grapheme, word, line, and document. Lines
//!   come from a [`LineNavigator`], which the layout engine implements.
//!
//! Grapheme and word boundaries follow Unicode (UAX #29), and inline embeds
//! behave as single atoms everywhere.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_editor::EditorState;
//! use canvist_editor::commands;
//! use canvist_model::BlockSpec;
//! use canvist_model::Document;
//! use canvist_model::Schema;
//!
//! let schema = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//! let mut state = EditorState::new(Document::new(schema, 1))?;
//!
//! commands::insert_text(&mut state, "Hello")?;
//! commands::split_block(&mut state)?;
//! commands::insert_text(&mut state, "world")?;
//!
//! assert_eq!(state.doc().plain_text(), "Hello\nworld");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod commands;
mod error;
pub mod motion;
pub mod position;
mod state;
mod structure;
#[cfg(test)]
mod testing;

pub mod text;
mod transaction;

pub use commands::CommandResult;
pub use commands::Slice;
pub use error::EditorError;
pub use motion::Direction;
pub use motion::LineNavigator;
pub use motion::ParagraphNavigator;
pub use motion::Unit;
pub use state::Composition;
pub use state::EditorState;
pub use transaction::EditKind;
pub use transaction::EditorTransaction;
pub use transaction::Origin;
pub use transaction::Update;
