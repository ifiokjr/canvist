//! Commands: plain functions that read an [`EditorState`](crate::EditorState),
//! decide whether they apply, and change it through one transaction.
//!
//! Every command returns a [`CommandResult`]: `Ok(Some(update))` when it
//! changed something, `Ok(None)` when it does not apply (so a keymap can try
//! the next binding), and `Err` only when the model rejected a step.

mod compose;
mod delete;
mod insert;
mod marks;
mod select;

pub use compose::cancel_composition;
pub use compose::commit_composition;
pub use compose::set_composition;
pub use delete::delete_backward;
pub use delete::delete_forward;
pub use delete::delete_selection;
pub use delete::join_backward;
pub use delete::join_forward;
pub use insert::insert_embed;
pub use insert::insert_text;
pub use marks::active_marks;
pub use marks::is_mark_active;
pub use marks::marks_at;
pub use marks::set_mark;
pub use marks::toggle_mark;
pub use marks::unset_mark;
pub use select::select_all;
pub use select::select_blocks;
pub use select::select_parent;
pub use select::set_selection;

use crate::EditorError;
use crate::Update;

/// What running a command did: `Some` update when it applied, `None` when
/// it did not apply to the current state.
pub type CommandResult = Result<Option<Update>, EditorError>;
