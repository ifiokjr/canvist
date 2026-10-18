//! Commands: plain functions that read an [`EditorState`](crate::EditorState),
//! decide whether they apply, and change it through one transaction.
//!
//! Every command returns a [`CommandResult`]: `Ok(Some(update))` when it
//! changed something, `Ok(None)` when it does not apply (so a keymap can try
//! the next binding), and `Err` only when the model rejected a step. Commands
//! that need block types, such as list commands, take them as arguments;
//! extensions resolve the types from the schema and bind the commands.

mod blocks;
mod compose;
mod delete;
mod insert;
mod lists;

mod marks;
mod paste;
mod select;
mod slice;
mod split;

pub use blocks::insert_leaf;
pub use blocks::is_block_active;
pub use blocks::lift_block;
pub use blocks::lift_empty_block;
pub use blocks::selected_text_blocks;
pub use blocks::set_block_attrs;
pub use blocks::set_block_type;
pub use blocks::toggle_block_type;
pub use blocks::toggle_wrap;
pub use blocks::wrap_in;
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
pub use lists::ListType;
pub use lists::join_list_item_backward;
pub use lists::lift_list_item;
pub use lists::sink_list_item;
pub use lists::split_list_item;
pub use lists::toggle_list;
pub use lists::wrap_in_list;
pub use marks::active_marks;
pub use marks::is_mark_active;
pub use marks::marks_at;
pub use marks::set_mark;
pub use marks::toggle_mark;
pub use marks::unset_mark;
pub use paste::replace_selection;
pub use select::select_all;
pub use select::select_blocks;
pub use select::select_parent;
pub use select::set_selection;
pub use slice::Slice;
pub use slice::slice_selection;
pub use split::split_block;

use crate::EditorError;
use crate::Update;

/// What running a command did: `Some` update when it applied, `None` when
/// it did not apply to the current state.
pub type CommandResult = Result<Option<Update>, EditorError>;
