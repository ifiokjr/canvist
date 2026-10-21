#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_model`
//!
//! The block-based rich-text document model behind the canvist editor.
//!
//! - A [`Schema`] declares which blocks, marks, and inline embeds exist. Apps
//!   build it from the extensions they use, so unused features cost nothing.
//! - A [`Document`] stores blocks by stable [`BlockId`]. Text blocks hold
//!   [`InlineText`]: one string plus run-length [`MarkSet`] formatting.
//! - [`Step`]s are the only way to change a document. Each is invertible,
//!   reports how it moves [`Point`]s, and maps one-to-one onto CRDT
//!   operations, so undo, collaboration, and rendering share one pipeline.
//! - A [`Transaction`] applies steps atomically and returns a [`Commit`]
//!   describing which blocks changed, which is what the layout engine uses to
//!   relayout only the affected blocks.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_model::Attrs;
//! use canvist_model::BlockSpec;
//! use canvist_model::Document;
//! use canvist_model::Mark;
//! use canvist_model::MarkSet;
//! use canvist_model::MarkSpec;
//! use canvist_model::Point;
//! use canvist_model::Schema;
//!
//! let schema = Arc::new(
//! 	Schema::builder()
//! 		.block(BlockSpec::text("paragraph"))
//! 		.mark(MarkSpec::new("bold"))
//! 		.build()?,
//! );
//! let bold = Mark::new(schema.mark_type("bold").expect("registered above"));
//! let paragraph = schema.default_text_block();
//!
//! let mut doc = Document::new(schema, 1);
//! let first = doc.first_text_block().expect("new documents have one block");
//!
//! let mut transaction = doc.transaction();
//! let end = transaction.insert_text(Point::new(first, 0), "Hello world", MarkSet::empty())?;
//! transaction.add_mark(first, 6..11, bold)?;
//! transaction.split(end, paragraph, Attrs::new())?;
//! let commit = transaction.commit();
//!
//! assert_eq!(doc.plain_text(), "Hello world\n");
//!
//! // Every commit carries its own undo.
//! doc.apply_steps(commit.undo_steps())?;
//! assert_eq!(doc.plain_text(), "");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod attrs;
mod block;
mod document;
mod id;
mod inline;
mod json;
mod mark;
mod savepoint;

mod schema;
mod selection;
mod step;
mod table;
#[cfg(test)]
mod testing;
mod transaction;

pub use attrs::Attrs;
pub use attrs::Value;
pub use block::Block;
pub use block::BlockTree;
pub use block::Content;
pub use block::TreeContent;
pub use document::Document;
pub use id::BlockId;
pub use id::IdGenerator;
pub use id::ParseBlockIdError;
pub use inline::EMBED_CHAR;
pub use inline::Embed;
pub use inline::InlineError;
pub use inline::InlineText;
pub use inline::RunRef;
pub use json::JsonError;
pub use mark::Mark;
pub use mark::MarkSet;
pub use savepoint::Savepoint;
pub use schema::BlockSpec;
pub use schema::BlockType;
pub use schema::BlockTypeId;
pub use schema::ContentKind;
pub use schema::ContentSpec;
pub use schema::EmbedSpec;
pub use schema::EmbedType;
pub use schema::EmbedTypeId;
pub use schema::MarkSpec;
pub use schema::MarkType;
pub use schema::MarkTypeId;
pub use schema::Schema;
pub use schema::SchemaBuilder;
pub use schema::SchemaError;
pub use selection::Bias;
pub use selection::Point;
pub use selection::Selection;
pub use step::Applied;
pub use step::Mapped;
pub use step::Step;
pub use step::StepError;
pub use step::StepMap;
pub use table::GridCell;
pub use table::GridRect;
pub use table::TableMap;
pub use transaction::Commit;
pub use transaction::Transaction;
