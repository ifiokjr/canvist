# Quick start: Lynx

In this guide you add a canvist editor to a [Lynx](https://lynxjs.org) app, give it a toolbar that follows the selection, and save what people write. It takes about fifteen minutes, most of it building the native editors and wiring them into your host apps.

**Status: available.** `@canvist/lynx` shows the native iOS and Android editors as a `<canvist-editor>` element, not a web view. Typing goes through each platform's own text input, and the Rust engine lays out and paints every line. Commands and queries are typed from the extensions you register, with the same API as [`@canvist/react-native`](./react-native.md) and [`@canvist/web`](./web.md), and they answer synchronously.

## 1. Install

You need ReactLynx, Lynx 4.1, iOS 17, Android 7 (API 24), and a [Rust toolchain](https://rustup.rs). The native editors aren't on a registry yet, so build them from the repository:

```sh
git clone https://github.com/canvist/canvist
canvist/platforms/apple/build.sh                                         # the iOS engine
canvist/platforms/android/build.sh                                       # the Android engine
(cd canvist/platforms/android && ./gradlew :canvist:publishToMavenLocal) # canvist-android, to Maven Local
(cd canvist/packages/canvist-headless && npm ci && npm run build:wasm && npm run build)
(cd canvist/packages/canvist-lynx && npm ci && npm run build)
```

Add the package to your Lynx app:

```sh
npm install path/to/canvist/packages/canvist-lynx path/to/canvist/packages/canvist-headless
```

Then register canvist in each host app. **iOS:** add the pods, and register the native module on your `LynxConfig`. The element registers itself as `<canvist-editor>`.

```ruby
# Podfile: canvist's iOS editor needs iOS 17.
platform :ios, '17.0'
pod 'Canvist', :path => 'path/to/canvist/platforms/apple'
pod 'CanvistLynx', :path => 'path/to/canvist/packages/canvist-lynx'
```

```swift
let config = LynxConfig(provider: provider)
config.register(CanvistLynxModule.self)
builder.config = config
```

**Android:** include `packages/canvist-lynx/android` as a Gradle project with Maven Local in your repositories, register the module once, and add the element's behavior to each `LynxViewBuilder`:

```kotlin
// settings.gradle.kts
include(":canvist-lynx")
project(":canvist-lynx").projectDir = file("path/to/canvist/packages/canvist-lynx/android")
```

```kotlin
LynxEnv.inst().init(application, null, null, null)
CanvistLynx.registerModule()

val lynxView = LynxViewBuilder()
    .setTemplateProvider(provider)
    .addBehavior(CanvistLynx.behavior())
    .build(context)
```

Lynx knows neither the safe area nor the keyboard, so size your LynxView between them in the host app: the keyboard layout guide on iOS, and window insets on Android.

## 2. Show an editor

`CanvistEditor` is the native editor. Give it a size, since it scrolls its own document:

```tsx
import { CanvistEditor } from "@canvist/lynx";

export function App() {
	return (
		<CanvistEditor
			content={{ format: "markdown", input: "# My note\n\nStart typing." }}
			accessibility-label="Note"
			style={{ flex: 1 }}
		/>
	);
}
```

## 3. Add a toolbar

`useCanvistEditor` gives you the editor and the attributes for its element, and `useEditorState` re-renders a component when something it reads changes:

```tsx
import {
	bold,
	CanvistView,
	heading,
	useCanvistEditor,
	useEditorState,
} from "@canvist/lynx";

export function App() {
	const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
	const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

	return (
		<view style={{ display: "flex", flexDirection: "column", height: "100%" }}>
			{/* `ignore-focus` keeps the keyboard up while you tap the toolbar. */}
			<view ignore-focus style={{ display: "flex", flexDirection: "row" }}>
				<view
					bindtap={() =>
						editor?.commands.toggleBold()}
				>
					<text>{isBold ? "Unbold" : "Bold"}</text>
				</view>
				<view bindtap={() => editor?.commands.toggleHeading({ level: 2 })}>
					<text>Heading</text>
				</view>
			</view>
			<CanvistView {...props} accessibility-label="Note" style={{ flex: 1 }} />
		</view>
	);
}
```

`editor` is `null` until the element has created its editor. Commands return whether they applied, queries answer at once, and a failure throws a `CanvistError` with a stable `code`. On Android, give elements that screen readers should reach `flatten={false}`, so Lynx keeps a view for them.

## 4. Save and load

```tsx
<CanvistEditor
	onUpdate={(editor) => save(editor.export("markdown"))}
	style={{ flex: 1 }}
/>;

editor.setContent({ format: "json", input: saved }); // ProseMirror JSON, lossless
```

## Performance

Keystrokes never cross into JavaScript: typing stays in the native view, under the same [keystroke-to-paint budgets](../guides/performance.md) as the iOS and Android editors. Commands and queries go through Lynx's native module bridge, which costs more than a React Native TurboModule: in Release builds a query takes about 120 µs at p95 on an iOS Simulator and 340 µs on an Android emulator.

## Next steps

- [`examples/lynx`](https://github.com/canvist/canvist/tree/main/examples/lynx) is the notes screen in ReactLynx, with iOS and Android host apps to copy from.
- [Commands and typed handles](../concepts/commands.md) lists what every extension can do.
