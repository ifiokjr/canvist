//! The editor: state, history, and the behaviour extensions contribute.

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::TreeContent;
use rustc_hash::FxHashMap;

use crate::Command;
use crate::CommandResult;
use crate::CommandSpec;
use crate::Context;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::Extension;
use crate::ExtensionMeta;
use crate::History;
use crate::InputRule;
use crate::KeyPress;
use crate::Keymap;
use crate::LineNavigator;
use crate::NodeKind;
use crate::NodeMeta;
use crate::Origin;
use crate::ParagraphNavigator;
use crate::PasteRule;
use crate::Platform;
use crate::QuerySpec;
use crate::RuleInput;
use crate::Slice;
use crate::UndoHistory;
use crate::Update;
use crate::UpdateFilter;
use crate::commands;
use crate::context::AppliedRule;
use crate::position;

/// What the host knows about the moment an input event happened.
pub struct Env<'a> {
	/// When the event happened, on any monotonic clock. History groups
	/// edits by it.
	pub now: Duration,
	lines: Option<&'a mut dyn LineNavigator>,
	fallback: ParagraphNavigator,
}

impl<'a> Env<'a> {
	/// An environment whose visual lines come from layout.
	#[must_use]
	pub fn new(now: Duration, lines: &'a mut dyn LineNavigator) -> Self {
		Self {
			now,
			lines: Some(lines),
			fallback: ParagraphNavigator,
		}
	}

	/// The navigator for visual lines.
	pub fn lines(&mut self) -> &mut dyn LineNavigator {
		match &mut self.lines {
			Some(lines) => &mut **lines,
			None => &mut self.fallback,
		}
	}
}

impl Env<'static> {
	/// An environment without layout, treating each block as one line.
	#[must_use]
	pub const fn headless(now: Duration) -> Self {
		Self {
			now,
			lines: None,
			fallback: ParagraphNavigator,
		}
	}
}

impl std::fmt::Debug for Env<'_> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Env")
			.field("now", &self.now)
			.finish_non_exhaustive()
	}
}

/// Everything the extensions contributed, resolved against the schema.
#[derive(Default)]
pub(crate) struct Registry {
	extensions: Vec<Arc<dyn Extension>>,
	keymap: Keymap,
	input_rules: Vec<InputRule>,
	paste_rules: Vec<PasteRule>,
	/// Every extension's update filters, in registration order.
	update_filters: Vec<UpdateFilter>,
	pub(crate) commands: FxHashMap<&'static str, CommandSpec>,
	pub(crate) queries: FxHashMap<&'static str, QuerySpec>,
	/// The command and query names each extension registered, in
	/// registration order.
	names: Vec<RegisteredNames>,
	/// What the extensions add, built on first use, so hosts that never
	/// read it, such as the browser editor, neither build nor compile it.
	metadata: OnceLock<Vec<ExtensionMeta>>,
	code_blocks: Vec<BlockTypeId>,
}

/// The names one extension registered.
struct RegisteredNames {
	commands: Vec<&'static str>,
	queries: Vec<&'static str>,
}

impl Registry {
	fn new(
		extensions: Vec<Arc<dyn Extension>>,
		schema: &Schema,
		platform: Platform,
	) -> Result<Self, EditorError> {
		let mut registry = Self::default();

		for extension in &extensions {
			for (binding, command) in extension.keymap(schema, platform)? {
				registry.keymap.bind(binding, platform, command)?;
			}

			registry.describe(extension.as_ref(), schema)?;
			registry.input_rules.extend(extension.input_rules(schema)?);
			registry.paste_rules.extend(extension.paste_rules(schema)?);
			registry
				.update_filters
				.extend(extension.update_filters(schema)?);
			registry.code_blocks.extend(extension.code_blocks(schema)?);
		}

		registry.extensions = extensions;
		Ok(registry)
	}

	/// What every extension adds, in registration order: the nodes the
	/// schema has, and the commands and queries it registered.
	pub(crate) fn metadata(&self, schema: &Schema) -> &[ExtensionMeta] {
		self.metadata.get_or_init(|| {
			self.extensions
				.iter()
				.zip(&self.names)
				.map(|(extension, names)| {
					ExtensionMeta {
						name: extension.name(),
						nodes: extension
							.nodes()
							.into_iter()
							.filter(|node| has_node(schema, node))
							.collect(),
						commands: names
							.commands
							.iter()
							.filter_map(|name| self.commands.get(name))
							.map(|spec| spec.meta().clone())
							.collect(),
						queries: names
							.queries
							.iter()
							.filter_map(|name| self.queries.get(name))
							.map(|spec| spec.meta().clone())
							.collect(),
					}
				})
				.collect()
		})
	}

	/// Whether every update filter lets `update` stand.
	pub(crate) fn allows(&self, state: &EditorState, update: &Update) -> bool {
		self.update_filters
			.iter()
			.all(|allows| allows(state, update))
	}

	/// Register an extension's commands and queries by name, refusing
	/// names another extension already took.
	fn describe(&mut self, extension: &dyn Extension, schema: &Schema) -> Result<(), EditorError> {
		let commands = extension.commands(schema)?;
		let queries = extension.queries(schema)?;
		let taken = |name: &str| EditorError::DuplicateName(name.to_owned());

		self.names.push(RegisteredNames {
			commands: commands.iter().map(|spec| spec.meta().name).collect(),
			queries: queries.iter().map(|spec| spec.meta().name).collect(),
		});

		for spec in commands {
			let name = spec.meta().name;

			if self.commands.insert(name, spec).is_some() {
				return Err(taken(name));
			}
		}

		for spec in queries {
			let name = spec.meta().name;

			if self.queries.insert(name, spec).is_some() {
				return Err(taken(name));
			}
		}

		Ok(())
	}
}

/// Configures and builds an [`Editor`].
#[derive(Default)]
pub struct EditorBuilder {
	pub(crate) extensions: Vec<Arc<dyn Extension>>,
	platform: Platform,
	client: u32,
	history: Option<Box<dyn History>>,
}

impl EditorBuilder {
	/// Add one extension.
	#[must_use]
	pub fn extension(mut self, extension: impl Extension + 'static) -> Self {
		self.extensions.push(Arc::new(extension));
		self
	}

	/// Add several extensions, such as a kit.
	#[must_use]
	pub fn extensions(mut self, extensions: impl IntoIterator<Item = Arc<dyn Extension>>) -> Self {
		self.extensions.extend(extensions);
		self
	}

	/// Set the platform, which decides what `Mod` means in key bindings.
	#[must_use]
	pub fn platform(mut self, platform: Platform) -> Self {
		self.platform = platform;
		self
	}

	/// Set the collaboration client id that seeds block ids.
	#[must_use]
	pub fn client(mut self, client: u32) -> Self {
		self.client = client;
		self
	}

	/// Replace the default [`UndoHistory`], e.g. with a collaborative one.
	#[must_use]
	pub fn history(mut self, history: impl History + 'static) -> Self {
		self.history = Some(Box::new(history));
		self
	}

	/// The extensions sorted by priority, highest first.
	fn sorted(&self) -> Vec<Arc<dyn Extension>> {
		let mut extensions = self.extensions.clone();
		extensions.sort_by_key(|extension| std::cmp::Reverse(extension.priority()));
		extensions
	}

	/// Build the schema the extensions describe.
	pub fn schema(&self) -> Result<Arc<Schema>, EditorError> {
		let builder = self
			.sorted()
			.iter()
			.fold(Schema::builder(), |builder, extension| {
				extension.schema(builder)
			});
		Ok(Arc::new(builder.build()?))
	}

	/// Build an editor with an empty document.
	pub fn build(self) -> Result<Editor, EditorError> {
		self.build_with(|schema, client| Ok(Document::new(schema, client)))
	}

	/// Build an editor whose document `content` creates from the schema and
	/// client id, e.g. with [`Document::from_json`].
	pub fn build_with(
		self,
		content: impl FnOnce(Arc<Schema>, u32) -> Result<Document, EditorError>,
	) -> Result<Editor, EditorError> {
		let schema = self.schema()?;
		let registry = Registry::new(self.sorted(), &schema, self.platform)?;
		let state = EditorState::new(content(schema, self.client)?)?;
		let history = self
			.history
			.unwrap_or_else(|| Box::new(UndoHistory::default()));

		Ok(Editor {
			state,
			history,
			registry,
			platform: self.platform,
			last_rule: None,
		})
	}
}

/// A complete editing engine: state, history, keymaps, rules, and the
/// extensions that provided them.
///
/// Hosts translate native events into calls such as [`Editor::handle_key`]
/// and [`Editor::insert_text`], then use the returned [`Update`] to relayout
/// and repaint. Every update passes through one place, which runs the
/// extensions' fix-ups and records history.
pub struct Editor {
	pub(crate) state: EditorState,
	pub(crate) history: Box<dyn History>,
	pub(crate) registry: Registry,
	platform: Platform,
	pub(crate) last_rule: Option<AppliedRule>,
}

impl std::fmt::Debug for Editor {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Editor")
			.field("state", &self.state)
			.field("platform", &self.platform)
			.finish_non_exhaustive()
	}
}

impl Editor {
	/// Start configuring an editor.
	#[must_use]
	pub fn builder() -> EditorBuilder {
		EditorBuilder::default()
	}

	/// The editor state.
	#[must_use]
	pub fn state(&self) -> &EditorState {
		&self.state
	}

	/// Take the editor apart, returning its state. Hosts that lend a
	/// document to an editor (a canvas text frame) take it back this way.
	#[must_use]
	pub fn into_state(self) -> EditorState {
		self.state
	}

	/// The schema.
	#[must_use]
	pub fn schema(&self) -> &Arc<Schema> {
		self.state.schema()
	}

	/// The platform the key bindings were built for.
	#[must_use]
	pub fn platform(&self) -> Platform {
		self.platform
	}

	/// Whether there is something to undo.
	#[must_use]
	pub fn can_undo(&self) -> bool {
		self.history.can_undo()
	}

	/// Whether there is something to redo.
	#[must_use]
	pub fn can_redo(&self) -> bool {
		self.history.can_redo()
	}

	/// Run the commands bound to `press` until one applies.
	pub fn handle_key(&mut self, press: KeyPress<'_>, env: &mut Env<'_>) -> CommandResult {
		let mut context = Context {
			state: &mut self.state,
			history: self.history.as_mut(),
			lines: env.lines(),
			last_rule: self.last_rule.as_ref(),
		};
		let update = first_applying(self.registry.keymap.lookup(press), &mut context)?;
		self.finish(update, env.now)
	}

	/// Run `command`.
	pub fn run(&mut self, command: &Command, env: &mut Env<'_>) -> CommandResult {
		let mut context = Context {
			state: &mut self.state,
			history: self.history.as_mut(),
			lines: env.lines(),
			last_rule: self.last_rule.as_ref(),
		};
		let update = command(&mut context)?;
		self.finish(update, env.now)
	}

	/// Type `text`, giving input rules the first chance to rewrite it.
	pub fn insert_text(&mut self, text: &str, env: &mut Env<'_>) -> CommandResult {
		if let Some(update) = self.apply_input_rules(text)? {
			let finished = self.finish(Some(update), env.now)?;
			self.last_rule = finished.as_ref().map(|update| {
				AppliedRule {
					version: self.state.doc().version(),
					selection: self.state.selection(),
					typed: text.to_owned(),
					undo_steps: update.commit.undo_steps(),
					selection_before: update.selection_before,
				}
			});

			return Ok(finished);
		}

		let update = commands::insert_text(&mut self.state, text)?;
		self.finish(update, env.now)
	}

	fn apply_input_rules(&mut self, text: &str) -> CommandResult {
		let Some(caret) = self.state.selection().as_caret() else {
			return Ok(None);
		};

		if self.state.composition().is_some() || self.in_code(caret.block) {
			return Ok(None);
		}

		let Some(input) = RuleInput::new(&self.state, caret, text) else {
			return Ok(None);
		};

		for rule in &self.registry.input_rules {
			let Some(found) = rule.find(&input) else {
				continue;
			};

			if let Some(mut update) = rule.apply(&mut self.state, &found)? {
				update.kind = EditKind::InputRule;

				return Ok(Some(update));
			}
		}

		Ok(None)
	}

	fn in_code(&self, block: canvist_model::BlockId) -> bool {
		let kind = self
			.state
			.doc()
			.block(block)
			.map(canvist_model::Block::kind);
		kind.is_some_and(|kind| self.registry.code_blocks.contains(&kind))
	}

	/// Undo the last group of changes.
	pub fn undo(&mut self, env: &mut Env<'_>) -> CommandResult {
		let update = self.history.undo(&mut self.state)?;
		self.finish(update, env.now)
	}

	/// Redo the last undone group.
	pub fn redo(&mut self, env: &mut Env<'_>) -> CommandResult {
		let update = self.history.redo(&mut self.state)?;
		self.finish(update, env.now)
	}

	/// Set the selection from a pointer or the host's own selection UI. The
	/// next edit starts a new undo step.
	pub fn select(&mut self, selection: Selection, env: &mut Env<'_>) -> CommandResult {
		self.history.close_group();
		let update = commands::set_selection(&mut self.state, selection)?;
		self.finish(update, env.now)
	}

	/// Copy the selection.
	pub fn copy(&self) -> Result<Slice, EditorError> {
		commands::slice_selection(&self.state)
	}

	/// Copy the selection and delete it.
	pub fn cut(&mut self, env: &mut Env<'_>) -> Result<(Slice, Option<Update>), EditorError> {
		let slice = commands::slice_selection(&self.state)?;
		let update = commands::delete_selection(&mut self.state)?;
		Ok((slice, self.finish(update, env.now)?))
	}

	/// Paste a slice over the selection, after the paste rules format it.
	pub fn paste(&mut self, slice: Slice, env: &mut Env<'_>) -> CommandResult {
		let mut slice = slice;
		self.format_pasted(&mut slice)?;
		let update = commands::replace_selection(&mut self.state, slice)?;
		self.finish(update, env.now)
	}

	/// Paste plain text: one paragraph per line, or verbatim inside code.
	pub fn paste_text(&mut self, text: &str, env: &mut Env<'_>) -> CommandResult {
		let in_code = position::text_range(self.state.doc(), self.state.selection())
			.is_some_and(|range| self.in_code(range.from.block));

		if !in_code {
			let slice = Slice::from_plain_text(self.schema(), text);

			return self.paste(slice, env);
		}

		let update = commands::insert_text(&mut self.state, text)?.map(|mut update| {
			update.kind = EditKind::Paste;
			update
		});
		self.finish(update, env.now)
	}

	fn format_pasted(&self, slice: &mut Slice) -> Result<(), EditorError> {
		if self.registry.paste_rules.is_empty() {
			return Ok(());
		}

		for tree in slice.blocks_mut() {
			self.format_tree(tree)?;
		}

		Ok(())
	}

	/// Run the paste rules over every text block in `tree`, recursing into
	/// containers and skipping code blocks, whose text must stay literal.
	fn format_tree(&self, tree: &mut BlockTree) -> Result<(), EditorError> {
		let is_code = self.registry.code_blocks.contains(&tree.kind);

		match &mut tree.content {
			TreeContent::Inline(text) if !is_code => {
				for rule in &self.registry.paste_rules {
					rule.apply(text, self.schema())?;
				}
			}

			TreeContent::Blocks(children) => {
				for child in children {
					self.format_tree(child)?;
				}
			}
			TreeContent::Inline(_) | TreeContent::Leaf => {}
		}

		Ok(())
	}

	/// Start or update IME preedit text.
	pub fn set_composition(&mut self, text: &str, env: &mut Env<'_>) -> CommandResult {
		let update = commands::set_composition(&mut self.state, text)?;
		self.finish(update, env.now)
	}

	/// Commit the composed text.
	pub fn commit_composition(&mut self, text: &str, env: &mut Env<'_>) -> CommandResult {
		let update = commands::commit_composition(&mut self.state, text)?;
		self.finish(update, env.now)
	}

	/// Abandon the composition.
	pub fn cancel_composition(&mut self, env: &mut Env<'_>) -> CommandResult {
		let update = commands::cancel_composition(&mut self.state)?;
		self.finish(update, env.now)
	}

	/// Apply steps from a collaborator.
	pub fn apply_remote(
		&mut self,
		steps: impl IntoIterator<Item = Step>,
		env: &mut Env<'_>,
	) -> Result<Update, EditorError> {
		let update = self
			.state
			.apply_steps(steps, EditKind::Remote, Origin::Remote, None)?;
		self.last_rule = None;
		self.history.record(&update, env.now);
		Ok(update)
	}

	/// Let `change` edit the document directly, as a collaborator's change.
	///
	/// A collaboration binding uses this to integrate Yjs updates into the
	/// editor's own document instead of a second copy. `change` must return
	/// the commit of exactly the steps it applied and leave the document
	/// unchanged when it fails. An empty commit is not recorded in history,
	/// so mirroring a local change that needed no follow-up keeps undo
	/// intact.
	pub fn apply_external<E: From<EditorError>>(
		&mut self,
		env: &mut Env<'_>,
		change: impl FnOnce(&mut Document) -> Result<Commit, E>,
	) -> Result<Update, E> {
		let update = self
			.state
			.apply_external(EditKind::Remote, Origin::Remote, change)?;

		if update.doc_changed() {
			self.last_rule = None;
			self.history.record(&update, env.now);
		}

		Ok(update)
	}

	/// The single path every local update takes: extensions fix up the
	/// state, and the result is recorded in history.
	pub(crate) fn finish(&mut self, update: Option<Update>, now: Duration) -> CommandResult {
		self.last_rule = None;

		let Some(update) = update else {
			return Ok(None);
		};
		let update = if update.origin == Origin::Local {
			self.fix_up(update)?
		} else {
			update
		};

		if update.origin == Origin::Local && !self.registry.allows(&self.state, &update) {
			self.state.apply_steps(
				update.commit.undo_steps(),
				update.kind,
				Origin::History,
				Some(update.selection_before),
			)?;

			return Ok(None);
		}

		self.history.record(&update, now);
		Ok(Some(update))
	}

	/// Let every extension adjust the state after a local update, folding
	/// each adjustment into the update so it undoes together.
	fn fix_up(&mut self, update: Update) -> Result<Update, EditorError> {
		let mut update = update;

		for extension in &self.registry.extensions {
			if let Some(extra) = extension.after_update(&mut self.state, &update)? {
				update = update.then(extra);
			}
		}

		Ok(update)
	}
}

/// Whether the schema has the type `node` describes. Extensions that add
/// attributes to other extensions' types, such as text alignment, declare
/// every type they could extend; the metadata keeps the ones that exist.
fn has_node(schema: &Schema, node: &NodeMeta) -> bool {
	match node.kind {
		NodeKind::Block => schema.block_type(node.name).is_some(),
		NodeKind::Mark => schema.mark_type(node.name).is_some(),
		NodeKind::Embed => schema.embed_type(node.name).is_some(),
	}
}

/// Run `commands` in order until one applies.
fn first_applying(commands: &[Command], context: &mut Context<'_>) -> CommandResult {
	for command in commands {
		if let Some(update) = command(context)? {
			return Ok(Some(update));
		}
	}

	Ok(None)
}
