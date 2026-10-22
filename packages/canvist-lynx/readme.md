<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/lynx.md"><strong>Quick start</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/lynx.yml"><img src="https://github.com/canvist/canvist/actions/workflows/lynx.yml/badge.svg?branch=main" alt="lynx status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/lynx

canvist's rich-text editor for [Lynx](https://lynxjs.org): the native iOS and Android `CanvistView`s as a `<canvist-editor>` element, not a web view. Typing goes through each platform's own text input, and the Rust engine lays out and paints every line. Commands, queries, and documents are typed from the extensions you register, with the same API as [`@canvist/web`](../canvist-web) and [`@canvist/react-native`](../canvist-react-native).

```tsx
import { CanvistEditor } from "@canvist/lynx";

<CanvistEditor
	content={{ format: "markdown", input: "# Notes" }}
	onUpdate={(editor) => save(editor.export("markdown"))}
	accessibility-label="Note"
	style={{ flex: 1 }}
/>;
```

Needs ReactLynx, Lynx 4.1, iOS 17, and Android 7 (API 24).

## A toolbar that follows the selection

```tsx
import {
	bold,
	CanvistView,
	heading,
	useCanvistEditor,
	useEditorState,
} from "@canvist/lynx";

function Notes() {
	const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
	const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

	return (
		<view style={{ flex: 1 }}>
			{/* `ignore-focus` keeps the keyboard up while you tap the toolbar. */}
			<view ignore-focus bindtap={() => editor?.commands.toggleBold()}>
				<text>{isBold ? "Unbold" : "Bold"}</text>
			</view>
			<CanvistView {...props} style={{ flex: 1 }} />
		</view>
	);
}
```

The editor has the same calls as on React Native: `commands`, `can`, `query`, `run`, `ask`, `setContent`, `export`, `toJson`, `selection`, `select`, `insertText`, `undo`, `redo`, `focus`, `blur`, and `subscribe`. They are synchronous, and failures throw a `CanvistError` with a stable `code`.

## Install

The native editors aren't on a package registry yet, so build them from the repository, and register the element and its native module in your host app.

**iOS.** Build the engine with `platforms/apple/build.sh`, add both pods to your Podfile, and register the module on your `LynxConfig`. The element registers itself as `<canvist-editor>`:

```ruby
pod "Canvist", path: "path/to/canvist/platforms/apple"
pod "CanvistLynx", path: "path/to/canvist/packages/canvist-lynx"
```

```swift
let config = LynxConfig(provider: provider)
config.register(CanvistLynxModule.self)
```

**Android.** Publish `canvist-android` to Maven Local with `platforms/android/build.sh` and `./gradlew :canvist:publishToMavenLocal`, include `packages/canvist-lynx/android` as a Gradle project, and register both pieces:

```kotlin
LynxEnv.inst().init(application, null, null, null)
CanvistLynx.registerModule()

val lynxView = LynxViewBuilder()
	.setTemplateProvider(provider)
	.addBehavior(CanvistLynx.behavior())
	.build(context)
```

Lynx knows neither the safe area nor the keyboard, so size your LynxView to fit between them, as [`examples/lynx`](../../examples/lynx) does.

<details>
<summary><strong>Development</strong></summary>

```sh
npm test                               # rstest: the typed editor, the element's attributes and events
npx tsc --noEmit                       # types, including the type-level tests
../../examples/lynx/e2e/test.sh ios    # the example on a simulator, driven by Maestro
../../examples/lynx/e2e/test.sh android
```

The build keeps the JSX (`dist/*.jsx`) for the app's ReactLynx compiler, as Lynx libraries do. The native module answers with a JSON envelope rather than throwing, because Lynx reports native exceptions to the host rather than to JavaScript.

</details>
