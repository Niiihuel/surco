# iOS with Rust and limn

Surco's iOS target uses the same Rust application, playback engine and limn
components as the desktop app. `crates/app/src/lib.rs` is the shared entry point;
the desktop executable calls `run()`, and `ios/main.m` forwards the main thread
to the exported `surco_start()` function. The Objective-C file only boots winit.
There are no duplicate Swift, React Native or web-view screens.

## Why this route

limn's renderer already uses eframe 0.36.2 with wgpu and default features disabled.
[That exact eframe release checks iOS in its own CI](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/rust.yml#L165).
The local limn 0.1.43 renderer also passes `cargo check` for `aarch64-apple-ios`.
No framework fork or unpublished sibling changes are required.

The older sibling project `dnmusic` was inspected as a reference. Its screens
combine Expo/React Native with some SwiftUI/UIKit components. Its mini-player
and audio-route integration are useful references, but its complete navigation,
settings and player cannot be moved as an independent Swift UI without a rewrite.
Keeping the rendering in limn preserves Surco's visual language and avoids
maintaining two application interfaces.

## Phone interface

The phone layout has bottom navigation, full-width pages, a searchable catalog,
library, mini-player, full player, lyrics, queue and settings. It uses the existing
commands and playback state. Touch targets are at least 44 logical pixels for the
main navigation and transport. Host input adapts finger scrolling without
turning a pan into a row click, and advertises focused fields to the platform
keyboard.

On a desktop, `cargo run -p surco -- --mobile` previews this layout. Existing
`--click`, `--slide` and `--screenshot` arguments still work for visual checks.

On iOS, CPAL exposes the system's current audio route rather than a separate
device for every Bluetooth headphone. Pair in iOS Settings and choose the route
in Control Center. An in-app native route picker is not part of this first slice.
The desktop output picker remains unchanged. Desktop executable self-updates
are disabled on iOS, where application bundles must be signed.

## Build and validation

The **iOS** GitHub Actions workflow uses macOS, Xcode and the pinned limn revision.
It builds the same Rust static library and links a minimal Xcode application for:

- `aarch64-apple-ios-sim`: installs on a temporary iPhone simulator, checks that
  the launched process survives startup, and saves a screenshot and logs.
- `aarch64-apple-ios`: builds and links an unsigned device application.

The script obtains the SDK with `xcrun` and the actual native linker flags from
`rustc --print native-static-libs`. Both Rust and Xcode target iOS 15 or later.
The setup follows [Rust's iOS target requirements](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
and [winit's main-thread static-library integration](https://docs.rs/winit/0.30.13/winit/platform/ios/).

On a Mac with Xcode and the Rust targets installed:

```sh
python3 scripts/build-ios.py simulator
python3 scripts/build-ios.py device-unsigned
```

Actions publishes `ios-simulator` and `ios-device-unsigned`. Each includes an
application tarball preserving executable permissions, build metadata and logs.
The simulator artifact also includes `simulator.png` and its startup result.

The unsigned application cannot be installed directly on an iPhone. Signing and
provisioning with an Apple development identity are required for physical-device
installation or TestFlight. Simulator startup does not establish Bluetooth audio,
background playback, lock-screen controls, interruption handling or performance
on a physical iPhone. Those remain device-validation work; the background-audio
bundle declaration alone is not proof that they work.
