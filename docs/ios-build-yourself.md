# Build Surco for your iPhone

Surco has no App Store listing. Whoever wants it on an iPhone builds it from the
source, signed with their own Apple ID. Nothing in the app is tied to the project's
Apple team: only the TestFlight upload needs one, and a fork brings its own.

## Without a Mac: GitHub Actions and a sideloading tool

1. Fork [Niiihuel/surco](https://github.com/Niiihuel/surco) and open the fork's
   **Actions** tab. GitHub asks once to enable workflows in a fork.
2. Run the **iOS** workflow (**Run workflow** on `main`). It checks out limn at the
   commit the app is built against, builds the Rust engine and links the Xcode app on
   a macOS runner.
3. When it finishes, download the `ios-device-unsigned` artifact and unzip it. It
   contains `Surco-unsigned.ipa`.
4. Install the IPA with a sideloading tool such as
   [Sideloadly](https://sideloadly.io) or [AltStore](https://altstore.io), signing
   in with your Apple ID. The tool re-signs the app for your device.

A free Apple ID works; Apple then limits the signature to seven days, after which
the tool signs it again. A paid Apple Developer membership extends it to a year.
Background playback, the Lock Screen and Control Center work the same way.

## With a paid Apple Developer membership: your own TestFlight

The **TestFlight** workflow signs, exports and uploads an App Store build.

1. Choose your own bundle ID (for example `com.example.surco`), register it and create
   the app record in App Store Connect.
2. In the fork's **Settings → Secrets and variables → Actions**, add the repository
   variable `SURCO_BUNDLE_ID` with that bundle ID, and the secrets
   `IOS_DISTRIBUTION_P12_BASE64`, `IOS_DISTRIBUTION_P12_PASSWORD`,
   `IOS_APP_STORE_PROFILE_BASE64`, `APPLE_TEAM_ID`, `APP_STORE_CONNECT_KEY_ID`,
   `APP_STORE_CONNECT_ISSUER_ID` and `APP_STORE_CONNECT_PRIVATE_KEY_BASE64`.
   [The TestFlight guide](testflight.md) explains how to make each one, from Linux too.
3. Run **TestFlight** with *upload* checked. Its preflight checks the secrets, the
   profile and the app record before compiling anything.

Without `SURCO_BUNDLE_ID` the workflow uses the project's own
`io.github.niiihuel.surco`, which only the project's team can upload.

## With a Mac and Xcode

With Xcode 26, the Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`, and
[limn](https://github.com/Niiihuel/limn) checked out beside this repository as
`../lienzo` (at the `LIMN_REF` of `.github/workflows/ios.yml`):

```sh
python3 scripts/build-ios.py simulator        # runs in the iOS Simulator
python3 scripts/build-ios.py device-unsigned  # target/ios/device-unsigned/artifacts/Surco-unsigned.ipa
```

Then sign and install the IPA with a sideloading tool as above. The Xcode project
does not link the Rust library by itself: `build-ios.py` passes it through the
generated `Rust.xcconfig`.
