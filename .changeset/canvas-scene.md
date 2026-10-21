---
"canvist_scene": minor
---

# Add `canvist_scene`, the canvas's document model

`canvist_scene` models a canvas as a tree of typed nodes (pages, frames, groups, rectangles, ellipses, polygons and stars, vector paths, booleans, images, text frames, components, and instances), each with typed properties, ordered by fractional keys.

- Every change is one of five invertible steps (`InsertNodes`, `RemoveNodes`, `Move`, `SetProp`, `Text`), grouped in transactions that roll back on drop and invert exactly for undo.
- A text frame holds a full `canvist_model` document, so the text editor works inside it.
- Scenes read and write JSON in a canonical flat form and in a nested form, with optional ids and keys, that people and agents write.
