//! Media blocks: images, YouTube and Twitch videos, audio, and file
//! attachments.
//!
//! Each is an atomic block whose attributes say what to show. The canvas
//! reserves a slot for it, and the host draws the picture, player, or file
//! card there, as a node view does in TipTap. Every media type is described
//! by a [`MediaDef`], so one implementation serves them all.

#![cfg_attr(
	not(all(
		feature = "audio",
		feature = "file",
		feature = "image",
		feature = "twitch",
		feature = "youtube"
	)),
	allow(dead_code, reason = "each media type uses a different subset")
)]

use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

/// The value a media attribute starts with.
#[derive(Clone, Copy, Debug)]
enum Initial {
	Null,
	Bool(bool),
	/// Small enough to keep the enum compact: pixel sizes and seconds.
	Int(u16),
}

/// What a media attribute holds.
#[derive(Clone, Copy, Debug)]
enum Kind {
	Text,
	Number,
	Flag,
}

/// One attribute of a media block.
#[derive(Clone, Copy, Debug)]
struct MediaAttr {
	name: &'static str,
	kind: Kind,
	default: Initial,
	doc: &'static str,
}

const fn text(name: &'static str, doc: &'static str) -> MediaAttr {
	MediaAttr {
		name,
		kind: Kind::Text,
		default: Initial::Null,
		doc,
	}
}

const fn number(name: &'static str, default: Initial, doc: &'static str) -> MediaAttr {
	MediaAttr {
		name,
		kind: Kind::Number,
		default,
		doc,
	}
}

const fn flag(name: &'static str, default: bool, doc: &'static str) -> MediaAttr {
	MediaAttr {
		name,
		kind: Kind::Flag,
		default: Initial::Bool(default),
		doc,
	}
}

impl MediaAttr {
	fn default_value(self) -> Value {
		match self.default {
			Initial::Null => Value::Null,
			Initial::Bool(flag) => Value::Bool(flag),
			Initial::Int(number) => Value::Int(i64::from(number)),
		}
	}

	fn value_type(self) -> ValueType {
		let ty = match self.kind {
			Kind::Text => ValueType::String,
			Kind::Number => ValueType::Float,
			Kind::Flag => ValueType::Bool,
		};

		match self.default {
			Initial::Null => ValueType::nullable(ty),
			_ => ty,
		}
	}

	/// The attribute as a node attribute, or as a command argument: the
	/// first attribute, the source, is required there.
	fn field(self, required: bool) -> Field {
		if required {
			Field::required(self.name, self.value_type(), self.doc)
		} else {
			Field::optional(self.name, self.value_type(), self.default_value(), self.doc)
		}
	}
}

/// Everything that distinguishes one media block from another.
#[derive(Debug)]
struct MediaDef {
	/// The block and extension name, as in TipTap.
	name: &'static str,
	/// The command that inserts one.
	command: &'static str,
	/// What the command does.
	doc: &'static str,
	/// The attributes; the first is the source, which the command needs.
	attrs: &'static [MediaAttr],
}

/// Typed access to a media extension such as [`Image`].
#[derive(Clone, Copy, Debug)]
pub struct MediaHandle {
	def: &'static MediaDef,
	_registration: Registration,
}

impl MediaHandle {
	/// Insert a block showing `src`, with these other attributes, at the
	/// selection, as a horizontal rule is. Unknown attributes are dropped.
	#[must_use]
	pub fn insert(self, src: &str, attrs: &Attrs) -> Command {
		self.def.insert(src, attrs)
	}
}

impl MediaDef {
	/// The command inserting `src` with `attrs`. Attributes the type lacks
	/// are dropped, and so are ones equal to their default, since an absent
	/// attribute is its default.
	fn insert(&'static self, src: &str, attrs: &Attrs) -> Command {
		let source = self.attrs[0].name;
		let mut attrs: Attrs = attrs
			.iter()
			.filter(|(key, value)| {
				self.attrs
					.iter()
					.any(|attr| attr.name == *key && attr.default_value() != **value)
			})
			.map(|(key, value)| (key, value.clone()))
			.collect();
		attrs.set(source, src);

		command(move |state| {
			let kind = resolve::block(state, self.name, self.name)?;
			commands::insert_leaf(state, kind, attrs.clone())
		})
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = self
			.attrs
			.iter()
			.map(|attr| (attr.name, attr.default_value()))
			.collect();
		schema.block(BlockSpec::leaf(self.name).attrs(attrs))
	}

	fn node(&self) -> Vec<NodeMeta> {
		let node = NodeMeta::new(NodeKind::Block, self.name);
		vec![
			self.attrs
				.iter()
				.fold(node, |node, attr| node.attr(attr.field(false))),
		]
	}

	fn commands(&'static self) -> Vec<CommandSpec> {
		let spec = CommandSpec::new(self.command, self.doc, move |args| {
			let source = args.string(self.attrs[0].name)?;
			let attrs: Attrs = self.attrs[1..]
				.iter()
				.map(|attr| Ok((attr.name, args.get(attr.name)?.clone())))
				.collect::<Result<_, EditorError>>()?;
			Ok(self.insert(source, &attrs))
		});

		let spec = spec.arg(self.attrs[0].field(true));
		vec![
			self.attrs[1..]
				.iter()
				.fold(spec, |spec, attr| spec.arg(attr.field(false))),
		]
	}

	fn handle(&'static self, registration: Registration) -> MediaHandle {
		MediaHandle {
			def: self,
			_registration: registration,
		}
	}
}

#[cfg(any(test, feature = "image"))]
static IMAGE: MediaDef = MediaDef {
	name: "image",
	command: "setImage",
	doc: "Insert an image at the selection.",
	attrs: &[
		text("src", "The image URL."),
		text("alt", "Text describing the image, for screen readers."),
		text("title", "A title shown on hover."),
		number("width", Initial::Null, "The width in pixels."),
		number("height", Initial::Null, "The height in pixels."),
	],
};

#[cfg(any(test, feature = "youtube"))]
static YOUTUBE: MediaDef = MediaDef {
	name: "youtube",
	command: "setYoutubeVideo",
	doc: "Insert a YouTube video at the selection.",
	attrs: &[
		text("src", "The video URL."),
		number(
			"start",
			Initial::Int(0),
			"Where playback starts, in seconds.",
		),
		number("width", Initial::Int(640), "The width in pixels."),
		number("height", Initial::Int(480), "The height in pixels."),
	],
};

#[cfg(any(test, feature = "twitch"))]
static TWITCH: MediaDef = MediaDef {
	name: "twitch",
	command: "setTwitchVideo",
	doc: "Insert a Twitch video, clip, or channel at the selection.",
	attrs: &[
		text("src", "The video, clip, or channel URL."),
		number("width", Initial::Int(640), "The width in pixels."),
		number("height", Initial::Int(480), "The height in pixels."),
		flag("autoplay", false, "Start playing straight away."),
		flag("muted", false, "Start muted."),
	],
};

#[cfg(any(test, feature = "audio"))]
static AUDIO: MediaDef = MediaDef {
	name: "audio",
	command: "setAudio",
	doc: "Insert an audio player at the selection.",
	attrs: &[
		text("src", "The audio URL."),
		flag("controls", true, "Show playback controls."),
		flag("autoplay", false, "Start playing straight away."),
		flag("loop", false, "Play again from the start when it ends."),
		flag("muted", false, "Start muted."),
	],
};

#[cfg(any(test, feature = "file"))]
static FILE: MediaDef = MediaDef {
	name: "file",
	command: "insertFile",
	doc: "Insert a file attachment at the selection.",
	attrs: &[
		text("url", "Where the file can be downloaded."),
		text("fileName", "The file's name, such as `report.pdf`."),
		text(
			"fileType",
			"The file's media type, such as `application/pdf`.",
		),
		number("fileSize", Initial::Int(0), "The file's size in bytes."),
	],
};

/// `![alt](src "title")`, as typed: group 2 is the alt text, 3 the source,
/// and 4 the title. The title's quotes may already be curly, when
/// typography is on.
#[cfg(feature = "image")]
const IMAGE_SYNTAX: &str = r#"(?:^|\s)(!\[([^\]]*)\]\((\S+)(?:\s+["'“‘]([^"'“”‘’]+)["'”’])?\))$"#;

/// Replace typed `![alt](src "title")` with an image block.
#[cfg(feature = "image")]
fn typed_image(
	state: &mut canvist_editor::EditorState,
	found: &canvist_editor::RuleMatch,
	kind: canvist_model::BlockTypeId,
) -> canvist_editor::CommandResult {
	let (Some(Some(range)), Some(src)) = (found.groups.get(1).cloned(), found.group(3)) else {
		return Ok(None);
	};
	let given = [("alt", found.group(2)), ("title", found.group(4))];
	let attrs: Attrs = given
		.into_iter()
		.filter_map(|(key, text)| Some((key, text.filter(|text| !text.is_empty())?)))
		.chain([("src", src)])
		.collect();

	state.atomic(|state| {
		let removed = state.change(canvist_editor::EditKind::InputRule, |tr| {
			tr.delete_text(found.block, range.start..found.caret)?;
			Ok(true)
		})?;
		let inserted = commands::insert_leaf(state, kind, attrs)?;

		Ok(match (removed, inserted) {
			(Some(removed), Some(inserted)) => Some(removed.then(inserted)),
			_ => None,
		})
	})
}

/// Declares a media extension: a unit struct whose [`Extension`] and
/// [`TypedExtension`] delegate to its [`MediaDef`].
macro_rules! media {
	($(#[$meta:meta])* $type:ident, $def:expr) => {
		$(#[$meta])*
		#[derive(Clone, Copy, Debug, Default)]
		pub struct $type;

		impl Extension for $type {
			fn name(&self) -> &'static str {
				$def.name
			}

			fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
				$def.schema(schema)
			}

			fn nodes(&self) -> Vec<NodeMeta> {
				$def.node()
			}

			fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
				Ok($def.commands())
			}

			fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
				media_rules(schema, &$def)
			}
		}

		impl TypedExtension for $type {
			type Handle = MediaHandle;

			fn handle(&self, registration: Registration) -> MediaHandle {
				$def.handle(registration)
			}
		}
	};
}

/// Images turn typed Markdown into a block; other media have no rule.
#[cfg_attr(
	not(feature = "image"),
	allow(
		clippy::unnecessary_wraps,
		reason = "images' rule can fail to build; without them nothing does"
	)
)]
fn media_rules(schema: &Schema, def: &'static MediaDef) -> Result<Vec<InputRule>, EditorError> {
	#[cfg(feature = "image")]
	if def.name == IMAGE.name {
		let kind = canvist_editor::block_type(schema, IMAGE.name, IMAGE.name)?;
		let rule = InputRule::new(IMAGE_SYNTAX, move |state, found| {
			typed_image(state, found, kind)
		})?;
		return Ok(vec![rule.on(")")]);
	}

	let _ = (schema, def);
	Ok(Vec::new())
}

#[cfg(feature = "image")]
media!(
	/// TipTap's `image` block, with `src`, `alt`, `title`, `width`, and
	/// `height`: `setImage`, or `![alt](src "title")` typed.
	Image,
	IMAGE
);

#[cfg(feature = "youtube")]
media!(
	/// TipTap's `youtube` block, with `src`, `start`, `width`, and
	/// `height`: `setYoutubeVideo`.
	Youtube,
	YOUTUBE
);

#[cfg(feature = "twitch")]
media!(
	/// TipTap's `twitch` block, with `src`, `width`, `height`, `autoplay`,
	/// and `muted`: `setTwitchVideo`.
	Twitch,
	TWITCH
);

#[cfg(feature = "audio")]
media!(
	/// TipTap's `audio` block, with `src`, `controls`, `autoplay`, `loop`,
	/// and `muted`: `setAudio`.
	Audio,
	AUDIO
);

#[cfg(feature = "file")]
media!(
	/// Remirror's `file` attachment card, with `url`, `fileName`,
	/// `fileType`, and `fileSize`: `insertFile`.
	File,
	FILE
);
