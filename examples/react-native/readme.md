# canvist on React Native

A notes screen on [`@canvist/react-native`](../../packages/canvist-react-native): the native iOS or Android editor, a toolbar that follows the selection, the note as Markdown, a word count, and a benchmark of the synchronous calls from JavaScript into the editor.

```sh
npm install
(cd ../../packages/canvist-headless && npm run build)
npm run ios            # or: npm run android, with Metro from npm start
```

The iOS app needs the `Canvist` pod built first (`platforms/apple/build.sh`); the Android app needs `canvist-android` in Maven Local (`platforms/android/build.sh`, then `./gradlew :canvist:publishToMavenLocal` there).

`e2e/test.sh ios` and `e2e/test.sh android` build the app in Release and run the Maestro flows in `e2e/`, which CI runs on a simulator and an emulator.
