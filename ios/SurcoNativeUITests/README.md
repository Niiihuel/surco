# Native iOS interaction tests

`SurcoNativeUI` is a separate shared XCTest scheme. It tests the Release app through public XCUIAutomation, without app launch scripts, injected selectors, or production test hooks. The regular `Surco` archive scheme remains app-only.

The four tests cover:

- The playlist card's real `UIPasteControl` prepares an import form from the runner's clipboard without fetching a playlist.
- The form's `UIPasteControl` pastes into its `UITextField`; Unicode typing survives keyboard presentation, model updates and rotation. Return dismisses the keyboard, and Cancel closes the form.
- A playlist draft closes by dragging its header. Reopening it proves the draft was canceled.
- Connections opens from Settings without playback and exposes the native route picker and volume slider.

Controls inside sheets are addressed by accessibility IDs. Until the Rust canvas exposes platform accessibility elements, entry taps have a coordinate fallback for modern iPhones with a home indicator, derived from the existing phone layout. Use the same iPhone simulator family as unsigned CI; this is not iPad coverage. Assertion failures include the UI hierarchy, and successful checkpoints retain screenshots in the result bundle.

After building the Rust simulator archive, run on a booted simulator owned by the caller:

```sh
python3 scripts/run-ios-ui-tests.py --udid SIMULATOR_UUID
```

The runner reuses `target/ios/simulator/Rust.xcconfig` and `DerivedData`, disables parallel tests, and writes `artifacts/native-ui/native-ui.xcresult`, the command, Xcode log and JSON summary. Existing result bundles are preserved; use `--artifacts` for another run. It requires at least four passed tests and rejects failures, skips and empty test plans.

The caller handles simulator creation, readiness, software keyboard configuration and cleanup. In ephemeral CI, disable the Simulator host preference `ConnectHardwareKeyboard` before launching tests so keyboard assertions exercise the on-screen keyboard. This script never erases, boots, shuts down or deletes a supplied simulator.

The iOS workflow enables `SURCO_IOS_NATIVE_UI_TESTS=1` only for its simulator matrix job. `build-ios.py` runs the native interactions after the existing startup smoke, using that same booted simulator before its `finally` cleanup. A test failure propagates through the build job; startup screenshot success cannot substitute for an interaction pass. Device builds keep their existing unsigned packaging path.

Apple references: [XCUIApplication](https://developer.apple.com/documentation/xcuiautomation/xcuiapplication), [coordinate gestures](https://developer.apple.com/documentation/xcuiautomation/xcuicoordinate), [Xcode test schemes](https://developer.apple.com/documentation/xcode/organizing-tests-to-improve-feedback), and [the xcresulttool commands introduced in Xcode 16](https://developer.apple.com/documentation/xcode-release-notes/xcode-16_3-release-notes).
