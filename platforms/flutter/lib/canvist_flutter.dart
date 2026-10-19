/// Flutter binding for canvist.
///
/// The document model and editing engine come from the pure Dart
/// [`canvist`](../dart) package, re-exported here, so the same code runs on
/// a Dart server and in a Flutter app. Load the engine once, before
/// `runApp`:
///
/// ```dart
/// final (:engine, :kit) = await Engine.starterKit();
/// ```
library;

export 'package:canvist/canvist.dart';
