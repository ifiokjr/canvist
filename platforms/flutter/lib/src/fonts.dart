import 'package:canvist/canvist.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';

/// How the editor turns the CSS `font-family` lists in canvist themes, such
/// as `Inter, ui-sans-serif, system-ui, sans-serif`, into Flutter fonts.
///
/// Layout measures and the painter draws through the same [style], so text
/// is always exactly as wide as layout expects, whichever font a platform
/// ends up using.
///
/// Each name in a list becomes a Flutter family, in fallback order, until a
/// generic name: `sans-serif`, `system-ui`, and `ui-sans-serif` end the list
/// on the platform's own font, and `monospace`, `ui-monospace`, and `serif`
/// end it on [monospace] and [serif]. [families] renames any of them.
@immutable
final class CanvistFonts {
  const CanvistFonts({
    this.families = const {},
    this.monospace,
    this.serif,
    this.only,
  });

  /// Every font drawn in [family], for apps that bundle one typeface and
  /// for tests that need the same font everywhere.
  const CanvistFonts.only(String family) : this(only: family);

  /// Flutter families for CSS family names, matched case-insensitively. A
  /// name mapped to `null` means the platform's own font.
  final Map<String, String?> families;

  /// The family `monospace` stands for. Defaults to Menlo on Apple
  /// platforms, Consolas on Windows, and `monospace` elsewhere.
  final String? monospace;

  /// The family `serif` stands for. Defaults to Georgia on Apple platforms
  /// and Windows, and `serif` elsewhere.
  final String? serif;

  /// Draw everything in this family instead.
  final String? only;

  /// The text style that draws [font] in [color].
  TextStyle style(FontSpec font, {Color? color, double letterSpacing = 0}) {
    final names = only == null ? _resolve(font.families) : [only!];

    return TextStyle(
      color: color,
      fontFamily: names.firstOrNull,
      fontFamilyFallback: names.length > 1 ? names.sublist(1) : null,
      fontSize: font.size,
      fontWeight: FontWeight.values.firstWhere(
        (weight) => weight.value >= font.weight,
        orElse: () => FontWeight.w900,
      ),
      fontStyle: font.italic ? FontStyle.italic : FontStyle.normal,
      letterSpacing: letterSpacing == 0 ? null : letterSpacing,
    );
  }

  List<String> _resolve(List<String> names) {
    final resolved = <String>[];

    for (final name in names) {
      final key = name.toLowerCase();

      if (families.containsKey(key) || families.containsKey(name)) {
        final family = families[key] ?? families[name];

        if (family == null) {
          break;
        }

        resolved.add(family);
        continue;
      }

      switch (key) {
        case 'sans-serif' || 'system-ui' || 'ui-sans-serif' || '-apple-system':
          return resolved;
        case 'monospace' || 'ui-monospace':
          resolved.add(monospace ?? _platformMonospace);
          return resolved;
        case 'serif' || 'ui-serif':
          resolved.add(serif ?? _platformSerif);
          return resolved;
        default:
          resolved.add(name);
      }
    }

    return resolved;
  }

  static String get _platformMonospace => switch (defaultTargetPlatform) {
    TargetPlatform.iOS || TargetPlatform.macOS => 'Menlo',
    TargetPlatform.windows => 'Consolas',
    _ => 'monospace',
  };

  static String get _platformSerif => switch (defaultTargetPlatform) {
    TargetPlatform.iOS ||
    TargetPlatform.macOS ||
    TargetPlatform.windows => 'Georgia',
    _ => 'serif',
  };

  @override
  bool operator ==(Object other) =>
      other is CanvistFonts &&
      mapEquals(other.families, families) &&
      other.monospace == monospace &&
      other.serif == serif &&
      other.only == only;

  @override
  int get hashCode => Object.hash(
    Object.hashAllUnordered(families.entries.map((e) => (e.key, e.value))),
    monospace,
    serif,
    only,
  );
}
