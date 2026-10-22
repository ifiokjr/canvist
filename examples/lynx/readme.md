# canvist on Lynx

The notes screen from [`examples/react-native`](../react-native), in ReactLynx on [`@canvist/lynx`](../../packages/canvist-lynx): the native editor, a toolbar that follows the selection, the note as Markdown, and a benchmark of the calls from JavaScript into the editor.

- `src/` is the Lynx app, which `npm run build` bundles into `dist/main.lynx.bundle`.
- `ios/` is the iOS host app (`xcodegen`, CocoaPods), and `android/` the Android one. Each loads the bundle, registers `@canvist/lynx`, and sizes its LynxView between the safe area and the keyboard, which Lynx knows nothing of.
- `e2e/` holds the Maestro flows CI runs, and `test.sh`, which builds everything and runs them:

```sh
npm install
(cd ../../packages/canvist-headless && npm run build) && (cd ../../packages/canvist-lynx && npm install && npm run build)
e2e/test.sh ios       # or: e2e/test.sh android
```
