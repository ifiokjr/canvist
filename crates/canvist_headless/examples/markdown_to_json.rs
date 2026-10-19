//! Convert Markdown on standard input to ProseMirror JSON on standard
//! output, as a build step or an import endpoint would.
//!
//! ```sh
//! echo '# Hello **world**' | cargo run -p canvist_headless --example markdown_to_json
//! ```

use std::io::Read;

use canvist_headless::Engine;
use canvist_headless::Format;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut markdown = String::new();
	std::io::stdin().read_to_string(&mut markdown)?;

	let doc = Engine::starter_kit().parse(Format::Markdown, &markdown)?;
	println!("{}", serde_json::to_string_pretty(&doc.to_json())?);
	Ok(())
}
