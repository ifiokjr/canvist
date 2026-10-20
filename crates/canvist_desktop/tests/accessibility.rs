//! The AccessKit tree and the actions screen readers send.

mod support;

use accesskit::Action;
use accesskit::ActionData;
use accesskit::ActionRequest;
use accesskit::Node;
use accesskit::NodeId;
use accesskit::Role;
use accesskit::TextPosition;
use accesskit::TextSelection;
use accesskit::Toggled;
use accesskit::TreeId;
use canvist_desktop::EDITOR_ID;
use canvist_desktop::InputEvent;
use canvist_desktop::WINDOW_ID;
use kurbo::Affine;
use support::Driver;

fn tree(driver: &mut Driver) -> Vec<(NodeId, Node)> {
	driver
		.editor
		.accessibility_tree("Notes", Affine::IDENTITY)
		.nodes
}

fn node(nodes: &[(NodeId, Node)], id: NodeId) -> &Node {
	&nodes
		.iter()
		.find(|(node, _)| *node == id)
		.expect("the node")
		.1
}

/// The text runs in tree order with their values.
fn runs(nodes: &[(NodeId, Node)]) -> Vec<(NodeId, String)> {
	let editor = node(nodes, EDITOR_ID);
	editor
		.children()
		.iter()
		.flat_map(|&block| node(nodes, block).children().to_vec())
		.map(|run| (run, node(nodes, run).value().unwrap_or_default().to_owned()))
		.collect()
}

fn request(action: Action, data: Option<ActionData>) -> ActionRequest {
	ActionRequest {
		action,
		target_tree: TreeId::ROOT,
		target_node: EDITOR_ID,
		data,
	}
}

#[test]
fn the_document_is_a_multiline_text_input_of_blocks() {
	let mut driver = Driver::new("# Title\n\nBody text\n\n- [x] done");
	let update = driver.editor.accessibility_tree("Notes", Affine::IDENTITY);
	let nodes = &update.nodes;

	assert_eq!(update.focus, EDITOR_ID, "the editor has focus");
	assert_eq!(node(nodes, WINDOW_ID).label(), Some("Notes"));
	assert_eq!(node(nodes, EDITOR_ID).role(), Role::MultilineTextInput);

	let blocks: Vec<&Node> = node(nodes, EDITOR_ID)
		.children()
		.iter()
		.map(|&id| node(nodes, id))
		.collect();
	assert_eq!(blocks[0].role(), Role::Heading);
	assert_eq!(blocks[0].level(), Some(1));
	assert_eq!(blocks[1].role(), Role::Paragraph);
	assert_eq!(blocks[2].role(), Role::CheckBox);
	assert_eq!(blocks[2].toggled(), Some(Toggled::True));

	let values: Vec<String> = runs(nodes).into_iter().map(|(_, value)| value).collect();
	assert_eq!(values, ["Title\n", "Body text\n", "done\n"]);
}

#[test]
fn wrapped_lines_are_separate_runs_with_positions() {
	let mut driver = Driver::sized(&"word ".repeat(30), support::NARROW, support::HEIGHT);
	let nodes = tree(&mut driver);
	let runs = runs(&nodes);

	assert!(runs.len() > 2, "{runs:?}");
	let first = node(&nodes, runs[0].0);
	let characters = first.character_lengths().len();
	assert_eq!(
		first.character_positions().map(<[f32]>::len),
		Some(characters)
	);
	assert!(
		first
			.character_positions()
			.expect("positions")
			.windows(2)
			.all(|pair| pair[0] <= pair[1]),
		"positions grow left to right"
	);
	assert_eq!(first.word_starts().first(), Some(&0));
}

#[test]
fn the_selection_is_reported_in_run_positions() {
	let mut driver = Driver::new("Hello world");
	let _ = driver.key("a", support::CTRL);
	let nodes = tree(&mut driver);
	let run = runs(&nodes)[0].0;
	let selection = node(&nodes, EDITOR_ID)
		.text_selection()
		.copied()
		.expect("a selection");

	assert_eq!(selection.anchor.node, run);
	assert_eq!(selection.anchor.character_index, 0);
	assert_eq!(
		selection.focus.character_index, 11,
		"on the final line break"
	);
}

#[test]
fn screen_readers_can_select_and_replace_text() {
	let mut driver = Driver::new("Hello world");
	let _ = driver.send(&InputEvent::Focus(false));
	let nodes = tree(&mut driver);
	let run = runs(&nodes)[0].0;
	let at = |character_index| {
		TextPosition {
			node: run,
			character_index,
		}
	};

	let response = driver
		.editor
		.accessibility_action(&request(
			Action::SetTextSelection,
			Some(ActionData::SetTextSelection(TextSelection {
				anchor: at(6),
				focus: at(11),
			})),
		))
		.expect("select");
	assert!(response.repaint);
	assert_eq!(driver.selected_text(), "world");

	let _ = driver
		.editor
		.accessibility_action(&request(
			Action::ReplaceSelectedText,
			Some(ActionData::Value("there".into())),
		))
		.expect("replace");
	assert_eq!(driver.markdown(), "Hello there");

	let _ = driver
		.editor
		.accessibility_action(&request(Action::Focus, None))
		.expect("focus");
	assert!(driver.editor.is_focused());
}

#[test]
fn long_documents_describe_only_what_is_on_screen() {
	let markdown: Vec<String> = (0..2_000).map(|index| format!("Line {index}")).collect();
	let mut driver = Driver::new(&markdown.join("\n\n"));
	let nodes = tree(&mut driver);
	let blocks = node(&nodes, EDITOR_ID).children().len();

	assert!((5..60).contains(&blocks), "{blocks} blocks described");

	// The caret's block is always described, even off screen.
	let _ = driver.press("End", support::CTRL);
	driver.editor.scroll_to(0.0);
	driver.frame();
	let nodes = tree(&mut driver);
	let values: Vec<String> = runs(&nodes).into_iter().map(|(_, value)| value).collect();
	assert!(
		values.iter().any(|value| value == "Line 1999\n"),
		"{values:?}"
	);
	assert!(node(&nodes, EDITOR_ID).text_selection().is_some());
}

#[test]
fn bounds_follow_the_window_transform() {
	let mut driver = Driver::sized("Hello", 300.0, 200.0);
	let to_window = Affine::scale(2.0) * Affine::translate((0.0, 40.0));
	let update = driver.editor.accessibility_tree("Notes", to_window);
	let bounds = node(&update.nodes, EDITOR_ID).bounds().expect("bounds");

	assert_eq!(
		(bounds.x0, bounds.y0, bounds.x1, bounds.y1),
		(0.0, 80.0, 600.0, 480.0)
	);
}
