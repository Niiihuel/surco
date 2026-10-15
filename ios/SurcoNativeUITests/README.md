# Native iOS interaction tests

`SurcoNativeUI` is a separate shared XCTest scheme. It tests the Release app through public XCUIAutomation, without app launch scripts, injected selectors, or production test hooks. The regular `Surco` archive scheme remains app-only.

The six tests cover:

- The playlist card's real `UIPasteControl` prepares an import form from the runner's clipboard without fetching a playlist.
- The form's `UIPasteControl` pastes into its `UITextField`; Unicode typing survives keyboard presentation, model updates and rotation. Return dismisses the keyboard, and Cancel closes the form.
- A playlist draft closes by dragging its header. Reopening it proves the draft was canceled.
- Connections opens from Settings without playback and exposes the native route picker and volume slider.
- Holding a Home card directly opens UIKit's context menu. Like and Unlike act on the held synthetic track without playback or network requests.
- The 119 canvas symbols and eight additional native action symbols resolve to real `UIImage` symbols in the simulator's UIKit, without launching the app. This checks the current CI runtime; older supported iOS versions still need hardware/runtime validation.

Controls inside sheets are addressed by accessibility IDs. Until the Rust canvas exposes platform accessibility elements, only entry taps and the card hold have coordinate fallbacks. `EntryGeometry.json` records their actual Rust Frame rectangles at the CI iPhone 17 Pro size (402×874, safe insets 62/34, canvas 402×778); a Rust unit test rejects drift. XCTest rejects another window size rather than guessing coordinates. This is not iPad coverage. Assertion failures include the UI hierarchy, and successful checkpoints retain screenshots in the result bundle.

After building the Rust simulator archive, run on a booted simulator owned by the caller:

```sh
python3 scripts/run-ios-ui-tests.py --udid SIMULATOR_UUID --seed-fixture
```

The runner reuses `target/ios/simulator/Rust.xcconfig` and `DerivedData`, disables parallel tests, and writes `artifacts/native-ui/native-ui.xcresult`, the command, source SHA, fixture hashes, Xcode log and JSON summary. It exports attachments into `artifacts/native-ui/attachments`, including when XCTest fails, so screenshots can be reviewed without Xcode. Existing result bundles are preserved; use `--artifacts` for another run. It requires at least six passed tests and rejects failures, skips and empty test plans.

`--seed-fixture` explicitly opts into synthetic app data for this run. After terminating Surco, the runner temporarily writes `SyntheticCollection.ron` to this bundle's `Library/Application Support/surco/app.ron`. It restores the original bytes in `finally`, or removes only that file if it did not exist. Downloads and other files are untouched. The fixture contains six invented tracks with no audio, lyrics or artwork URLs, no playlists, autoplay disabled and volume zero. Without this flag, the caller must prepare equivalent data for the Home context-menu test. The tests cancel drafts and never submit imports or start playback. Normal app startup can still request recommendations and version information; the held card is persisted locally and does not depend on those responses.

The caller handles simulator creation, readiness, software keyboard configuration and cleanup. In ephemeral CI, disable the Simulator host preference `ConnectHardwareKeyboard` before launching tests so keyboard assertions exercise the on-screen keyboard. This script never erases, boots, shuts down or deletes a supplied simulator.

The iOS workflow enables `SURCO_IOS_NATIVE_UI_TESTS=1` only for its simulator matrix job. `build-ios.py` opts into the fixture and runs the native interactions after the existing startup smoke, using that same booted simulator before its `finally` cleanup. A test failure propagates through the build job; startup screenshot success cannot substitute for an interaction pass. Device builds keep their existing unsigned packaging path.

Apple references: [XCUIApplication](https://developer.apple.com/documentation/xcuiautomation/xcuiapplication), [coordinate gestures](https://developer.apple.com/documentation/xcuiautomation/xcuicoordinate), [Xcode test schemes](https://developer.apple.com/documentation/xcode/organizing-tests-to-improve-feedback), and [the xcresulttool commands introduced in Xcode 16](https://developer.apple.com/documentation/xcode-release-notes/xcode-16_3-release-notes).
