//! Collaborative-editing tests: two editors, one document.
//!
//! The sync loop here is what a websocket does: each editor hands its
//! updates to the other. Tests with `targets = [headless, loopback]` run
//! the same script on the in-process engine and through the daemon's
//! agent protocol, so the wire round-trips the bytes Yjs produced.
//!
//! Runs with `--features collab`; without it the file builds to nothing.

#![cfg(feature = "collab")]

use canvist_testkit::EditorConfig;
use canvist_testkit::EditorHarness;
use canvist_testkit::Target;
use canvist_testkit::cross_test;

cross_test! {
	/// What one editor types, the other sees after one sync.
	typing_reaches_the_peer, targets = [headless, loopback], async fn sync_typing(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		a.load_text("hello").await?;
		a.move_to_document_end().await?;
		a.share(1).await?;

		let mut b = EditorHarness::join(a.target(), EditorConfig::default().client(2), &a.encode_state().await?).await?;
		b.move_to_document_end().await?;

		a.type_text(" world").await?;
		a.sync_to(&mut b).await?;

		a.assert_synced_with(&mut b).await?;
		b.assert_text("hello world").await?;
		b.close().await
	}
}

cross_test! {
	/// Two peers typing in the same document converge, in both directions.
	both_peers_converge, targets = [headless, loopback], async fn converge(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		a.load_text("a1 b1").await?;
		a.move_to_document_end().await?;
		a.share(1).await?;

		let mut b = EditorHarness::join(a.target(), EditorConfig::default().client(2), &a.encode_state().await?).await?;
		b.move_to_document_end().await?;

		a.type_text(" more-a").await?;
		b.type_text(" more-b").await?;
		a.sync_to(&mut b).await?;
		b.sync_to(&mut *a).await?;

		a.assert_synced_with(&mut b).await?;
		Ok(())
	}
}

cross_test! {
	/// Undo reverts this peer's work, never the collaborator's.
	undo_never_touches_the_peer, targets = [headless, loopback], async fn own_undo(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		a.load_text("").await?;
		a.share(1).await?;

		let mut b = EditorHarness::join(a.target(), EditorConfig::default().client(2), &a.encode_state().await?).await?;

		b.type_text("from b").await?;
		b.sync_to(&mut *a).await?;

		a.move_to_document_end().await?;
		a.type_text(" from a").await?;
		a.sync_to(&mut b).await?;

		// Undo reverts only a's own " from a"; b still holds it until
		// the undo's inverse syncs back, and then both agree.
		a.undo().await?;
		a.assert_text("from b").await?;

		a.sync_to(&mut b).await?;
		b.assert_text("from b").await?;
		b.close().await
	}
}

cross_test! {
	/// Extensions and collaboration together: a heading typed through the
	/// input rules syncs as a heading, not as characters.
	/// Also crosses targets: a headless peer and a loopback peer share one
	/// document through the same bytes.
	structured_typing_syncs_across_targets, targets = [headless], async fn cross_target(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		a.load_text("").await?;
		a.share(1).await?;

		let mut b = EditorHarness::join(Target::Loopback, EditorConfig::default().client(2), &a.encode_state().await?).await?;

		a.type_text("## shared heading").await?;
		a.sync_to(&mut b).await?;

		b.assert_markdown("## shared heading").await?;
		b.close().await
	}
}

cross_test! {
	/// A peer that rejoins from the shared state sees the whole document,
	/// including everything it missed while away.
	rejoin_from_state_sees_missed_work, targets = [headless, loopback], async fn rejoin(a: &mut EditorHarness) -> canvist_testkit::TestResult<()> {
		a.load_text("base").await?;
		a.move_to_document_end().await?;
		a.share(1).await?;

		let mut b = EditorHarness::join(a.target(), EditorConfig::default().client(2), &a.encode_state().await?).await?;

		a.type_text(" plus").await?;
		a.sync_to(&mut b).await?;

		// A third editor joins from the state as a whole, later.
		let mut c = EditorHarness::join(a.target(), EditorConfig::default().client(3), &a.encode_state().await?).await?;
		c.assert_text("base plus").await?;

		b.close().await?;
		c.close().await
	}
}
