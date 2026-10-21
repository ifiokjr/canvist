import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';

import 'load.dart';
import 'rust/api/canvist.dart';
import 'rust/api/view.dart';

part 'extensions.g.dart';
part 'view.dart';

/// A format documents are read from and written to.
enum Format {
  /// ProseMirror and TipTap JSON, with block ids in `attrs.id`.
  json(DocFormat.json),

  /// Semantic HTML.
  html(DocFormat.html),

  /// Markdown, with GitHub task lists and strikethrough.
  markdown(DocFormat.markdown),

  /// Plain text: one paragraph per line.
  text(DocFormat.text);

  const Format(this._bridge);

  final DocFormat _bridge;
}

/// Which platform's key bindings an engine follows: what `Mod` in a binding
/// such as `Mod-b` means.
enum KeyPlatform {
  /// macOS, iOS, and iPadOS: `Mod` is Cmd.
  apple(DocPlatform.apple),

  /// Everything else: `Mod` is Ctrl.
  other(DocPlatform.other);

  const KeyPlatform(this._bridge);

  final DocPlatform _bridge;
}

/// The stable name of each kind of failure, shared with Rust and
/// TypeScript.
enum ErrorCode {
  config,
  parse,
  schema,
  unknownName,
  invalidArgs,
  invalidPosition,
  invalidKey,
  editor,
  snapshot,
  notShared,
  alreadyShared,
  collab,
  render,
}

/// A failure inside canvist.
final class CanvistException implements Exception {
  const CanvistException(this.code, this.message);

  /// What kind of failure this is.
  final ErrorCode code;

  /// What went wrong.
  final String message;

  @override
  String toString() => 'CanvistException(${code.name}): $message';
}

/// Run [call], turning bridge failures into [CanvistException]s.
T _guard<T>(T Function() call) {
  try {
    return call();
  } on CanvistFailure catch (failure) {
    throw CanvistException(
      ErrorCode.values.byName(failure.code),
      failure.message,
    );
  }
}

/// A point in a text block: child indexes from the root, and UTF-16 code
/// units into its text, like Dart string indexes.
final class Position {
  const Position(this.path, this.offset);

  /// Child indexes from the root: `[1, 0, 0]` is the paragraph in the
  /// first item of a list that is the second block.
  final List<int> path;

  /// UTF-16 code units into the block's text.
  final int offset;

  DocPosition get _bridge =>
      DocPosition(path: Uint32List.fromList(path), offset: offset);

  @override
  bool operator ==(Object other) =>
      other is Position &&
      other.offset == offset &&
      _listEquals(other.path, path);

  @override
  int get hashCode => Object.hash(offset, Object.hashAll(path));

  @override
  String toString() => 'Position($path, $offset)';
}

bool _listEquals(List<int> a, List<int> b) =>
    a.length == b.length &&
    Iterable<int>.generate(a.length).every((index) => a[index] == b[index]);

/// A text selection; a caret when [anchor] equals [head].
typedef Range = ({Position anchor, Position head});

/// What [Doc.receiveSync] says to send.
typedef SyncReply = ({
  List<Uint8List> reply,
  List<Uint8List> broadcast,
  bool changed,
});

/// How well a shared document's state fits the schema.
typedef Validation = ({int rejectedBlocks, bool waiting});

/// Proof that a handle's extension was registered on one engine. Only
/// [EngineBuilder.add] creates handles, so code cannot reach the commands
/// of an extension it never registered.
final class Registration {
  Registration._();
}

/// An extension that can be registered on an [EngineBuilder], returning a
/// handle of type [H].
sealed class Extension<H extends ExtensionHandle> {
  const Extension(this.name);

  /// The extension's name in the metadata.
  final String name;

  H _handle(Registration registration);
}

/// Methods for one registered extension's commands and queries.
abstract base class ExtensionHandle {
  ExtensionHandle._(this._registration);

  final Registration _registration;

  Command _command(String name, [Map<String, Object?> args = const {}]) =>
      Command._(_registration, name, args);

  Query<T> _query<T>(
    String name,
    T Function(Object? json) decode, [
    Map<String, Object?> args = const {},
  ]) => Query._(_registration, name, decode, args);
}

/// A command from a handle, run with [Doc.run] or checked with [Doc.can].
final class Command {
  const Command._(this._registration, this.name, this.args);

  final Registration _registration;

  /// The command's name in the metadata.
  final String name;

  /// Its arguments.
  final Map<String, Object?> args;
}

/// A query from a handle, answered with [Doc.query].
final class Query<T> {
  const Query._(this._registration, this.name, this._decode, this.args);

  final Registration _registration;

  /// The query's name in the metadata.
  final String name;

  /// Its arguments.
  final Map<String, Object?> args;

  final T Function(Object? json) _decode;
}

/// Registers extensions and builds an [Engine].
///
/// ```dart
/// final builder = EngineBuilder();
/// final bold = builder.add(Extensions.bold);
/// final engine = await builder.build();
/// final doc = engine.parse(Format.markdown, 'Hello');
/// doc.selectAll();
/// doc.run(bold.toggleBold());
/// ```
final class EngineBuilder {
  EngineBuilder();

  final _registration = Registration._();
  final _names = <String>[];

  /// Paragraphs, which every engine has.
  late final ParagraphHandle paragraph = ParagraphHandle._(_registration);

  /// Register [extension] and return its handle.
  H add<H extends ExtensionHandle>(Extension<H> extension) {
    if (!_names.contains(extension.name)) {
      _names.add(extension.name);
    }

    return extension._handle(_registration);
  }

  /// Load canvist if needed and build the engine. Block ids are allocated
  /// as [client], which collaborating peers must not share. Key bindings
  /// follow [platform]: editors pass [KeyPlatform.apple] on Apple devices,
  /// so `Mod-b` is Cmd-B there.
  Future<Engine> build({
    int client = 1,
    KeyPlatform platform = KeyPlatform.other,
  }) async {
    await loadCanvist();

    return Engine._(
      _guard(
        () => CanvistEngine(
          extensions: _names,
          client: client,
          platform: platform._bridge,
        ),
      ),
      _registration,
      platform,
    );
  }
}

/// A set of extensions that creates documents.
final class Engine {
  Engine._(this._bridge, this._registration, this.platform);

  final CanvistEngine _bridge;
  final Registration _registration;

  /// Which platform's key bindings the engine's documents follow.
  final KeyPlatform platform;

  /// An engine with TipTap's StarterKit and the handles for all of it.
  static Future<({Engine engine, StarterKitHandles kit})> starterKit({
    int client = 1,
    KeyPlatform platform = KeyPlatform.other,
  }) async {
    final builder = EngineBuilder();
    final kit = StarterKitHandles._(builder);

    return (
      engine: await builder.build(client: client, platform: platform),
      kit: kit,
    );
  }

  /// What every registered extension adds, as Rust reports it.
  List<Object?> get metadata => jsonDecode(_bridge.metadata()) as List<Object?>;

  /// A document with one empty paragraph.
  Doc create() => Doc._(_guard(_bridge.create), _registration);

  /// Read [input] as [format].
  Doc parse(Format format, String input) => Doc._(
    _guard(() => _bridge.parse(format: format._bridge, input: input)),
    _registration,
  );

  /// Read a ProseMirror or TipTap document.
  Doc fromJson(Map<String, Object?> json) =>
      parse(Format.json, jsonEncode(json));

  /// Join a shared document from v1 Yjs [state] as yrs client [clientId].
  Doc fromYjs(Uint8List state, {required int clientId}) => Doc._(
    _guard(() => _bridge.fromYjs(state: state, clientId: clientId)),
    _registration,
  );

  /// Recreate a document from [Doc.snapshot] text.
  Doc restore(String snapshot) =>
      Doc._(_guard(() => _bridge.restore(snapshot: snapshot)), _registration);

  /// Release the engine's memory now rather than when it is collected.
  void dispose() => _bridge.dispose();
}

/// One document and the editor that changes it. Every call is one undo
/// step.
final class Doc {
  Doc._(this._bridge, this._registration);

  final CanvistDoc _bridge;
  final Registration _registration;

  void _check(Registration registration, String name) {
    if (!identical(registration, _registration)) {
      throw ArgumentError.value(name, 'name', 'comes from another engine');
    }
  }

  /// Run [command] against the selection; returns whether it applied.
  bool run(Command command) {
    _check(command._registration, command.name);

    return runNamed(command.name, command.args);
  }

  /// Whether [command] would apply. Nothing changes.
  bool can(Command command) {
    _check(command._registration, command.name);

    return _guard(
      () => _bridge.can(name: command.name, args: jsonEncode(command.args)),
    );
  }

  /// Answer [query].
  T query<T>(Query<T> query) {
    _check(query._registration, query.name);

    return query._decode(queryNamed(query.name, query.args));
  }

  /// Run the command named [name], for callers that pick commands at
  /// runtime. Arguments are checked against the command's declaration.
  bool runNamed(String name, [Map<String, Object?> args = const {}]) =>
      _guard(() => _bridge.run(name: name, args: jsonEncode(args)));

  /// Whether the command named [name] would apply.
  bool canNamed(String name, [Map<String, Object?> args = const {}]) =>
      _guard(() => _bridge.can(name: name, args: jsonEncode(args)));

  /// Answer the query named [name], as decoded JSON.
  Object? queryNamed(String name, [Map<String, Object?> args = const {}]) =>
      jsonDecode(
        _guard(() => _bridge.query(name: name, args: jsonEncode(args))),
      );

  /// The document as [format].
  String export(Format format) => _bridge.export_(format: format._bridge);

  /// The document as ProseMirror JSON, with block ids in `attrs.id`.
  Map<String, Object?> toJson() =>
      jsonDecode(export(Format.json)) as Map<String, Object?>;

  /// The text selection, or `null` for a block selection.
  Range? get selection {
    final ends = _bridge.selection();

    if (ends == null) {
      return null;
    }

    Position position(DocPosition end) =>
        Position(end.path.toList(), end.offset);

    return (anchor: position(ends[0]), head: position(ends[1]));
  }

  /// Select from [anchor] to [head], or place a caret at [anchor].
  void select(Position anchor, [Position? head]) => _guard(
    () =>
        _bridge.select(anchor: anchor._bridge, head: (head ?? anchor)._bridge),
  );

  /// Select the whole document.
  void selectAll() => _guard(_bridge.selectAll);

  /// Replace the selection with [text], without input rules.
  bool insertText(String text) => _guard(() => _bridge.insertText(text: text));

  /// Type [text] one character at a time, so input rules such as `## `
  /// apply.
  bool typeText(String text) => _guard(() => _bridge.typeText(text: text));

  /// Delete the selection.
  bool deleteSelection() => _guard(_bridge.deleteSelection);

  /// Press a key binding such as `Enter`, `Tab`, or `Mod-b` (`Mod` is Ctrl).
  bool press(String key) => _guard(() => _bridge.press(key: key));

  /// Paste [content] read as [format] over the selection.
  bool paste(Format format, String content) =>
      _guard(() => _bridge.paste(format: format._bridge, content: content));

  /// Undo the last call that changed the document.
  bool undo() => _guard(_bridge.undo);

  /// Redo the last undone call.
  bool redo() => _guard(_bridge.redo);

  /// The content and selection as snapshot JSON, for [Engine.restore].
  String snapshot() => _bridge.snapshot();

  /// Start sharing the document as yrs client [clientId].
  void share(int clientId) => _guard(() => _bridge.share(clientId: clientId));

  /// Apply a v1 Yjs update; returns whether the document changed.
  bool applyUpdate(Uint8List update) =>
      _guard(() => _bridge.applyUpdate(update: update));

  /// The whole shared state as one v1 update.
  Uint8List encodeState() => _guard(_bridge.encodeState);

  /// The v1 state vector.
  Uint8List stateVector() => _guard(_bridge.stateVector);

  /// Everything a peer at [stateVector] is missing, as one v1 update.
  Uint8List encodeDiff(Uint8List stateVector) =>
      _guard(() => _bridge.encodeDiff(stateVector: stateVector));

  /// Drain this peer's own v1 updates for broadcasting.
  List<Uint8List> takeUpdates() => _guard(_bridge.takeUpdates);

  /// Check the shared state against the schema.
  Validation validate() {
    final validation = _guard(_bridge.validate);

    return (
      rejectedBlocks: validation.rejectedBlocks,
      waiting: validation.waiting,
    );
  }

  /// The y-sync message to greet a connecting client with.
  Uint8List syncStart() => _guard(_bridge.syncStart);

  /// Handle one y-sync message from a connection.
  SyncReply receiveSync(Uint8List message) {
    final sync = _guard(() => _bridge.receiveSync(message: message));

    return (
      reply: sync.reply,
      broadcast: sync.broadcast,
      changed: sync.changed,
    );
  }

  /// This peer's changes since the last call, as y-sync messages for every
  /// connection.
  List<Uint8List> flushSync() => _guard(_bridge.flushSync);

  /// HTML and the stylesheet that lays it out [width] pixels wide exactly
  /// as the editor does, for server rendering.
  ({String html, String css}) toHtml(double width) {
    final html = _bridge.toHtml(width: width);

    return (html: html.html, css: html.css);
  }

  /// The whole document as an SVG image [width] pixels wide, measured with
  /// the bundled Atkinson Hyperlegible font.
  String toSvg(double width) => _guard(() => _bridge.toSvg(width: width));

  /// What [toSvg] paints, as a display list, for hosts that compare their
  /// own rendering with server rendering.
  ViewDisplay serverDisplay(double width) =>
      _guard(() => _bridge.serverDisplay(width: width));

  /// An interactive view of this document for a UI host, measuring text
  /// with [measurer]. See [EditorView].
  EditorView view(
    TextMeasurer measurer, {
    ViewOptions options = const ViewOptions(dark: false, overscan: 0),
  }) => EditorView._(this, measurer, options);

  /// Release the document's memory now rather than when it is collected.
  void dispose() => _bridge.dispose();
}
