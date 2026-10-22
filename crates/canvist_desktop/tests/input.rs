//! Keyboard, clipboard, and input method events, driven headlessly.

mod support;

use canvist_desktop::ClipboardContent;
use canvist_desktop::DesktopEditor;
use canvist_desktop::Ime;
use canvist_desktop::InputEvent;
use canvist_desktop::KeyInput;
use canvist_headless::Engine;
use canvist_headless::canvist_core::Platform;
use canvist_layout::DisplayItem;
use canvist_layout::RectKind;
use support::CTRL;
use support::Driver;
use support::META;
use support::NONE;
use support::SHIFT;
use support::SHIFT_CTRL;

#[test]
fn typing_runs_input_rules() {
	let mut driver = Driver::new("");

	driver.type_text("## Plan");
	let _ = driver.press("Enter", NONE);
	driver.type_text("- milk");

	assert_eq!(driver.markdown(), "## Plan\n\n- milk");
}

#[test]
fn backspace_and_enter_edit_blocks() {
	let mut driver = Driver::new("Hello");
	let _ = driver.press("End", CTRL);

	let _ = driver.press("Backspace", NONE);
	let _ = driver.press("Enter", NONE);
	driver.type_text("there");

	assert_eq!(driver.markdown(), "Hell\n\nthere");
}

#[test]
fn mod_is_ctrl_on_linux_and_windows() {
	let mut driver = Driver::new("bold");
	let _ = driver.key("a", CTRL);

	assert!(driver.key("b", CTRL).consumed);
	assert_eq!(driver.markdown(), "**bold**");
	assert!(!driver.key("i", META).consumed, "Meta is not Mod here");
	assert_eq!(driver.markdown(), "**bold**", "and types nothing");
}

#[test]
fn mod_is_cmd_on_macos() {
	let options = support::options().platform(Platform::Apple);
	let mut driver = Driver::with_options("bold", support::WIDTH, support::HEIGHT, options);
	let _ = driver.key("a", META);

	assert!(driver.key("b", META).consumed);
	assert_eq!(driver.markdown(), "**bold**");
}

#[test]
fn undo_takes_back_a_typing_burst() {
	let mut driver = Driver::new("");
	driver.type_text("first");
	driver.now += std::time::Duration::from_secs(2);
	driver.type_text(" second");

	let _ = driver.key("z", CTRL);
	assert_eq!(driver.markdown(), "first");

	let _ = driver.key("z", SHIFT_CTRL);
	assert_eq!(driver.markdown(), "first second");
}

#[test]
fn arrows_move_by_visual_lines() {
	// Narrow enough that one paragraph wraps onto several lines.
	let mut driver = Driver::sized(&"word ".repeat(30), support::NARROW, support::HEIGHT);
	let _ = driver.press("Home", CTRL);
	let (_, first) = driver.caret();

	let _ = driver.press("ArrowDown", NONE);
	driver.frame();
	let (_, second) = driver.caret();

	assert!(
		second > first,
		"the caret moved down a line: {first} → {second}"
	);
	assert_eq!(
		driver
			.editor
			.doc()
			.selected_range()
			.map(|(_, head)| head.path),
		Some(vec![0]),
		"within the same paragraph"
	);
}

#[test]
fn shift_arrows_select() {
	let mut driver = Driver::new("Hello world");
	let _ = driver.press("Home", CTRL);

	for _ in 0..5 {
		let _ = driver.press("ArrowRight", SHIFT);
	}

	assert_eq!(driver.selected_text(), "Hello");
}

#[test]
fn copy_writes_html_and_text_and_paste_keeps_formatting() {
	let mut driver = Driver::new("Some **bold** words");
	let _ = driver.key("a", CTRL);

	assert!(driver.key("c", CTRL).consumed);
	let copied = driver.clipboard.content.clone().expect("copied");
	assert_eq!(copied.text, "Some bold words");
	assert!(
		copied
			.html
			.as_deref()
			.is_some_and(|html| html.contains("<strong>bold</strong>")),
		"{copied:?}"
	);

	// ArrowRight collapses the selection to its end.
	let _ = driver.press("ArrowRight", NONE);
	let _ = driver.press("Enter", NONE);
	assert!(driver.key("v", CTRL).consumed);
	assert_eq!(
		driver.markdown(),
		"Some **bold** words\n\nSome **bold** words"
	);
}

#[test]
fn shift_mod_v_pastes_plain_text() {
	let mut driver = Driver::new("");
	driver.clipboard.content = Some(ClipboardContent {
		html: Some("<p><em>shiny</em></p>".to_owned()),
		text: "shiny".to_owned(),
	});

	let _ = driver.key("V", SHIFT_CTRL);

	assert_eq!(driver.markdown(), "shiny");
}

#[test]
fn cut_removes_the_selection() {
	let mut driver = Driver::new("keep cut");
	let _ = driver.press("End", CTRL);

	for _ in 0..4 {
		let _ = driver.press("ArrowLeft", SHIFT);
	}

	assert!(driver.key("x", CTRL).repaint);
	assert_eq!(driver.markdown(), "keep");
	assert_eq!(driver.clipboard.content.expect("cut").text, " cut");
}

#[test]
fn copy_with_nothing_selected_leaves_the_clipboard_alone() {
	let mut driver = Driver::new("text");
	driver.clipboard.content = Some(ClipboardContent {
		html: None,
		text: "before".to_owned(),
	});

	assert!(!driver.key("c", CTRL).consumed);
	assert_eq!(driver.clipboard.content.expect("kept").text, "before");
}

#[test]
fn preedit_lives_in_the_document_until_committed() {
	let mut driver = Driver::new("");
	let _ = driver.ime(Ime::Enabled);

	let _ = driver.ime(Ime::Preedit("ni".to_owned()));
	assert!(driver.editor.is_composing());
	assert_eq!(driver.markdown(), "ni");
	driver.frame();
	let area = driver.editor.ime_area().expect("an IME area");
	assert!(area.width > 0.0, "the area spans the preedit: {area:?}");

	// winit clears the preedit right before committing.
	let _ = driver.ime(Ime::Preedit("你".to_owned()));
	let _ = driver.ime(Ime::Preedit(String::new()));
	let _ = driver.ime(Ime::Commit("你".to_owned()));

	assert!(!driver.editor.is_composing());
	assert_eq!(driver.markdown(), "你");
}

#[test]
fn an_empty_preedit_cancels_the_composition() {
	let mut driver = Driver::new("a");
	let _ = driver.press("End", CTRL);

	let _ = driver.ime(Ime::Preedit("か".to_owned()));
	let _ = driver.ime(Ime::Preedit(String::new()));

	assert!(!driver.editor.is_composing());
	assert_eq!(driver.markdown(), "a");
}

#[test]
fn the_composition_is_underlined() {
	let mut driver = Driver::new("");
	let _ = driver.ime(Ime::Preedit("kana".to_owned()));
	driver.frame();

	let mut recorder = canvist_paint::Recorder::new();
	driver.editor.paint(&mut recorder, kurbo::Affine::IDENTITY);
	let without = recorder.ops.len();

	let _ = driver.ime(Ime::Commit("kana".to_owned()));
	driver.frame();
	let mut recorder = canvist_paint::Recorder::new();
	driver.editor.paint(&mut recorder, kurbo::Affine::IDENTITY);

	assert_eq!(recorder.ops.len() + 1, without, "one underline less");
}

#[test]
fn the_caret_shows_only_while_focused() {
	let mut driver = Driver::new("text");
	let has_caret = |driver: &Driver| {
		driver.editor.display_list().items.iter().any(|item| {
			matches!(
				item,
				DisplayItem::Rect {
					kind: RectKind::Caret,
					..
				}
			)
		})
	};

	assert!(has_caret(&driver));

	let _ = driver.send(&InputEvent::Focus(false));
	driver.frame();

	assert!(!has_caret(&driver));
}

#[test]
fn page_down_moves_the_caret_a_screen() {
	let markdown: Vec<String> = (0..200).map(|index| format!("Line {index}")).collect();
	let mut driver = Driver::sized(&markdown.join("\n\n"), support::WIDTH, support::SHORT);
	let _ = driver.press("Home", CTRL);

	let _ = driver.press("PageDown", NONE);

	assert!(driver.editor.scroll_top() > 200.0);
	let (_, head) = driver.editor.doc().selected_range().expect("a caret");
	assert!(
		head.path[0] > 4,
		"moved down past the first screen: {head:?}"
	);
}

#[test]
fn keys_without_a_binding_are_left_to_the_host() {
	let mut driver = Driver::new("text");

	assert!(!driver.press("F5", NONE).consumed);
	assert!(
		!driver
			.send(&InputEvent::Key(KeyInput::text("q", CTRL)))
			.consumed
	);
	assert_eq!(driver.markdown(), "text");
}

#[test]
fn unregistered_extensions_have_no_commands() {
	use canvist_headless::canvist_extensions::Bold;
	use canvist_headless::canvist_extensions::Document;
	use canvist_headless::canvist_extensions::Paragraph;

	let engine = Engine::new(|builder| {
		builder.add(Document);
		builder.add(Paragraph);
		builder.add(Bold)
	})
	.expect("engine");
	let mut editor = DesktopEditor::new(engine, support::options()).expect("editor");
	let bold = editor.handles();

	editor
		.load(canvist_headless::Format::Text, "only bold")
		.expect("text");
	editor.select_all().expect("select");
	assert!(editor.run(&bold.toggle()).expect("toggle"));
	// `bold` is the only handle: `editor.handles().italic` does not compile.
	assert_eq!(
		editor.export(canvist_headless::Format::Markdown),
		"**only bold**"
	);
}
