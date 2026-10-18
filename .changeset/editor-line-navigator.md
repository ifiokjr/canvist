---
"canvist_editor": minor
"canvist_extensions": patch
---

Settle the `LineNavigator` interface that `canvist_layout` implements.

- `LineNavigator` now has `vertical(&mut self, doc, from: Caret, direction: VerticalDirection, goal_x: Option<f32>) -> Result<Option<VerticalMotion>, NavigationError>` and `line_edge(&mut self, doc, at: Caret, edge: LineEdge) -> Result<Caret, NavigationError>`. Methods take `&mut self` so layout can measure blocks lazily, and return errors instead of guessing when it cannot.
- `Caret` pairs a point with an `Affinity` (`Upstream` or `Downstream`), so the offset where a line soft-wraps can be the end of one line or the start of the next. `EditorState::affinity` tracks it: line and vertical motion set it, and every other change resets it to `Downstream`.
- `Env` holds the navigator mutably. Build one with `Env::new(now, &mut navigator)`, or `Env::headless(now)` for the `ParagraphNavigator` fallback. `Editor` methods now take `&mut Env`.
- `EditorError::Navigation` carries a navigator's error.
