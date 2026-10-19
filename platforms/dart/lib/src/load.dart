import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart';

import 'rust/frb_generated.dart';

/// The native asset the build hook produces, named after the library that
/// owns the generated bindings.
const _asset = 'package:canvist/src/rust/frb_generated.io.dart';

/// A symbol every flutter_rust_bridge library exports, used to find ours.
@Native<Int32 Function()>(symbol: 'frb_get_rust_content_hash', assetId: _asset)
external int _contentHash();

final class _DlInfo extends Struct {
  external Pointer<Utf8> fileName;
  external Pointer<Void> fileBase;
  external Pointer<Utf8> symbolName;
  external Pointer<Void> symbolAddress;
}

typedef _DladdrNative = Int32 Function(Pointer<Void>, Pointer<_DlInfo>);
typedef _Dladdr = int Function(Pointer<Void>, Pointer<_DlInfo>);

Future<void>? _loading;

/// Load the Rust library once.
///
/// flutter_rust_bridge's default loader opens the library by file name,
/// which finds it inside a Flutter app bundle but not where `dart run`,
/// `dart test`, and compiled executables keep native assets. On those, the
/// Dart VM resolves the asset itself when a `@Native` function bound to it
/// is first used, so canvist resolves one and asks the dynamic linker which
/// file it came from.
Future<void> loadCanvist() => _loading ??= RustLib.init(
  externalLibrary: Platform.isAndroid || Platform.isIOS
      ? null
      : ExternalLibrary.open(_nativeAssetPath()),
);

String _nativeAssetPath() {
  final symbol = Native.addressOf<NativeFunction<Int32 Function()>>(
    _contentHash,
  ).cast<Void>();
  final dladdr = DynamicLibrary.process()
      .lookupFunction<_DladdrNative, _Dladdr>('dladdr');
  final info = calloc<_DlInfo>();

  try {
    if (dladdr(symbol, info) == 0) {
      throw StateError('the dynamic linker does not know the canvist library');
    }

    return info.ref.fileName.toDartString();
  } finally {
    calloc.free(info);
  }
}
