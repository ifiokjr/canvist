<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/react-native.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/react-native.yml"><img src="https://github.com/canvist/canvist/actions/workflows/react-native.yml/badge.svg?branch=main" alt="react-native status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/react-native

canvist's rich-text editor for React Native: the native iOS and Android `CanvistView`s as a Fabric component, not a WebView. Typing goes through each platform's own text input (the keyboard, input methods, autocorrect, dictation, selection handles), and the Rust engine lays out and paints every line. Commands, queries, and documents are typed from the extensions you register, with the same API as [`@canvist/web`](../canvist-web).

```tsx
import { CanvistEditor } from "@canvist/react-native";

<CanvistEditor
	content={{ format: "markdown", input: "# Notes" }}
	onUpdate={(editor) => save(editor.export("markdown"))}
	accessibilityLabel="Note"
	style={{ flex: 1 }}
/>;
```

Needs React Native 0.80 or newer with the New Architecture, iOS 17, and Android 7 (API 24).

## A toolbar that follows the selection

`useCanvistEditor` gives you the editor and the props for its view; `useEditorState` re-renders when something you read changes. Calls are synchronous, so a query answers at once:

```tsx
import {
	bold,
	CanvistView,
	heading,
	useCanvistEditor,
	useEditorState,
} from "@canvist/react-native";

function Notes() {
	const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
	const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

	return (
		<>
			<Button
				title={isBold ? "Unbold" : "Bold"}
				onPress={() => editor?.commands.toggleBold()}
			/>
			<CanvistView {...props} style={{ flex: 1 }} />
		</>
	);
}
```

Only registered extensions have commands: with `[bold, heading]`, `editor.commands.toggleItalic()` is a type error, and so is `setHeading({ level: 7 })`.

## The editor

`editor` is `null` until the native view has created its editor. Then it offers:

- `commands`, `can`, and `query`, one method per registered command and query, plus `run`, `canRun`, and `ask` by name.
- `setContent(content)`, `export(format)` (`"json"`, `"html"`, `"markdown"`, or `"text"`), and `toJson()`.
- `selection`, `select(anchor, head)`, `selectAll()`, and `insertText(text)`, which types as the keyboard does.
- `undo()`, `redo()`, `canUndo()`, and `canRedo()`.
- `focus()`, `blur()`, and `focused`.
- `subscribe(listener)`, called after every change to the document, the selection, or focus.

Failures throw a `CanvistError` with a stable `code`, as on every other platform.

## Install

The native editors aren't on a package registry yet, so build them from the repository. For iOS, add the `Canvist` pod to your Podfile after building it with `platforms/apple/build.sh`:

```ruby
pod "Canvist", path: "path/to/canvist/platforms/apple"
```

For Android, the package depends on `io.github.ifiokjr:canvist-android`; until it's on Maven Central, publish it to Maven Local with `platforms/android/build.sh` and `./gradlew :canvist:publishToMavenLocal`, and add `mavenLocal()` to your repositories. [`examples/react-native`](../../examples/react-native) is set up this way.

<details>
<summary><strong>Development</strong></summary>

```sh
npm test                                      # Jest: the typed editor, the view's props and events
npx tsc --noEmit                              # types, including the type-level tests
../../examples/react-native/e2e/test.sh ios   # the example in Release on a simulator, driven by Maestro
../../examples/react-native/e2e/test.sh android
```

The package's `react-native` export is its TypeScript source: React Native's codegen reads the Fabric and TurboModule specs in `src/` when an app builds.

</details>
