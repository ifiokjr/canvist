//! The platform's standard text-editing keys.

use std::sync::Arc;

use canvist_model::Schema;

use crate::Binding;
use crate::Command;
use crate::Context;
use crate::Direction;
use crate::EditorError;
use crate::Extension;
use crate::Platform;
use crate::Unit;
use crate::command;
use crate::commands;
use crate::motion;

/// Arrow keys, Home and End, Backspace and Delete with their word and line
/// variants, Enter, Select All, and Escape, bound the way each platform
/// binds them. It has the lowest priority, so extensions override any key.
#[derive(Clone, Copy, Debug, Default)]
pub struct BaseKeymap;

fn caret(direction: Direction, unit: Unit, extend: bool) -> Command {
	Arc::new(move |cx: &mut Context<'_>| {
		motion::move_caret(cx.state, direction, unit, extend, cx.lines)
	})
}

fn vertical(direction: Direction, extend: bool) -> Command {
	Arc::new(move |cx: &mut Context<'_>| {
		motion::move_vertical(cx.state, direction, extend, cx.lines)
	})
}

fn delete(direction: Direction, unit: Unit) -> Command {
	Arc::new(move |cx: &mut Context<'_>| {
		match direction {
			Direction::Backward => commands::delete_backward(cx.state, unit, cx.lines),
			Direction::Forward => commands::delete_forward(cx.state, unit, cx.lines),
		}
	})
}

/// Bindings for one motion `unit`: plain moves, Shift extends.
fn motion_pair(bindings: &mut Vec<Binding>, keys: [&'static str; 4], unit: Unit) {
	let [backward, forward, extend_backward, extend_forward] = keys;
	bindings.push((backward, caret(Direction::Backward, unit, false)));
	bindings.push((forward, caret(Direction::Forward, unit, false)));
	bindings.push((extend_backward, caret(Direction::Backward, unit, true)));
	bindings.push((extend_forward, caret(Direction::Forward, unit, true)));
}

fn common() -> Vec<Binding> {
	let mut bindings = vec![
		("Backspace", delete(Direction::Backward, Unit::Grapheme)),
		(
			"Shift-Backspace",
			delete(Direction::Backward, Unit::Grapheme),
		),
		("Delete", delete(Direction::Forward, Unit::Grapheme)),
		("Enter", command(commands::lift_empty_block)),
		("Enter", command(commands::split_block)),
		("Mod-a", command(commands::select_all)),
		("Escape", command(commands::select_parent)),
		("ArrowUp", vertical(Direction::Backward, false)),
		("ArrowDown", vertical(Direction::Forward, false)),
		("Shift-ArrowUp", vertical(Direction::Backward, true)),
		("Shift-ArrowDown", vertical(Direction::Forward, true)),
	];
	let arrows = [
		"ArrowLeft",
		"ArrowRight",
		"Shift-ArrowLeft",
		"Shift-ArrowRight",
	];
	let home_end = ["Home", "End", "Shift-Home", "Shift-End"];
	motion_pair(&mut bindings, arrows, Unit::Grapheme);
	motion_pair(&mut bindings, home_end, Unit::Line);
	bindings
}

fn apple() -> Vec<Binding> {
	let mut bindings = vec![
		("Alt-Backspace", delete(Direction::Backward, Unit::Word)),
		("Alt-Delete", delete(Direction::Forward, Unit::Word)),
		("Mod-Backspace", delete(Direction::Backward, Unit::Line)),
		("Mod-Delete", delete(Direction::Forward, Unit::Line)),
		("Ctrl-h", delete(Direction::Backward, Unit::Grapheme)),
		("Ctrl-d", delete(Direction::Forward, Unit::Grapheme)),
		("Ctrl-k", delete(Direction::Forward, Unit::Line)),
		("Ctrl-a", caret(Direction::Backward, Unit::Line, false)),
		("Ctrl-e", caret(Direction::Forward, Unit::Line, false)),
	];
	let words = [
		"Alt-ArrowLeft",
		"Alt-ArrowRight",
		"Alt-Shift-ArrowLeft",
		"Alt-Shift-ArrowRight",
	];
	let lines = [
		"Mod-ArrowLeft",
		"Mod-ArrowRight",
		"Mod-Shift-ArrowLeft",
		"Mod-Shift-ArrowRight",
	];
	let document = [
		"Mod-ArrowUp",
		"Mod-ArrowDown",
		"Mod-Shift-ArrowUp",
		"Mod-Shift-ArrowDown",
	];
	motion_pair(&mut bindings, words, Unit::Word);
	motion_pair(&mut bindings, lines, Unit::Line);
	motion_pair(&mut bindings, document, Unit::Document);
	bindings
}

fn other() -> Vec<Binding> {
	let mut bindings = vec![
		("Ctrl-Backspace", delete(Direction::Backward, Unit::Word)),
		("Ctrl-Delete", delete(Direction::Forward, Unit::Word)),
	];
	let words = [
		"Ctrl-ArrowLeft",
		"Ctrl-ArrowRight",
		"Ctrl-Shift-ArrowLeft",
		"Ctrl-Shift-ArrowRight",
	];
	let document = ["Ctrl-Home", "Ctrl-End", "Ctrl-Shift-Home", "Ctrl-Shift-End"];
	motion_pair(&mut bindings, words, Unit::Word);
	motion_pair(&mut bindings, document, Unit::Document);
	bindings
}

impl Extension for BaseKeymap {
	fn name(&self) -> &'static str {
		"baseKeymap"
	}

	fn priority(&self) -> i32 {
		0
	}

	fn keymap(&self, _schema: &Schema, platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let mut bindings = common();
		bindings.extend(match platform {
			Platform::Apple => apple(),
			Platform::Other => other(),
		});

		Ok(bindings)
	}
}

/// Backspace right after an input rule restores the text that triggered
/// it, so `## ` then Backspace leaves a literal `## `. It runs before every
/// other Backspace binding, such as joining list items, because the rule's
/// effect is what the user is taking back.
#[derive(Clone, Copy, Debug, Default)]
pub struct UndoInputRule;

impl Extension for UndoInputRule {
	fn name(&self) -> &'static str {
		"undoInputRule"
	}

	fn priority(&self) -> i32 {
		i32::MAX
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![(
			"Backspace",
			Arc::new(crate::undo_input_rule) as Command,
		)])
	}
}

/// The standard keys add no commands of their own, so their handle is just
/// the proof of registration.
impl crate::TypedExtension for BaseKeymap {
	type Handle = crate::Registration;

	fn handle(&self, registration: crate::Registration) -> crate::Registration {
		registration
	}
}

impl crate::TypedExtension for UndoInputRule {
	type Handle = crate::Registration;

	fn handle(&self, registration: crate::Registration) -> crate::Registration {
		registration
	}
}
