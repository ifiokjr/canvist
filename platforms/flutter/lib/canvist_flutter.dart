/// Flutter binding for the canvist Rust core.
///
/// Call `await RustLib.init()` once before creating an [EditorHandle].
library;

export 'src/rust/api/editor.dart';
export 'src/rust/frb_generated.dart' show RustLib;
