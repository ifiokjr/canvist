//! First-class editor action envelope and deterministic event conversion.
//!
//! Actions are serializable, intent-level records produced from canonical
//! [`EditorEvent`](crate::EditorEvent) values.

use serde::Deserialize;
use serde::Serialize;

use crate::CompositionPhase;
use crate::EditorEvent;
use crate::EditorKey;
use crate::Modifiers;
use crate::PointerEvent;
use crate::Position;
use crate::Selection;

/// Stable identifier for an action.
pub type ActionId = String;

/// Actor/user identifier associated with an action.
pub type ActorId = String;

/// Envelope metadata required for deterministic replay and auditing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionMeta {
	pub id: ActionId,
	pub actor: ActorId,
	pub logical_clock: u64,
	pub timestamp_ms: i64,
}

/// Intent represented by an [`Action`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionIntent {
	TextInsert,
	TextDeleteBackward,
	TextDeleteForward,
	KeyDown,
	KeyUp,
	Pointer,
	SelectionSet,
	CursorMove,
	Composition,
	ClipboardCopy,
	ClipboardCut,
	ClipboardPaste,
	Focus,
	Blur,
}

/// Deterministically resolved arguments for an action intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionArgs {
	TextInsert {
		text: String,
	},
	TextDeleteBackward {
		count: usize,
	},
	TextDeleteForward {
		count: usize,
	},
	KeyDown {
		key: EditorKey,
		modifiers: Modifiers,
		repeat: bool,
	},
	KeyUp {
		key: EditorKey,
		modifiers: Modifiers,
	},
	Pointer(PointerEvent),
	SelectionSet {
		selection: Selection,
	},
	CursorMove {
		position: Position,
		extend: bool,
	},
	Composition {
		phase: CompositionPhase,
		text: String,
	},
	ClipboardCopy,
	ClipboardCut,
	ClipboardPaste {
		text: String,
	},
	Focus,
	Blur,
}

/// Serializable action envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
	pub meta: ActionMeta,
	pub intent: ActionIntent,
	pub args: ActionArgs,
}

/// Deterministic validation failure when deriving an action from an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionValidationError {
	EmptyActionId,
	EmptyActor,
	NonMonotonicClock,
	NegativeTimestamp,
	EmptyInsertedText,
	EmptyPasteText,
	ZeroDeleteCount,
	InvalidPointerCoordinates,
	InvalidPointerPressure,
	EmptyCompositionTextOnUpdateOrEnd,
}

/// Validation context for action metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionValidationContext {
	/// The previous logical clock observed for this actor/session.
	pub previous_logical_clock: Option<u64>,
}

impl Action {
	/// Creates an [`Action`] by deterministically mapping from an [`EditorEvent`]
	/// and validating envelope/argument invariants.
	pub fn from_event(
		event: EditorEvent,
		meta: ActionMeta,
		context: ActionValidationContext,
	) -> Result<Self, ActionValidationError> {
		validate_meta(&meta, context)?;

		let (intent, args) = map_event(event);
		validate_args(&args)?;

		Ok(Self { meta, intent, args })
	}
}

fn map_event(event: EditorEvent) -> (ActionIntent, ActionArgs) {
	match event {
		EditorEvent::TextInsert { text } => {
			(ActionIntent::TextInsert, ActionArgs::TextInsert { text })
		}
		EditorEvent::TextDeleteBackward { count } => {
			(
				ActionIntent::TextDeleteBackward,
				ActionArgs::TextDeleteBackward { count },
			)
		}
		EditorEvent::TextDeleteForward { count } => {
			(
				ActionIntent::TextDeleteForward,
				ActionArgs::TextDeleteForward { count },
			)
		}
		EditorEvent::KeyDown {
			key,
			modifiers,
			repeat,
		} => {
			(
				ActionIntent::KeyDown,
				ActionArgs::KeyDown {
					key,
					modifiers,
					repeat,
				},
			)
		}
		EditorEvent::KeyUp { key, modifiers } => {
			(ActionIntent::KeyUp, ActionArgs::KeyUp { key, modifiers })
		}
		EditorEvent::Pointer(pointer) => (ActionIntent::Pointer, ActionArgs::Pointer(pointer)),
		EditorEvent::SelectionSet { selection } => {
			(
				ActionIntent::SelectionSet,
				ActionArgs::SelectionSet { selection },
			)
		}
		EditorEvent::CursorMove { position, extend } => {
			(
				ActionIntent::CursorMove,
				ActionArgs::CursorMove { position, extend },
			)
		}
		EditorEvent::Composition { phase, text } => {
			(
				ActionIntent::Composition,
				ActionArgs::Composition { phase, text },
			)
		}
		EditorEvent::ClipboardCopy => (ActionIntent::ClipboardCopy, ActionArgs::ClipboardCopy),
		EditorEvent::ClipboardCut => (ActionIntent::ClipboardCut, ActionArgs::ClipboardCut),
		EditorEvent::ClipboardPaste { text } => {
			(
				ActionIntent::ClipboardPaste,
				ActionArgs::ClipboardPaste { text },
			)
		}
		EditorEvent::Focus => (ActionIntent::Focus, ActionArgs::Focus),
		EditorEvent::Blur => (ActionIntent::Blur, ActionArgs::Blur),
	}
}

fn validate_meta(
	meta: &ActionMeta,
	context: ActionValidationContext,
) -> Result<(), ActionValidationError> {
	if meta.id.trim().is_empty() {
		return Err(ActionValidationError::EmptyActionId);
	}
	if meta.actor.trim().is_empty() {
		return Err(ActionValidationError::EmptyActor);
	}
	if let Some(previous) = context.previous_logical_clock
		&& meta.logical_clock <= previous
	{
		return Err(ActionValidationError::NonMonotonicClock);
	}
	if meta.timestamp_ms < 0 {
		return Err(ActionValidationError::NegativeTimestamp);
	}

	Ok(())
}

fn validate_args(args: &ActionArgs) -> Result<(), ActionValidationError> {
	match args {
		ActionArgs::TextInsert { text } if text.is_empty() => {
			Err(ActionValidationError::EmptyInsertedText)
		}
		ActionArgs::ClipboardPaste { text } if text.is_empty() => {
			Err(ActionValidationError::EmptyPasteText)
		}
		ActionArgs::TextDeleteBackward { count } | ActionArgs::TextDeleteForward { count }
			if *count == 0 =>
		{
			Err(ActionValidationError::ZeroDeleteCount)
		}
		ActionArgs::Pointer(pointer) if !pointer.x.is_finite() || !pointer.y.is_finite() => {
			Err(ActionValidationError::InvalidPointerCoordinates)
		}
		ActionArgs::Pointer(pointer) if !pointer.pressure.is_finite() => {
			Err(ActionValidationError::InvalidPointerPressure)
		}
		ActionArgs::Composition { phase, text }
			if matches!(phase, CompositionPhase::Update | CompositionPhase::End)
				&& text.is_empty() =>
		{
			Err(ActionValidationError::EmptyCompositionTextOnUpdateOrEnd)
		}
		_ => Ok(()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::PointerPhase;

	fn valid_meta() -> ActionMeta {
		ActionMeta {
			id: "action-1".to_owned(),
			actor: "user-1".to_owned(),
			logical_clock: 2,
			timestamp_ms: 100,
		}
	}

	#[test]
	fn converts_event_to_action_deterministically() {
		let action = Action::from_event(
			EditorEvent::TextInsert {
				text: "hello".to_owned(),
			},
			valid_meta(),
			ActionValidationContext {
				previous_logical_clock: Some(1),
			},
		)
		.expect("event should convert");

		assert_eq!(action.intent, ActionIntent::TextInsert);
		assert_eq!(
			action.args,
			ActionArgs::TextInsert {
				text: "hello".to_owned(),
			}
		);
	}

	#[test]
	fn rejects_non_monotonic_clock() {
		let error = Action::from_event(
			EditorEvent::Focus,
			valid_meta(),
			ActionValidationContext {
				previous_logical_clock: Some(2),
			},
		)
		.expect_err("clock should be monotonic");

		assert_eq!(error, ActionValidationError::NonMonotonicClock);
	}

	#[test]
	fn rejects_invalid_pointer_coordinates() {
		let error = Action::from_event(
			EditorEvent::Pointer(PointerEvent {
				phase: PointerPhase::Move,
				pointer_id: 1,
				x: f64::NAN,
				y: 1.0,
				pressure: 0.5,
				modifiers: Modifiers::default(),
			}),
			valid_meta(),
			ActionValidationContext {
				previous_logical_clock: Some(1),
			},
		)
		.expect_err("invalid pointer should fail");

		assert_eq!(error, ActionValidationError::InvalidPointerCoordinates);
	}

	#[test]
	fn maps_events_to_expected_intent_and_args() {
		let composition = Action::from_event(
			EditorEvent::Composition {
				phase: CompositionPhase::Update,
				text: "候補".to_owned(),
			},
			valid_meta(),
			ActionValidationContext {
				previous_logical_clock: Some(1),
			},
		)
		.expect("composition should convert");
		assert_eq!(composition.intent, ActionIntent::Composition);
		assert_eq!(
			composition.args,
			ActionArgs::Composition {
				phase: CompositionPhase::Update,
				text: "候補".to_owned(),
			}
		);

		let paste = Action::from_event(
			EditorEvent::ClipboardPaste {
				text: "pasted".to_owned(),
			},
			ActionMeta {
				id: "action-2".to_owned(),
				actor: "user-1".to_owned(),
				logical_clock: 3,
				timestamp_ms: 101,
			},
			ActionValidationContext {
				previous_logical_clock: Some(2),
			},
		)
		.expect("paste should convert");
		assert_eq!(paste.intent, ActionIntent::ClipboardPaste);
		assert_eq!(
			paste.args,
			ActionArgs::ClipboardPaste {
				text: "pasted".to_owned(),
			}
		);
	}
}
