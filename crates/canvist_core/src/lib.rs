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
//! - [`action`] — validated action envelope (`Action`, actor/meta/intent/args)
//! - [`runtime`] — deterministic runtime orchestration (`EditorRuntime`)
//! - [`operation`] — atomic edit operations with transaction support and replay log
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

pub mod action;
pub mod collaboration;
pub mod document;
pub mod event;
pub mod extension;
pub mod layout;
pub mod operation;
mod position;
pub mod runtime;
pub mod selection;
pub mod style;

// Re-export the primary types at the crate root for convenience.
pub use action::Action;
pub use action::ActionArgs;
pub use action::ActionId;
pub use action::ActionIntent;
pub use action::ActionMeta;
pub use action::ActionValidationContext;
pub use action::ActionValidationError;
pub use action::ActorId;
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
pub use extension::Command;
pub use extension::Extension;
pub use extension::ExtensionRuntime;
pub use extension::InputRule;
pub use extension::RuntimeTransaction;
pub use extension::TransactionHook;
pub use extension::TransactionMeta;
pub use operation::Change;
pub use operation::Patch;
pub use operation::Transaction;
pub use position::Position;
pub use runtime::EditorRuntime;
pub use runtime::Invalidation;
pub use runtime::RuntimeError;
pub use runtime::RuntimeOutput;
pub use selection::Selection;
pub use style::Color;
pub use style::FontWeight;
pub use style::Style;
