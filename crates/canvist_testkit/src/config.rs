//! What an editor under test is made of.

use canvist_headless::canvist_core::Platform;

/// The extensions the StarterKit registers, by name: the default set every
/// target can build.
pub const STARTER_KIT: [&str; 17] = [
	"heading",
	"blockquote",
	"codeBlock",
	"hardBreak",
	"horizontalRule",
	"bulletList",
	"orderedList",
	"listItem",
	"taskList",
	"bold",
	"italic",
	"underline",
	"strike",
	"code",
	"link",
	"history",
	"trailingNode",
];

/// How a test builds its editor.
///
/// Extensions register by name on every target, the way the JavaScript,
/// Dart, Swift, and Kotlin bindings register them, so a test's editor is
/// the same editor everywhere. Rust-only extensions with typed handles are
/// for product code; tests reach commands by name.
#[derive(Clone, Debug)]
pub struct EditorConfig {
	/// The extensions to register, such as `["bold", "heading", "history"]`.
	/// The default is [`STARTER_KIT`].
	pub extensions: Vec<String>,
	/// The id this editor allocates blocks as. Collaborating editors must
	/// not share one.
	pub client: u32,
	/// Which modifier key bindings call `Mod`: Command on Apple platforms,
	/// Control elsewhere. The default matches the target: Apple on iOS,
	/// whatever the page reports on the web, Control in the headless
	/// engine.
	pub platform: Option<Platform>,
}

impl Default for EditorConfig {
	fn default() -> Self {
		Self {
			extensions: STARTER_KIT.iter().map(|name| (*name).to_owned()).collect(),
			client: 1,
			platform: None,
		}
	}
}

impl EditorConfig {
	/// An editor with the default StarterKit extensions.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// An editor with only the extensions named, plus nothing else: for
	/// tests that check one extension on its own or a pair together.
	#[must_use]
	pub fn with_extensions(names: &[&str]) -> Self {
		Self {
			extensions: names.iter().map(|name| (*name).to_owned()).collect(),
			client: 1,
			platform: None,
		}
	}

	/// Allocate blocks as `client`.
	#[must_use]
	pub fn client(mut self, client: u32) -> Self {
		self.client = client;
		self
	}

	/// Read key bindings for `platform`.
	#[must_use]
	pub fn platform(mut self, platform: Platform) -> Self {
		self.platform = Some(platform);
		self
	}

	/// The config with the extensions named added to the set.
	#[must_use]
	pub fn plus(mut self, names: &[&str]) -> Self {
		for name in names {
			let name = (*name).to_owned();
			if !self.extensions.contains(&name) {
				self.extensions.push(name);
			}
		}

		self
	}
}
