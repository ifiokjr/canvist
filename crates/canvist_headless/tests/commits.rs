//! `Doc::record_commits`: a host that lays the document out sees every
//! change, including the ones collaboration makes on its own.

#![cfg(all(feature = "starter-kit", feature = "journal"))]

use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;

#[test]
fn commits_are_not_kept_until_tracking_starts() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, "Hello")?;

	doc.select_range(&Position::new([0], 5), &Position::new([0], 5))?;
	doc.insert_text("!")?;

	assert!(doc.take_commits().is_empty());
	Ok(())
}

#[test]
fn local_changes_are_kept_in_order_and_drained_once() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, "Hello")?;
	doc.record_commits();

	doc.select_range(&Position::new([0], 5), &Position::new([0], 5))?;
	doc.insert_text(" world")?;
	doc.press("Enter")?;

	let commits = doc.take_commits();
	assert_eq!(commits.len(), 2, "the selection change is not a commit");
	assert!(!commits[0].structural, "typing changes one block");
	assert!(commits[1].structural, "Enter splits the block");
	assert!(commits[0].version < commits[1].version);
	assert!(doc.take_commits().is_empty());
	Ok(())
}

#[test]
fn undo_is_a_commit_too() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();
	let mut doc = engine.parse(Format::Markdown, "Hello")?;
	doc.select_all()?;
	doc.insert_text("Bye")?;
	doc.record_commits();

	assert!(doc.can_undo());
	assert!(doc.undo()?);

	assert_eq!(doc.take_commits().len(), 1);
	assert_eq!(doc.export(Format::Markdown), "Hello");
	assert!(doc.can_redo());
	Ok(())
}

#[cfg(feature = "collab")]
mod collab {
	use super::*;

	#[test]
	fn remote_updates_are_kept() -> Result<(), HeadlessError> {
		let engine = Engine::starter_kit();
		let mut first = engine.parse(Format::Markdown, "Hello")?;
		first.share(1)?;
		let mut second = engine.from_yjs(&first.encode_state()?, 2)?;
		second.record_commits();

		first.select_range(&Position::new([0], 5), &Position::new([0], 5))?;
		first.insert_text("!")?;

		for update in first.take_updates()? {
			second.apply_update(&update)?;
		}

		let commits = second.take_commits();
		assert_eq!(commits.len(), 1);
		assert_eq!(second.export(Format::Markdown), "Hello!");
		Ok(())
	}

	#[test]
	fn shared_undo_is_kept() -> Result<(), HeadlessError> {
		let engine = Engine::starter_kit();
		let mut doc = engine.parse(Format::Markdown, "Hello")?;
		doc.share(1)?;
		doc.select_range(&Position::new([0], 5), &Position::new([0], 5))?;
		doc.insert_text("!")?;
		doc.record_commits();

		assert!(doc.can_undo());
		assert!(doc.undo()?);

		assert_eq!(doc.take_commits().len(), 1);
		assert_eq!(doc.export(Format::Markdown), "Hello");
		assert!(!doc.can_undo());
		assert!(doc.can_redo());
		Ok(())
	}

	#[test]
	fn the_paragraph_a_peer_emptying_the_document_leaves_is_kept() -> Result<(), HeadlessError> {
		let engine = Engine::starter_kit();
		let mut first = engine.parse(Format::Markdown, "Hello\n\n---")?;
		first.share(1)?;
		let mut second = engine.from_yjs(&first.encode_state()?, 2)?;
		second.record_commits();

		first.select_all()?;
		first.delete_selection()?;

		for update in first.take_updates()? {
			second.apply_update(&update)?;
		}

		let commits = second.take_commits();
		assert!(!commits.is_empty());
		assert!(
			commits.iter().any(|commit| commit.structural),
			"the blocks were removed or replaced"
		);
		assert_eq!(second.document().children(None).len(), 1);
		Ok(())
	}
}
