//! # `canvist_core`
//!
//! Core document model, editor operations, and CRDT collaboration for the
//! canvist canvas editor.
//!
//! This crate provides the foundational data structures and algorithms for
//! building a canvas-based text editor with real-time collaboration:
//!
//! - [`Document`] — the hierarchical node tree representing rich text
//! - [`Selection`] and [`Position`] — cursor and range selection model
//! - [`Style`] — composable text styling with builder pattern
//! - [`operation`] — atomic edit operations with transaction support
//! - [`collaboration`] — Yjs CRDT integration for real-time sync
//! - [`layout`] — text layout computation with line breaking
//!
//! # Examples
//!
//! ```
//! use canvist_core::Document;
//! use canvist_core::Position;
//! use canvist_core::Selection;
//! use canvist_core::Style;
//!
//! let mut doc = Document::new();
//! doc.insert_text(Position::zero(), "Hello, canvist!");
//!
//! let style = Style::new().bold().font_size(24.0);
//! doc.apply_style(Selection::all(&doc), &style);
//!
//! assert_eq!(doc.plain_text(), "Hello, canvist!");
//! ```

pub mod collaboration;
pub mod document;
pub mod event;
pub mod layout;
pub mod operation;
mod position;
pub mod selection;
pub mod style;

// Re-export the primary types at the crate root for convenience.
pub use document::Document;
pub use document::Node;
pub use document::NodeId;
pub use document::NodeKind;
pub use event::CompositionPhase;
pub use event::EditorEvent;
pub use event::EditorKey;
pub use event::EventSource;
pub use event::Modifiers;
pub use event::PointerEvent;
pub use event::PointerPhase;
pub use position::Position;
pub use selection::Selection;
pub use style::Color;
pub use style::FontWeight;
pub use style::Style;
