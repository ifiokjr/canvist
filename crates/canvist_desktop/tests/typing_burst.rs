//! Typing fast must never crash: random bursts of keys, clicks, drags,
//! input method events, pastes, scrolls, and resizes, with a frame (and
//! sometimes a paint and an accessibility update) between them, as a busy
//! window would interleave them.

mod support;

use canvist_desktop::ClipboardContent;
use canvist_desktop::Ime;
use canvist_desktop::InputEvent;
use canvist_desktop::KeyInput;
use canvist_desktop::Modifiers;
use canvist_desktop::PointerButton;
use canvist_desktop::ScrollDelta;
use canvist_desktop::Viewport;
use kurbo::Affine;
use kurbo::Point;
use proptest::prelude::*;
use support::Driver;

/// One thing a user does.
#[derive(Clone, Debug)]
enum Act {
	Event(InputEvent),
	Resize(f32),
	Paint,
	Accessibility,
	Undo,
}

fn modifiers() -> impl Strategy<Value = Modifiers> {
	(any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(shift, ctrl, alt)| {
		Modifiers {
			shift,
			ctrl,
			alt,
			meta: false,
		}
	})
}

fn key() -> impl Strategy<Value = InputEvent> {
	let named = prop::sample::select(vec![
		"Enter",
		"Backspace",
		"Delete",
		"Tab",
		"ArrowLeft",
		"ArrowRight",
		"ArrowUp",
		"ArrowDown",
		"Home",
		"End",
		"PageUp",
		"PageDown",
		"Escape",
	]);
	let typed = prop::sample::select(vec![
		"a", "b", "z", " ", "#", "-", ">", "*", "`", "[", "]", "1", ".", "é", "你", "😀", "ش", "c",
		"v", "x",
	]);

	prop_oneof![
		(named, modifiers())
			.prop_map(|(key, modifiers)| { InputEvent::Key(KeyInput::named(key, modifiers)) }),
		(typed, modifiers())
			.prop_map(|(text, modifiers)| InputEvent::Key(KeyInput::text(text, modifiers))),
	]
}

fn ime() -> impl Strategy<Value = InputEvent> {
	let text = prop::sample::select(vec!["", "ni", "你", "かな", "한"]);

	prop_oneof![
		text.clone()
			.prop_map(|text| InputEvent::Ime(Ime::Preedit(text.to_owned()))),
		text.prop_map(|text| InputEvent::Ime(Ime::Commit(text.to_owned()))),
		Just(InputEvent::Ime(Ime::Enabled)),
		Just(InputEvent::Ime(Ime::Disabled)),
	]
}

fn pointer() -> impl Strategy<Value = InputEvent> {
	let position = (-50.0..700.0f64, -50.0..500.0f64).prop_map(|(x, y)| Point::new(x, y));

	prop_oneof![
		(position.clone(), any::<bool>()).prop_map(|(position, shift)| {
			InputEvent::PointerDown {
				position,
				button: PointerButton::Primary,
				modifiers: Modifiers {
					shift,
					..Modifiers::NONE
				},
			}
		}),
		position.prop_map(|position| InputEvent::PointerMove { position }),
		Just(InputEvent::PointerUp {
			button: PointerButton::Primary,
		}),
		(-400.0..400.0f32).prop_map(|y| InputEvent::Scroll(ScrollDelta::Pixels { x: 0.0, y })),
		any::<bool>().prop_map(InputEvent::Focus),
	]
}

fn act() -> impl Strategy<Value = Act> {
	prop_oneof![
		8 => key().prop_map(Act::Event),
		2 => ime().prop_map(Act::Event),
		3 => pointer().prop_map(Act::Event),
		1 => (80.0..900.0f32).prop_map(Act::Resize),
		1 => Just(Act::Paint),
		1 => Just(Act::Accessibility),
		1 => Just(Act::Undo),
	]
}

/// Run `acts` against an editor, checking the document stays valid.
fn run(markdown: &str, acts: &[Act]) {
	let mut driver = Driver::new(markdown);
	driver.clipboard.content = Some(ClipboardContent {
		html: Some("<h2>Pasted</h2><ul><li><p>item</p></li></ul>".to_owned()),
		text: "Pasted\n\nitem".to_owned(),
	});

	for act in acts {
		match act {
			Act::Event(event) => {
				let _ = driver.send(event);
			}
			Act::Resize(width) => {
				driver.editor.resize(Viewport {
					width: *width,
					height: 400.0,
					scale: 2.0,
				});
			}
			Act::Paint => {
				let mut recorder = canvist_paint::Recorder::new();
				driver.editor.paint(&mut recorder, Affine::IDENTITY);
			}
			Act::Accessibility => {
				let _ = driver.editor.accessibility_tree("Notes", Affine::IDENTITY);
			}
			Act::Undo => {
				let _ = driver.editor.undo().expect("undo");
			}
		}

		driver.frame();
	}

	// Whatever happened, the layout still answers for the whole document.
	let doc = driver.editor.doc().document();
	assert!(!doc.children(None).is_empty());
	assert!(driver.editor.layout().height() > 0.0);
	let _ = driver.editor.export(canvist_headless::Format::Json);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn random_bursts_never_crash(acts in prop::collection::vec(act(), 1..160)) {
		run("# Notes\n\nSome **bold** text\n\n- one\n- two\n\n> quoted", &acts);
	}
}

#[test]
fn ten_thousand_keys_without_a_frame_in_between() {
	let mut driver = Driver::new("");
	let text = "The quick brown fox jumps over the lazy dog. ";

	for (index, character) in text.chars().cycle().take(10_000).enumerate() {
		let _ = driver.key(&character.to_string(), Modifiers::NONE);

		if index % 700 == 699 {
			let _ = driver.press("Enter", Modifiers::NONE);
		}
	}

	driver.frame();
	let markdown = driver.markdown();
	assert_eq!(
		markdown.chars().filter(|c| *c != '\n').count(),
		10_000,
		"every key landed"
	);
	let (_, y) = driver.caret();
	assert!((0.0..400.0).contains(&y), "the caret is on screen");
}
