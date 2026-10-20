//! Typography: smart quotes, dashes, arrows, fractions, and symbols as you
//! type.
//!
//! The rules are TipTap's, plus Remirror's `<->` arrow. A rule whose
//! pattern has a capture group replaces only that group, so `1/2 ` becomes
//! `½ ` and keeps its space. Backspace straight after a rule fires puts
//! back what was typed.
//!
//! Input rules only run when the typed character is punctuation or white
//! space, so typing letters and digits never pays for them. TipTap's rules
//! that end in a digit (`2x3`, `^2`, `^3`) therefore fire on the space or
//! punctuation typed after them instead.

use std::sync::Arc;

use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Lookback;
use canvist_editor::Registration;
use canvist_editor::RuleMatch;
use canvist_editor::TypedExtension;
use canvist_editor::commands;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;

/// What ends a word, so a rule ending in a digit can fire.
const BOUNDARY: &str = r"[\s.,;:!?)\]]";
/// The characters [`BOUNDARY`] matches, which are the only typed characters
/// that can complete a bounded rule.
const BOUNDARY_CHARS: &str = " \t\n.,;:!?)]";
/// How much text a bounded rule needs before the caret: the longest is a
/// multiplication such as `1024 x 768`.
const BOUNDED_LOOKBACK: Lookback = Lookback::Bytes(24);

/// One rule: its TipTap option name, its pattern without the trailing `$`,
/// and what it produces.
#[derive(Clone, Copy, Debug)]
struct Rule {
	name: &'static str,
	pattern: &'static str,
	output: &'static str,
	/// Whether the pattern ends at a typed word boundary, which the rule
	/// keeps, rather than at the replaced text.
	bounded: bool,
	/// The typed character that completes a rule whose pattern ends in a
	/// group, such as an opening quote.
	trigger: Option<&'static str>,
}

const fn rule(name: &'static str, pattern: &'static str, output: &'static str) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: false,
		trigger: None,
	}
}

/// A rule completed by typing `trigger`, whose pattern cannot say so.
const fn on(
	trigger: &'static str,
	name: &'static str,
	pattern: &'static str,
	output: &'static str,
) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: false,
		trigger: Some(trigger),
	}
}

const fn bounded(name: &'static str, pattern: &'static str, output: &'static str) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: true,
		trigger: Some(BOUNDARY_CHARS),
	}
}

/// Every rule, in the order they are tried. Opening quotes come before
/// closing ones, and `←>` before `->`.
const RULES: [Rule; 23] = [
	rule("emDash", "--", "—"),
	rule("ellipsis", r"\.\.\.", "…"),
	on("\"", "openDoubleQuote", r#"(?:^|[\s{\[(<'"‘“])(")"#, "“"),
	rule("closeDoubleQuote", "\"", "”"),
	on("'", "openSingleQuote", r#"(?:^|[\s{\[(<'"‘“])(')"#, "‘"),
	rule("closeSingleQuote", "'", "’"),
	rule("leftArrow", "<-", "←"),
	rule("leftRightArrow", "←>", "↔"),
	rule("rightArrow", "->", "→"),
	rule("copyright", r"\(c\)", "©"),
	rule("trademark", r"\(tm\)", "™"),
	rule("servicemark", r"\(sm\)", "℠"),
	rule("registeredTrademark", r"\(r\)", "®"),
	bounded("oneHalf", r"(?:^|\s)(1/2)", "½"),
	rule("plusMinus", r"\+/-", "±"),
	rule("notEqual", "!=", "≠"),
	rule("laquo", "<<", "«"),
	rule("raquo", ">>", "»"),
	bounded("multiplication", r"\d+\s?([*x])\s?\d+", "×"),
	bounded("superscriptTwo", r"(\^2)", "²"),
	bounded("superscriptThree", r"(\^3)", "³"),
	bounded("oneQuarter", r"(?:^|\s)(1/4)", "¼"),
	bounded("threeQuarters", r"(?:^|\s)(3/4)", "¾"),
];

/// Smart punctuation as you type, as in TipTap's Typography.
///
/// ```
/// use canvist_extensions::Typography;
///
/// // Keep straight quotes and use guillemets for `<<` and `>>`.
/// let typography = Typography::default()
/// 	.without(&["openDoubleQuote", "closeDoubleQuote"])
/// 	.with_output("laquo", "‹");
/// assert_eq!(typography.rule_names().count(), 21);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Typography {
	/// Rules turned off, by TipTap option name.
	pub disabled: Vec<&'static str>,
	/// Rules whose output is replaced, by TipTap option name.
	pub outputs: Vec<(&'static str, Arc<str>)>,
}

impl Typography {
	/// Turn off the rules named in `names`, such as `"emDash"`.
	#[must_use]
	pub fn without(mut self, names: &[&'static str]) -> Self {
		self.disabled.extend_from_slice(names);
		self
	}

	/// Make the rule named `name` produce `output` instead.
	#[must_use]
	pub fn with_output(mut self, name: &'static str, output: &str) -> Self {
		self.outputs.push((name, Arc::from(output)));
		self
	}

	/// The names of the rules that are on, in the order they are tried.
	pub fn rule_names(&self) -> impl Iterator<Item = &'static str> + '_ {
		self.rules().map(|(rule, _)| rule.name)
	}

	/// The rules that are on, with their outputs.
	fn rules(&self) -> impl Iterator<Item = (Rule, Arc<str>)> + '_ {
		RULES
			.into_iter()
			.filter(|rule| !self.disabled.contains(&rule.name))
			.map(|rule| {
				let output = self
					.outputs
					.iter()
					.find(|(name, _)| *name == rule.name)
					.map_or_else(|| Arc::from(rule.output), |(_, output)| output.clone());
				(rule, output)
			})
	}
}

/// Typed access to [`Typography`]. It has no commands: its rules fire as
/// you type.
#[derive(Clone, Copy, Debug)]
pub struct TypographyHandle {
	_registration: Registration,
}

/// Replace capture group 1 of `found` (or the whole match) with `output`,
/// keeping any typed text after the group, such as the space after `1/2`.
fn replace(state: &mut EditorState, found: &RuleMatch, output: &str) -> CommandResult {
	let Some(target) = found
		.groups
		.get(1)
		.or(found.groups.first())
		.cloned()
		.flatten()
	else {
		return Ok(None);
	};
	let block = found.block;
	let removed_end = target.end.min(found.caret);
	let kept_typed = target.end <= found.caret;
	let marks = commands::marks_at(state.doc(), Point::new(block, target.start));

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(block, target.start..removed_end)?;
		let mut end = tr.insert_text(Point::new(block, target.start), output, marks.clone())?;

		if kept_typed {
			let typed_at = Point::new(block, end.offset + found.caret - removed_end);
			end = tr.insert_text(typed_at, &found.typed, marks)?;
		}

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

impl Extension for Typography {
	fn name(&self) -> &'static str {
		"typography"
	}

	fn input_rules(&self, _schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		self.rules()
			.map(|(rule, output)| {
				let pattern = if rule.bounded {
					format!("{}{BOUNDARY}$", rule.pattern)
				} else {
					format!("{}$", rule.pattern)
				};
				let mut input =
					InputRule::new(&pattern, move |state, found| replace(state, found, &output))?;

				if let Some(trigger) = rule.trigger {
					input = input.on(trigger);
				}

				if rule.bounded {
					input = input.lookback(BOUNDED_LOOKBACK);
				}

				Ok(input)
			})
			.collect()
	}
}

impl TypedExtension for Typography {
	type Handle = TypographyHandle;

	fn handle(&self, registration: Registration) -> TypographyHandle {
		TypographyHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_pattern_compiles() {
		let schema = Schema::builder()
			.block(canvist_model::BlockSpec::text("paragraph"))
			.build()
			.expect("valid schema");
		let rules = Typography::default()
			.input_rules(&schema)
			.expect("patterns compile");

		assert_eq!(rules.len(), RULES.len());
	}
}
