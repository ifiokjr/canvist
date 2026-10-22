---
"canvist_ffi": feat
"canvist-dart": feat
"canvist_flutter": feat
---

# Paint ink strokes on iOS, Android, and in Flutter

Display lists now carry ink strokes past the web canvas to the native and Flutter hosts.

- `canvist_ffi` adds `DisplayItem::Stroke` with the stroke's points, width, colour, and bounds. The iOS and Android `DisplayPainter`s draw it as a polyline with round caps and joins.
- `canvist-dart` adds `ViewItemKind.stroke`, with `ViewItem.points` as `x, y` pairs and `ViewItem.lineWidth`. The item's rectangle bounds the points and caps.
- `canvist_flutter`'s `DisplayPainter` draws strokes the same way. A stroke of one point is a dot.
