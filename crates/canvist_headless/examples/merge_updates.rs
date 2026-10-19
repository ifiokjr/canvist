//! A server merging two peers' Yjs updates into its copy of a document,
//! checking the result against its schema, and storing it.
//!
//! ```sh
//! cargo run -p canvist_headless --features collab --example merge_updates
//! ```

use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;

fn main() -> Result<(), HeadlessError> {
	let engine = Engine::starter_kit();

	// The server owns the document and shares it first.
	let mut server = engine.parse(Format::Markdown, "# Minutes\n\nAttendees:")?;
	server.share(1)?;
	let state = server.encode_state()?;

	// Two editors join from the stored state and edit concurrently.
	let mut alice = engine.clone().with_client(2).from_yjs(&state, 2)?;
	let mut bob = engine.clone().with_client(3).from_yjs(&state, 3)?;
	let end = Position::new([1], 10);
	alice.select_range(&end, &end)?;
	alice.insert_text(" Alice")?;
	bob.select_range(&end, &end)?;
	bob.insert_text(" Bob")?;

	// Updates may arrive in any order; the server merges them.
	for update in bob.take_updates()?.into_iter().chain(alice.take_updates()?) {
		server.apply_update(&update)?;
	}

	let validation = server.validate()?;
	println!(
		"merged: {}",
		server.export(Format::Text).replace('\n', " / ")
	);
	println!(
		"valid: {} ({} rejected blocks)",
		validation.is_valid(),
		validation.rejected_blocks
	);
	println!("stored state: {} bytes", server.encode_state()?.len());
	Ok(())
}
