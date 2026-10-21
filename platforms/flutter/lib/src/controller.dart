import 'package:canvist/canvist.dart';
import 'package:flutter/foundation.dart';

import 'fonts.dart';
import 'measurer.dart';

/// The document a [CanvistEditor] shows, and the way to change it.
///
/// Commands come from the engine's typed handles, so a toolbar can only call
/// extensions the engine has:
///
/// ```dart
/// final (:engine, :kit) = await Engine.starterKit(
///   platform: CanvistController.keyPlatform,
/// );
/// final controller = CanvistController(engine: engine);
/// controller.run(kit.bold.toggleBold());
/// final bold = controller.query(kit.bold.isBold());
/// ```
///
/// Listeners hear about every change the editor or this controller makes:
/// typing, selection, commands, undo, and collaborators' updates applied
/// through [applyUpdate] or [receiveSync]. Code that changes [doc] directly
/// calls [refresh] afterwards.
class CanvistController extends ChangeNotifier {
  /// A controller for [doc], or for a new empty document from [engine].
  /// Text is laid out with [fonts], which the editor paints with too. The
  /// controller disposes a document it created, never one it was given.
  CanvistController({
    required this.engine,
    Doc? doc,
    this.fonts = const CanvistFonts(),
  }) : doc = doc ?? engine.create(),
       _ownsDoc = doc == null {
    view = this.doc.view(
      ParagraphMeasurer(fonts: fonts),
      options: const ViewOptions(dark: false, overscan: 400),
    );
  }

  /// The key platform of the device the app runs on, for building the
  /// engine: `Engine.starterKit(platform: CanvistController.keyPlatform)`
  /// makes `Mod` Cmd on Apple devices and Ctrl elsewhere, as people expect.
  static KeyPlatform get keyPlatform => switch (defaultTargetPlatform) {
    TargetPlatform.iOS || TargetPlatform.macOS => KeyPlatform.apple,
    _ => KeyPlatform.other,
  };

  /// The engine the document's extensions come from.
  final Engine engine;

  /// The document.
  final Doc doc;

  /// How CSS font families in the theme become Flutter fonts.
  final CanvistFonts fonts;

  /// The document laid out for the editor. The editor drives it; use this
  /// controller's methods to change the document.
  late final EditorView view;

  final bool _ownsDoc;
  var _disposed = false;

  /// The text selection, or `null` for a block selection.
  Range? get selection => doc.selection;

  /// Run [command] from one of the engine's handles against the selection,
  /// as a user would. Returns whether it applied.
  bool run(Command command) => _changed(view.run(command));

  /// Whether [command] would apply.
  bool can(Command command) => doc.can(command);

  /// Answer [query] about the document and selection.
  T query<T>(Query<T> query) => doc.query(query);

  /// Undo the last change. On a shared document, only this peer's.
  bool undo() => _changed(view.undo());

  /// Redo the last undone change.
  bool redo() => _changed(view.redo());

  /// Select from [anchor] to [head], or place the caret at [anchor].
  void select(Position anchor, [Position? head]) {
    doc.select(anchor, head);
    refresh();
  }

  /// Select the whole document.
  void selectAll() {
    doc.selectAll();
    refresh();
  }

  /// Insert [text] at the selection as typing does.
  bool insertText(String text) => _changed(view.insertText(text));

  /// Paste [content] read as [format] over the selection.
  bool paste(Format format, String content) =>
      _changed(doc.paste(format, content));

  /// The document as [format].
  String export(Format format) => doc.export(format);

  /// Apply a collaborator's v1 Yjs update. Returns whether the document
  /// changed.
  bool applyUpdate(Uint8List update) => _changed(doc.applyUpdate(update));

  /// Handle one y-sync message from a provider and say what to send back.
  SyncReply receiveSync(Uint8List message) {
    final reply = doc.receiveSync(message);
    _changed(reply.changed);
    return reply;
  }

  /// Tell listeners the document or selection changed, after changing
  /// [doc] directly.
  void refresh() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  bool _changed(bool changed) {
    if (changed) {
      refresh();
    }

    return changed;
  }

  @override
  void dispose() {
    _disposed = true;
    view.dispose();

    if (_ownsDoc) {
      doc.dispose();
    }

    super.dispose();
  }
}
