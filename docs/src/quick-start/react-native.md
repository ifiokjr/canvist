# Quick start: React Native

In this guide you add a canvist editor to a React Native app, give it a toolbar that follows the selection, and save what people write. It takes about ten minutes, most of it building the native editors once.

**Status: preview.** `@canvist/react-native` works and its tests run on every change, but it won't be on npm in canvist's first release, because the native editors it builds on aren't on CocoaPods or Maven Central yet. Install it from GitHub, as below. It shows the native iOS and Android editors as a Fabric component, not a WebView. Typing goes through each platform's own text input, so the keyboard, input methods, autocorrect, dictation, selection handles, VoiceOver, and TalkBack all work as they do in a native text view. Commands and queries are typed from the extensions you register, with the same API as [`@canvist/web`](./web.md), and they answer synchronously.

## 1. Install

You need React Native 0.80 or newer with the New Architecture (the default), Xcode 16 or newer, iOS 17, Android 7 (API 24), and a [Rust toolchain](https://rustup.rs). The native editors aren't on a registry yet, so build them from the repository:

```sh
git clone https://github.com/canvist/canvist
canvist/platforms/apple/build.sh                                         # the iOS engine
canvist/platforms/android/build.sh                                       # the Android engine
(cd canvist/platforms/android && ./gradlew :canvist:publishToMavenLocal) # canvist-android, to Maven Local
(cd canvist/packages/canvist-headless && npm ci && npm run build:wasm && npm run build)
```

Then add the package, and point your app at the native editors:

```sh
npm install path/to/canvist/packages/canvist-react-native path/to/canvist/packages/canvist-headless
```

```ruby
# ios/Podfile, inside your app's target: canvist's iOS editor needs iOS 17.
platform :ios, '17.0'
pod 'Canvist', :path => 'path/to/canvist/platforms/apple'
```

```groovy
// android/build.gradle
allprojects {
    repositories {
        mavenLocal { content { includeGroup("io.github.ifiokjr") } }
    }
}
```

canvist's engine is built for `arm64-v8a` and `x86_64` on Android, so build your app for those (`reactNativeArchitectures=arm64-v8a,x86_64` in `gradle.properties`). Run `pod install`, then build the app as usual.

## 2. Show an editor

`CanvistEditor` is the native editor. Give it a size, since it scrolls its own document:

```tsx
import { CanvistEditor } from "@canvist/react-native";

export function NoteScreen() {
	return (
		<CanvistEditor
			content={{ format: "markdown", input: "# My note\n\nStart typing." }}
			accessibilityLabel="Note"
			style={{ flex: 1 }}
		/>
	);
}
```

Run the app and tap the text. Type `##` at the start of a line for a heading, `-` for a list, or `[ ]` for a checklist.

## 3. Add a toolbar

`useCanvistEditor` gives you the editor and the props for its view, and `useEditorState` re-renders a component when something it reads changes. Register only the extensions you use; the rest are type errors:

```tsx
import {
	bold,
	CanvistView,
	heading,
	history,
	useCanvistEditor,
	useEditorState,
} from "@canvist/react-native";
import { Button, View } from "react-native";

export function NoteScreen() {
	const { editor, props } = useCanvistEditor({
		extensions: [bold, heading, history],
	});
	const isBold = useEditorState(editor, (e) => e.query.isBold(), false);
	const canUndo = useEditorState(editor, (e) => e.canUndo(), false);

	return (
		<View style={{ flex: 1 }}>
			<View style={{ flexDirection: "row" }}>
				<Button
					title={isBold ? "Unbold" : "Bold"}
					onPress={() => editor?.commands.toggleBold()}
				/>
				<Button
					title="Heading"
					onPress={() => editor?.commands.toggleHeading({ level: 2 })}
				/>
				<Button
					title="Undo"
					disabled={!canUndo}
					onPress={() => editor?.undo()}
				/>
			</View>
			<CanvistView {...props} accessibilityLabel="Note" style={{ flex: 1 }} />
		</View>
	);
}
```

`editor` is `null` until the native view has created its editor. Commands return whether they applied, queries answer at once, and a failure throws a `CanvistError` with a stable `code` such as `invalidArgs`. Put `keyboardShouldPersistTaps="always"` on a `ScrollView` that holds your toolbar, so tapping it keeps the keyboard up.

## 4. Save and load

Export the document in any format, and load it back later:

```tsx
<CanvistEditor
	onUpdate={(editor) => save(editor.export("markdown"))}
	style={{ flex: 1 }}
/>;

editor.setContent({ format: "json", input: saved }); // ProseMirror JSON, lossless
```

`onUpdate` runs after every change to the document, the selection, or focus. To save less often, debounce it.

## Performance

Keystrokes never cross into JavaScript: typing stays in the native view, under the same [keystroke-to-paint budgets](../guides/performance.md) as the iOS and Android editors. Your toolbar's commands and queries do cross, through a synchronous TurboModule. In Release builds a query takes about 6 µs at p95 on an iOS Simulator and 250 µs on an Android emulator, and a command 45 µs and 350 µs. The example app's benchmark measures them on your device.

## Next steps

- [`examples/react-native`](https://github.com/canvist/canvist/tree/main/examples/react-native) is a notes screen with a full toolbar, a live Markdown view, and that benchmark.
- [Commands and typed handles](../concepts/commands.md) lists what every extension can do.
- On Lynx instead? See the [Lynx quick start](./lynx.md).
