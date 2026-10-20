#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
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
//!   come from a [`LineNavigator`], which the layout engine implements and
//!   hosts pass per event through [`Env::new`]; [`Env::headless`] falls back
//!   to [`ParagraphNavigator`]. [`Caret`] carries the [`Affinity`] that
//!   tells the two sides of a soft wrap apart.
//! - [`Editor`]: the state plus [`History`], a [`Keymap`] with platform-aware
//!   `Mod`, [`InputRule`]s, and [`PasteRule`]s, all contributed by
//!   [`Extension`]s. [`BaseKeymap`] binds the platform's standard keys and
//!   [`UndoInputRule`] lets Backspace take back an input rule.
//! - A headless, typed API: [`EditorBuilder::add`] returns each extension's
//!   typed handle; [`CommandSpec`] and [`QuerySpec`] give dynamic hosts
//!   argument-checked named commands and queries; [`Editor::metadata`]
//!   describes all of it for binding generators; and [`Editor::can`] and
//!   [`Editor::chain`] dry-run or combine commands without side effects,
//!   on top of the model's exact savepoints.
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
//!
//! An [`Editor`] adds keymaps, rules, and history on top:
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_editor::BaseKeymap;
//! use canvist_editor::Editor;
//! use canvist_editor::Env;
//! use canvist_editor::Extension;
//! use canvist_editor::KeyPress;
//! use canvist_model::BlockSpec;
//! use canvist_model::SchemaBuilder;
//!
//! struct Paragraphs;
//!
//! impl Extension for Paragraphs {
//! 	fn name(&self) -> &'static str {
//! 		"paragraphs"
//! 	}
//!
//! 	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
//! 		schema.block(BlockSpec::text("paragraph"))
//! 	}
//! }
//!
//! let mut editor = Editor::builder().extension(Paragraphs).extension(BaseKeymap).build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Hi", &mut env)?;
//! editor.handle_key(KeyPress::plain("Enter"), &mut env)?;
//! editor.undo(&mut env)?;
//!
//! assert_eq!(editor.state().doc().plain_text(), "Hi");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod base_keymap;
pub mod commands;
mod context;
mod editor;
mod error;
mod extension;
mod history;
mod keymap;
mod meta;
pub mod motion;
mod navigation;
pub mod position;
mod rules;
mod spec;
mod state;
mod structure;
#[cfg(test)]
mod testing;
pub mod text;
mod transaction;
mod typed;

pub use base_keymap::BaseKeymap;
pub use base_keymap::UndoInputRule;
pub use commands::CommandResult;
pub use commands::Slice;
pub use context::Command;
pub use context::Context;
pub use context::command;
pub use context::redo;
pub use context::undo;
pub use context::undo_input_rule;
pub use editor::Editor;
pub use editor::EditorBuilder;
pub use editor::Env;
pub use error::EditorError;
pub use extension::Binding;
pub use extension::Extension;
pub use extension::block_type;
pub use extension::embed_type;
pub use extension::mark_type;
pub use history::History;
pub use history::HistoryConfig;
pub use history::UndoHistory;
pub use keymap::KeyPress;
pub use keymap::Keymap;
pub use keymap::Modifiers;
pub use keymap::Platform;
pub use keymap::parse_binding;
pub use meta::CommandMeta;
pub use meta::ExtensionMeta;
pub use meta::Field;
pub use meta::NodeKind;
pub use meta::NodeMeta;
pub use meta::QueryMeta;
pub use meta::ValueType;
pub use meta::value_to_json;
pub use motion::Direction;
pub use motion::Unit;
pub use navigation::Affinity;
pub use navigation::Caret;
pub use navigation::LineEdge;
pub use navigation::LineNavigator;
pub use navigation::NavigationError;
pub use navigation::ParagraphNavigator;
pub use navigation::VerticalDirection;
pub use navigation::VerticalMotion;
pub use rules::DEFAULT_LOOKBACK;
pub use rules::InputRule;
pub use rules::Lookback;
pub use rules::MarkAttrs;
pub use rules::PasteRule;
pub use rules::RULE_CONTEXT;
pub use rules::RuleHandler;
pub use rules::RuleInput;
pub use rules::RuleMatch;
pub use spec::Args;
pub use spec::CommandBuilder;
pub use spec::CommandSpec;
pub use spec::QueryFn;
pub use spec::QuerySpec;
pub use state::Composition;
pub use state::EditorState;
pub use transaction::EditKind;
pub use transaction::EditorTransaction;
pub use transaction::Origin;
pub use transaction::Update;
pub use typed::Chain;
pub use typed::Registration;
pub use typed::TypedExtension;
