---
"@canvist/react-native": feat
"@canvist/headless": feat
canvist_ffi: feat
---

# A native rich-text editor for React Native

`@canvist/react-native` puts the iOS and Android `CanvistView`s in React Native apps as a Fabric component, not a WebView, so typing, input methods, autocorrect, dictation, selection handles, VoiceOver, and TalkBack work as they do in the native editors. Commands, queries, and documents are typed from the extensions you register, with the same API as `@canvist/web`, and calls go through a synchronous TurboModule, so a query answers at once:

```tsx
const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
const isBold = useEditorState(editor, (e) => e.query.isBold(), false);

<CanvistView {...props} style={{ flex: 1 }} />;
editor?.commands.toggleHeading({ level: 2 });
editor?.commands.toggleItalic(); // type error: italic is not registered
```

`CanvistEditor` wraps both in one component with `onReady` and `onUpdate`. A call from JavaScript into the editor takes 6 to 47 µs at p95 on an iOS Simulator and 210 to 344 µs on an Android emulator (Release builds); keystrokes never cross into JavaScript. The example app in `examples/react-native` is a notes screen with a formatting toolbar, and its Maestro flows run on a simulator and an emulator in CI.

- `@canvist/headless/native` is the typed editor over any host that runs canvist natively: a small synchronous bridge of names and JSON, with failures rethrown as `CanvistError`s. `@canvist/lynx` will share it.
- The Swift and Android libraries gain what JavaScript hosts share: `CanvistHostView`, a `CanvistView` started from an element's first props and reporting changes and focus; `HostedEditors`, a thread-safe registry of editors by id with failures as `[canvist:<code>] <reason>`; JSON ranges (`selectionJSON()`, `select(rangeJSON:)`); `CanvistView.onFocusChange`; and `ExtensionSet(names:)`.
- `platforms/apple/Canvist.podspec` builds the Swift package with CocoaPods.
- `canvist-android` compiles for the Kotlin 2.2 language and standard library, so apps on Kotlin 2.2, React Native 0.88's version, can use it.
