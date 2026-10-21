import 'package:flutter/widgets.dart';

/// Keys for the widgets the tests drive, shared by the app and its tests.
abstract final class Keys {
  /// The editor on the single-document screen.
  static const editor = Key('editor');

  /// The button that shows the document as Markdown.
  static const export = Key('export');

  /// The Markdown in the export sheet.
  static const markdown = Key('markdown');

  /// The collaborator's pane in two-peer mode.
  static const peer = Key('peer');

  /// The switch that turns two-peer mode on and off.
  static const peers = Key('peers');

  /// This device's pane in two-peer mode.
  static const you = Key('you');

  /// The toolbar button for [action], such as `bold` or `undo`.
  static Key toolbar(String action) => Key(action);
}
