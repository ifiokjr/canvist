//! The editor as the iOS and Android views drive it, with a fixed-width
//! measurer standing in for CoreText and `Paint`.

use std::sync::Arc;
use std::sync::Mutex;

use canvist_ffi::CanvistError;
use canvist_ffi::Command;
use canvist_ffi::DisplayItem;
use canvist_ffi::DocFormat;
use canvist_ffi::DocPosition;
use canvist_ffi::Editor;
use canvist_ffi::EditorChange;
use canvist_ffi::EditorObserver;
use canvist_ffi::EditorOptions;
use canvist_ffi::FillKind;
use canvist_ffi::LineKind;
use canvist_ffi::Modifiers;
use canvist_ffi::OffsetRange;
use canvist_ffi::TextSelection;
use canvist_ffi::ThemeOptions;
use canvist_ffi::testing::FixedPlatformMeasurer;

type Result = std::result::Result<(), CanvistError>;

fn editor(markdown: &str) -> Arc<Editor> {
	let editor = Editor::new(
		EditorOptions::starter_kit(),
		Arc::new(FixedPlatformMeasurer::default()),
	)
	.expect("the StarterKit is valid");
	editor
		.load(DocFormat::Markdown, markdown)
		.expect("valid markdown");
	editor.set_viewport(400.0, 600.0);
	editor
}

fn command(name: &str, args: &str) -> Command {
	Command {
		name: name.to_owned(),
		args: args.to_owned(),
	}
}

fn caret(editor: &Editor) -> u32 {
	let selection = editor.selection();
	assert_eq!(selection.anchor, selection.head, "expected a caret");
	selection.head
}

fn texts(items: &[DisplayItem]) -> Vec<&str> {
	items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Text { text, .. } => Some(text.as_str()),
				_ => None,
			}
		})
		.collect()
}

#[test]
fn typing_runs_input_rules_and_line_breaks_press_enter() -> Result {
	let editor = editor("");

	editor.insert_text("## ")?;
	editor.insert_text("Title\nfirst\n")?;
	editor.insert_text("- ")?;
	editor.insert_text("item")?;

	assert_eq!(
		editor.export(DocFormat::Markdown),
		"## Title\n\nfirst\n\n- item"
	);
	assert_eq!(
		editor.plain_text(),
		"Title\nfirst\nitem\n",
		"the trailing node adds a paragraph after the list"
	);
	assert_eq!(caret(&editor), editor.text_length() - 1);
	Ok(())
}

#[test]
fn text_offsets_address_blocks_joined_by_line_feeds() -> Result {
	let editor = editor("# Hi\n\nab😀c");

	assert_eq!(editor.text_length(), 2 + 1 + 5);
	assert_eq!(editor.text_in(OffsetRange { start: 1, end: 5 }), "i\nab");

	editor.select(5, 7)?;
	assert_eq!(editor.selection(), TextSelection { anchor: 5, head: 7 });
	assert_eq!(
		editor.selected_positions(),
		Some(vec![
			DocPosition {
				path: vec![1],
				offset: 2
			},
			DocPosition {
				path: vec![1],
				offset: 4
			},
		])
	);

	editor.select(8, 3)?;
	assert_eq!(editor.selection(), TextSelection { anchor: 8, head: 3 });
	Ok(())
}

#[test]
fn autocorrect_replaces_a_range_and_dictation_inserts() -> Result {
	let editor = editor("teh cat");
	editor.select(7, 7)?;

	editor.replace(OffsetRange { start: 0, end: 3 }, "the")?;
	assert_eq!(editor.plain_text(), "the cat");

	editor.select(7, 7)?;
	editor.insert_text(" sat on the mat")?;
	assert_eq!(editor.plain_text(), "the cat sat on the mat");

	editor.replace(OffsetRange { start: 3, end: 7 }, "")?;
	assert_eq!(editor.plain_text(), "the sat on the mat");
	Ok(())
}

#[test]
fn marked_text_composes_and_commits() -> Result {
	let editor = editor("A");
	editor.select(1, 1)?;

	editor.set_marked_text("k", OffsetRange { start: 1, end: 1 })?;
	editor.set_marked_text("か", OffsetRange { start: 1, end: 1 })?;
	assert_eq!(
		editor.marked_range(),
		Some(OffsetRange { start: 1, end: 2 })
	);
	assert_eq!(editor.plain_text(), "Aか");

	editor.set_marked_text("漢字", OffsetRange { start: 0, end: 2 })?;
	assert_eq!(editor.selection(), TextSelection { anchor: 1, head: 3 });

	editor.insert_text("漢字")?;
	assert_eq!(editor.marked_range(), None);
	assert_eq!(editor.plain_text(), "A漢字");
	assert_eq!(caret(&editor), 3);

	assert!(editor.undo()?);
	assert_eq!(
		editor.plain_text(),
		"A",
		"the composition undoes as one step"
	);
	Ok(())
}

#[test]
fn unmarking_keeps_the_text_and_the_caret() -> Result {
	let editor = editor("");

	editor.set_marked_text("nihon", OffsetRange { start: 2, end: 2 })?;
	assert_eq!(caret(&editor), 2);
	editor.unmark_text()?;

	assert_eq!(editor.marked_range(), None);
	assert_eq!(editor.plain_text(), "nihon");
	assert_eq!(caret(&editor), 2);
	Ok(())
}

#[test]
fn a_composing_region_is_replaced_by_the_next_marked_text() -> Result {
	let editor = editor("helo world");
	editor.select(4, 4)?;

	editor.set_composing_region(Some(OffsetRange { start: 0, end: 4 }))?;
	assert_eq!(
		editor.marked_range(),
		Some(OffsetRange { start: 0, end: 4 })
	);
	assert_eq!(editor.plain_text(), "helo world", "marking changes nothing");

	editor.set_marked_text("hello", OffsetRange { start: 5, end: 5 })?;
	editor.unmark_text()?;

	assert_eq!(editor.plain_text(), "hello world");
	assert_eq!(caret(&editor), 5);
	Ok(())
}

#[test]
fn backspace_deletes_and_joins_blocks() -> Result {
	let editor = editor("ab\n\ncd");
	editor.select(3, 3)?;

	editor.delete_backward()?;
	assert_eq!(editor.plain_text(), "abcd");
	editor.delete_backward()?;
	assert_eq!(editor.plain_text(), "acd");
	Ok(())
}

#[test]
fn typed_commands_format_and_queries_answer() -> Result {
	let editor = editor("Hello world");
	editor.select(6, 11)?;

	assert!(editor.can(command("toggleBold", "null"))?);
	assert!(editor.run(command("toggleBold", "null"))?);
	assert_eq!(editor.query("isBold", "null")?, "true");
	assert!(editor.run(command("toggleHeading", r#"{"level":2}"#))?);
	assert_eq!(editor.query("headingLevel", "{}")?, "2");
	assert_eq!(editor.export(DocFormat::Markdown), "## Hello **world**");

	let error = editor
		.run(command("toggleHeading", r#"{"level":9}"#))
		.expect_err("level 9 is out of range");
	assert_eq!(error.code(), "invalidArgs");
	let error = editor
		.run(command("explode", "null"))
		.expect_err("no such command");
	assert_eq!(error.code(), "unknownName");
	let error = editor
		.run(command("toggleBold", "{"))
		.expect_err("not JSON");
	assert_eq!(error.code(), "invalidArgs");
	Ok(())
}

#[test]
fn key_bindings_use_the_platforms_mod_key() -> Result {
	let mut options = EditorOptions::starter_kit();
	options.platform = canvist_ffi::KeyPlatform::Apple;
	let editor = Editor::new(options, Arc::new(FixedPlatformMeasurer::default()))?;
	editor.load(DocFormat::Markdown, "bold")?;
	editor.select(0, 4)?;

	let command_key = Modifiers {
		meta: true,
		..Modifiers::default()
	};
	assert!(editor.press("b", command_key)?);
	assert_eq!(editor.export(DocFormat::Markdown), "**bold**");
	assert!(
		!editor.press("F13", Modifiers::default())?,
		"unbound keys fall through"
	);
	Ok(())
}

#[test]
fn undo_and_redo() -> Result {
	let editor = editor("");
	assert!(!editor.can_undo());

	editor.insert_text("one")?;
	assert!(editor.can_undo());
	assert!(editor.undo()?);
	assert_eq!(editor.plain_text(), "");
	assert!(editor.can_redo());
	assert!(editor.redo()?);
	assert_eq!(editor.plain_text(), "one");
	Ok(())
}

#[test]
fn frames_paint_the_viewport_and_then_only_what_changed() -> Result {
	let editor = editor("first\n\nsecond");
	editor.set_focused(true);

	let frame = editor.frame()?;
	assert!(frame.full);
	assert!(frame.measured > 0);
	assert!(frame.height > 0.0);
	let items = frame.items.expect("the first frame paints");
	assert_eq!(texts(&items), ["first", "second"]);
	assert!(items.iter().any(|item| {
		matches!(
			item,
			DisplayItem::Fill {
				kind: FillKind::Caret,
				..
			}
		)
	}));

	let idle = editor.frame()?;
	assert_eq!(idle.items, None, "nothing changed");
	assert!(!idle.full);

	editor.select(13, 13)?;
	editor.insert_text("!")?;
	let typed = editor.frame()?;
	assert!(!typed.full, "typing repaints only the block it changed");
	assert!(!typed.damage.is_empty());
	assert_eq!(typed.measured, 1, "one new segment, measured in one call");
	assert_eq!(texts(&typed.items.expect("items")), ["first", "second!"]);
	Ok(())
}

#[test]
fn the_caret_blinks_and_hides_without_focus() -> Result {
	let editor = editor("text");
	let has_caret = |editor: &Editor| -> std::result::Result<bool, CanvistError> {
		editor.set_scroll(0.0);
		let frame = editor.frame()?;
		Ok(frame.items.unwrap_or_default().iter().any(|item| {
			matches!(
				item,
				DisplayItem::Fill {
					kind: FillKind::Caret,
					..
				}
			)
		}))
	};

	assert!(!has_caret(&editor)?, "no focus, no caret");
	editor.set_focused(true);
	editor.set_scroll(1.0);
	assert!(has_caret(&editor)?);
	editor.set_caret_visible(false);
	editor.set_scroll(1.0);
	assert!(!has_caret(&editor)?);
	Ok(())
}

#[test]
fn marked_text_is_underlined() -> Result {
	let editor = editor("");
	editor.set_marked_text("にほん", OffsetRange { start: 3, end: 3 })?;

	let frame = editor.frame()?;
	let underlined = frame.items.unwrap_or_default().iter().any(|item| {
		matches!(
			item,
			DisplayItem::Line {
				kind: LineKind::Underline,
				..
			}
		)
	});
	assert!(underlined);
	Ok(())
}

#[test]
fn hit_testing_and_geometry() -> Result {
	let editor = editor("hello world");
	editor.frame()?;

	let rect = editor.caret_rect(0).expect("a caret rect");
	let end = editor.caret_rect(5).expect("a caret rect");
	assert!(end.x > rect.x);
	assert_eq!(editor.offset_at(end.x, end.y + end.height / 2.0), Some(5));
	assert_eq!(editor.offset_at(-50.0, 1.0), Some(0));

	let rects = editor.selection_rects(OffsetRange { start: 0, end: 5 });
	assert_eq!(rects.len(), 1);
	assert!((rects[0].width - (end.x - rect.x)).abs() < 1.0);
	assert_eq!(
		editor.first_rect(OffsetRange { start: 2, end: 2 }),
		editor.caret_rect(2)
	);
	assert_eq!(editor.word_range(8), OffsetRange { start: 6, end: 11 });
	assert_eq!(editor.block_range(3), OffsetRange { start: 0, end: 11 });

	editor.select(0, 5)?;
	assert!(editor.selection_bounds().is_some());
	assert!(editor.is_in_selection(rect.x + 10.0, rect.y + 2.0));
	assert!(!editor.is_in_selection(end.x + 30.0, end.y + 2.0));
	Ok(())
}

#[test]
fn moving_by_visual_lines() -> Result {
	let editor = editor(&"word ".repeat(40));
	editor.frame()?;

	let below = editor
		.vertical_offset(2, false, None)
		.expect("a second line");
	assert!(below > 2);
	assert_eq!(editor.vertical_offset(below, true, None), Some(2));
	assert_eq!(
		editor.vertical_offset(2, true, None),
		None,
		"the first line"
	);
	let end = editor.line_edge_offset(2, true).expect("a line end");
	assert!(end > 2 && end < below);
	assert_eq!(
		editor.line_edge_offset(end - 1, false),
		Some(0),
		"the line's end offset is also the next line's start"
	);
	Ok(())
}

#[test]
fn blocks_and_lines_for_screen_readers() -> Result {
	let editor = editor("# Title\n\n- [x] done\n\n1. first\n\n> quoted\n\n```\ncode\n```");
	editor.set_viewport(400.0, 2000.0);
	let frame = editor.frame()?;

	let roles: Vec<_> = editor
		.blocks(0.0, frame.height)
		.into_iter()
		.map(|block| block.role)
		.collect();
	assert_eq!(
		roles,
		[
			canvist_ffi::BlockRole::Heading { level: 1 },
			canvist_ffi::BlockRole::ListItem {
				ordered: false,
				checked: Some(true)
			},
			canvist_ffi::BlockRole::ListItem {
				ordered: true,
				checked: None
			},
			canvist_ffi::BlockRole::Quote,
			canvist_ffi::BlockRole::Code,
		]
	);

	let lines = editor.lines(0.0, frame.height);
	let texts: Vec<String> = lines
		.iter()
		.map(|line| editor.text_in(line.range))
		.collect();
	assert_eq!(texts, ["Title", "done", "first", "quoted", "code"]);
	assert!(
		lines
			.windows(2)
			.all(|pair| pair[0].bounds.y < pair[1].bounds.y)
	);
	Ok(())
}

#[test]
fn themes_restyle_without_losing_the_document() -> Result {
	let editor = editor("text");
	let light = editor.frame()?;

	editor.set_theme(ThemeOptions {
		dark: true,
		font_size: 24.0,
		..ThemeOptions::default()
	})?;
	let dark = editor.frame()?;

	assert!(dark.full);
	assert_ne!(light.background, dark.background);
	assert!(dark.height > light.height);
	assert_eq!(editor.plain_text(), "text");

	let error = editor
		.set_theme(ThemeOptions {
			font_size: 0.0,
			..ThemeOptions::default()
		})
		.expect_err("a zero font size");
	assert_eq!(error.code(), "invalidArgs");
	Ok(())
}

#[derive(Default)]
struct Recorder {
	changes: Mutex<Vec<EditorChange>>,
	editor: Mutex<Option<Arc<Editor>>>,
	seen: Mutex<Vec<String>>,
}

impl EditorObserver for Recorder {
	fn changed(&self, change: EditorChange) {
		self.changes.lock().expect("lock").push(change);

		// Observers may call back in: the editor is unlocked by now.
		if let Some(editor) = self.editor.lock().expect("lock").as_ref() {
			self.seen.lock().expect("lock").push(editor.plain_text());
		}
	}
}

#[test]
fn observers_hear_about_changes_and_may_call_back_in() -> Result {
	let editor = editor("");
	let recorder = Arc::new(Recorder::default());
	*recorder.editor.lock().expect("lock") = Some(editor.clone());
	let id = editor.add_observer(recorder.clone());

	editor.insert_text("a")?;
	editor.select(0, 0)?;
	editor.set_marked_text("x", OffsetRange { start: 1, end: 1 })?;
	editor.remove_observer(id);
	editor.insert_text("ignored")?;

	let changes = recorder.changes.lock().expect("lock").clone();
	assert_eq!(changes.len(), 3);
	assert!(changes[0].document && changes[0].selection && !changes[0].updates);
	assert!(!changes[1].document && changes[1].selection);
	assert!(changes[2].composition);
	assert_eq!(recorder.seen.lock().expect("lock")[0], "a");
	*recorder.editor.lock().expect("lock") = None;
	Ok(())
}

#[test]
fn two_editors_collaborate_over_yjs() -> Result {
	let first = editor("Hello");
	first.share(1)?;
	let second = editor("");
	second.join(first.encode_state()?, 2)?;
	assert_eq!(second.plain_text(), "Hello");

	first.select(5, 5)?;
	first.insert_text(" world")?;

	for update in first.take_updates()? {
		assert!(second.apply_update(update)?);
	}

	second.select(0, 0)?;
	second.insert_text("Oh, ")?;

	for update in second.take_updates()? {
		first.apply_update(update)?;
	}

	assert_eq!(first.plain_text(), "Oh, Hello world");
	assert_eq!(second.plain_text(), "Oh, Hello world");
	assert_eq!(caret(&first), 15, "the remote insert moved the caret along");

	let frame = second.frame()?;
	assert_eq!(
		texts(&frame.items.expect("items")).concat(),
		"Oh, Hello world"
	);

	assert!(second.undo()?, "undo only this editor's change");
	assert_eq!(second.plain_text(), "Hello world");
	Ok(())
}

#[test]
fn y_sync_between_two_editors() -> Result {
	let server = editor("shared");
	server.share(1)?;
	let client = editor("");
	client.join(server.encode_state()?, 2)?;

	// Each side asks for what it lacks; both are up to date.
	let answer = server.receive_sync(client.sync_start()?)?;

	for message in answer.reply {
		assert!(!client.receive_sync(message)?.changed);
	}

	client.select(6, 6)?;
	client.insert_text(" text")?;

	for message in client.flush_sync()? {
		let relayed = server.receive_sync(message)?;
		assert!(relayed.changed);
		assert_eq!(relayed.broadcast.len(), 1, "relayed to other connections");
	}

	assert_eq!(server.plain_text(), "shared text");
	Ok(())
}

#[test]
fn metadata_lists_the_registered_commands() {
	let editor = editor("");
	let metadata: serde_json::Value =
		serde_json::from_str(&editor.metadata()).expect("metadata is JSON");
	let names: Vec<&str> = metadata
		.as_array()
		.expect("an array")
		.iter()
		.filter_map(|extension| extension["name"].as_str())
		.collect();
	assert!(names.contains(&"bold") && names.contains(&"heading"));

	let error =
		canvist_ffi::extension_metadata(vec!["nope".to_owned()]).expect_err("unknown extension");
	assert_eq!(error.code(), "config");
}

#[test]
fn copy_cut_and_paste_carry_html_and_text() -> Result {
	let editor = editor("Hello **bold** world");
	assert_eq!(editor.copy()?, None, "nothing is selected");

	editor.select(6, 10)?;
	let copied = editor.copy()?.expect("a selection");
	assert_eq!(copied.text, "bold");
	assert!(
		copied.html.contains("<strong>bold</strong>"),
		"{}",
		copied.html
	);

	let cut = editor.cut()?.expect("a selection");
	assert_eq!(cut, copied);
	assert_eq!(editor.plain_text(), "Hello  world");

	editor.paste_html(&copied.html)?;
	assert_eq!(editor.export(DocFormat::Markdown), "Hello **bold** world");

	editor.select(0, 0)?;
	editor.paste_text("one\ntwo ")?;
	assert_eq!(editor.plain_text(), "one\ntwo Hello bold world");

	editor.paste_markdown("*it* ")?;
	assert!(editor.export(DocFormat::Markdown).contains("*it*"));
	Ok(())
}

#[test]
fn tapping_a_checkbox_toggles_its_task() -> Result {
	let editor = editor("- [ ] first\n- [x] second\n\nafter");
	editor.select(17, 17)?;
	let frame = editor.frame()?;
	let boxes: Vec<_> = frame
		.items
		.unwrap_or_default()
		.into_iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Checkbox { bounds, checked } => Some((bounds, checked)),
				_ => None,
			}
		})
		.collect();
	assert_eq!(boxes.len(), 2);

	let (first, checked) = boxes[0];
	assert!(!checked);
	assert!(editor.toggle_task_at(first.x + 1.0, first.y + 1.0)?);
	assert!(
		editor
			.export(DocFormat::Markdown)
			.starts_with("- [x] first")
	);
	assert_eq!(caret(&editor), 17, "the selection stays put");

	let (_, below) = boxes[1];
	assert!(below);
	let paragraph = editor.caret_rect(17).expect("laid out");
	assert!(
		!editor.toggle_task_at(0.0, paragraph.y + 1.0)?,
		"not a task"
	);
	Ok(())
}

#[test]
fn geometry_describes_the_document_straight_after_an_edit() -> Result {
	let editor = editor("hello");
	editor.frame()?;
	editor.select(5, 5)?;

	editor.insert_text(" world")?;
	let before_frame = editor.caret_rect(11).expect("a caret");
	editor.frame()?;
	let after_frame = editor.caret_rect(11).expect("a caret");

	assert_eq!(before_frame, after_frame);
	assert!(before_frame.x > 0.0, "not at the block's start");
	Ok(())
}
