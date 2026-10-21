/// canvist's rich-text editor for Flutter.
///
/// [CanvistEditor] paints a document laid out by canvist's Rust core and
/// edits it through the platform's text input, keys, touch, and mouse. A
/// [CanvistController] holds the document and runs typed commands from the
/// engine's extension handles. The document model and editing engine come
/// from the pure Dart `canvist` package, re-exported here, so the same code
/// runs on a Dart server and in a Flutter app.
///
/// ```dart
/// final (:engine, :kit) = await Engine.starterKit(
///   platform: CanvistController.keyPlatform,
/// );
/// final controller = CanvistController(engine: engine);
///
/// CanvistEditor(controller: controller, autofocus: true);
/// controller.run(kit.bold.toggleBold());
/// ```
library;

export 'package:canvist/canvist.dart';

export 'src/controller.dart' show CanvistController;
export 'src/editor.dart'
    show CanvistContextMenuBuilder, CanvistEditor, CanvistStyle;
export 'src/fonts.dart' show CanvistFonts;
export 'src/input.dart' show EditorInput;
export 'src/measurer.dart' show ParagraphMeasurer;
export 'src/painter.dart' show DisplayPainter;
