/// canvist's document model and editing engine in pure Dart, with no
/// Flutter: convert ProseMirror/TipTap JSON, HTML, Markdown, and text, run
/// typed commands on a server, and serve Yjs collaboration.
///
/// ```dart
/// final (:engine, :kit) = await Engine.starterKit();
/// final doc = engine.parse(Format.markdown, 'Hello world');
///
/// doc.select(const Position([0], 6), const Position([0], 11));
/// doc.run(kit.bold.toggleBold());
/// print(doc.export(Format.html)); // <p>Hello <strong>world</strong></p>
/// ```
///
/// Handles come from registering extensions, so code cannot call the
/// commands of an extension the engine does not have:
///
/// ```dart
/// final builder = EngineBuilder();
/// final heading = builder.add(Extensions.heading);
/// final engine = await builder.build();
/// final doc = engine.create()..run(heading.setHeading(level: 2));
/// ```
library;

export 'src/engine.dart';
