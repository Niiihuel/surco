# iOS with Rust and limn

Surco's iOS target uses the same Rust application, playback engine and limn
components as the desktop app. `crates/app/src/lib.rs` is the shared entry point;
the desktop executable calls `run()`, and `ios/main.m` forwards the main thread
to the exported `surco_start()` function. The Objective-C file boots winit and
resolves the app's private storage directory through Foundation.
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

The phone layout has Home/Search/Library navigation, album/artist/playlist detail,
mini-player, full player, lyrics, queue and spacious settings. It uses the existing
commands, collection and playback state. Search debounces typing, rejects stale
answers and records explicit submissions. Opening the Search tab waits for a tap
before requesting the keyboard; Home's discovery action enters editing directly.

Track and library options are reachable by a visible overflow button or a stationary
hold. Moving, scrolling or cancelling the gesture cancels the hold; releasing a
consumed hold does not play the row. Library options work in list, compact and grid
views, capture the stable item key/title/artwork, and open, pin/unpin or remove the
selected album/artist without relying on its old list index. Track menus retain the
selected track; removing a queue occurrence checks its captured queue first.

The redesigned navigation, transport, overflow, sheet actions, form fields and EQ
sliders use targets of at least 44 points. Playlist/import/share/confirmation
forms and the equalizer use limn bottom sheets: the body scrolls while close and
footer actions remain reachable above the keyboard. The Host reads native keyboard
geometry, respects safe areas and egui scale, avoids counting a keyboard resize
twice, and clears focus on explicit dismissal. Floating keyboards do not resize the
entire app. Reusable focus, sheets and gesture behavior comes from merged limn
[PR #46](https://github.com/Niiihuel/limn/pull/46),
[#47](https://github.com/Niiihuel/limn/pull/47) and
[#48](https://github.com/Niiihuel/limn/pull/48).

On a desktop, `cargo run -p surco -- --mobile` previews this layout. Existing
`--click`, `--slide` and `--screenshot` arguments still work for visual checks.

On iOS, CPAL exposes the system's current audio route rather than a separate
device for every Bluetooth headphone. The player, mini-player and settings open a
connections sheet with the current route's real name and a visible native
AVRoutePickerView. iOS owns the destination list; pair new headphones/speakers in
iOS Settings first, then use the system route control. There is no simulated device
list, private picker invocation, fake AVPlayer or Bluetooth scan. See
[the native keyboard/route bridge](ios-ui-native.md). The desktop output picker
remains unchanged. Desktop executable self-updates are disabled on iOS, where
application bundles must be signed.

## System playback

`ios/SurcoMedia.m` bridges the existing Rust backend to AVAudioSession and Media
Player. It configures `Playback`, `Default`, and `LongFormAudio`, and activates
the session on a play request. Opening Surco alone does not open its audio device.
Bluetooth A2DP follows the system route without microphone permission or hands-free
mode. The Rust decoder, queue, crossfade and equalizer remain the playback engine.

Pausing releases the iOS output device. Cancelling readers lets a decoder leave a
network wait promptly without invalidating the downloaded buffer. Resume and route
recovery reload from the last reported position, reusing that buffer when available.
Interruptions pause playback; automatic resume requires Apple's `shouldResume`, the
same track, and no later transport decision. Disconnecting headphones pauses;
resetting media services rebuilds configuration and waits for a new play request.
An explicit Play retries session activation even when iOS did not send the matching
interruption-ended notification; automatic resume still requires the conditions above.

Now Playing metadata and remote play/pause/next/previous/seek commands run through
the backend's existing `MediaKeys` boundary, including while the window sleeps.
Artwork loads asynchronously and is checked against the current track and URL.
iOS extrapolates progress between meaningful metadata updates. The bridge publishes
the data used by the system's Lock Screen, Control Center and supported Dynamic
Island presentation; iOS controls that presentation. No ActivityKit extension or
duplicate Swift interface is needed. Native observers and remote-command targets
are removed when the integration shuts down. These hardware behaviors still need
the physical-device checks listed below.

Sources: [Apple's playback configuration](https://developer.apple.com/documentation/avfoundation/configuring-your-app-for-media-playback),
[interruption handling](https://developer.apple.com/documentation/avfaudio/handling-audio-interruptions),
[route changes](https://developer.apple.com/documentation/avfaudio/responding-to-audio-route-changes),
[Now Playing](https://developer.apple.com/documentation/mediaplayer/mpnowplayinginfocenter),
and [system services in Dynamic Island](https://developer.apple.com/news/?id=qpqf1gru).

Settings, collections and player preferences use `Library/Application Support/surco/app.ron`
inside the current iOS app container. Offline audio and its index live in the
adjacent `offline` directory, which is excluded from backups and survives normal
app restarts. Foundation resolves the container each launch; no device-specific
container path is persisted. This follows Apple's guidance for
[application-support files and downloaded media](https://developer.apple.com/documentation/foundation/using-the-file-system-effectively).
The iOS path is supplied explicitly because eframe 0.36.2 does not provide a
default storage directory for iOS. Desktop paths stay unchanged.

## Build and validation

Playback is merged in [Surco PR #2](https://github.com/Niiihuel/surco/pull/2),
and the mobile UI/native Host in [PR #3](https://github.com/Niiihuel/surco/pull/3).
The UI code head `16bd78e21c67a2563ff6999eb7030ef9303ff502` passed
[Linux/macOS/Windows CI](https://github.com/Niiihuel/surco/actions/runs/38711825798)
and [iOS simulator/device CI](https://github.com/Niiihuel/surco/actions/runs/38711825772)
before merging to `main` as `77ecb0d6ec9aaed61fbdfb4db7528b78d0b5a7df`.
The subsequent [clipboard/link fix PR #4](https://github.com/Niiihuel/surco/pull/4)
adds explicit system copy/paste and enables public UIKit URL opening. Its code
head `787764e5d4aed7b899fdadf8d4629e30d57a7ab7` passed
[iOS simulator/device CI](https://github.com/Niiihuel/surco/actions/runs/38717476331)
and [Linux/macOS/Windows CI](https://github.com/Niiihuel/surco/actions/runs/38717476437).
It merged into `main` as `e5c0e8df3a2a2ee356f104d261b4f97aca77c54b`.
Local validation passed 189 Rust tests (3 existing network tests ignored),
9 packaging/signing-script tests and workspace/all-target Clippy with warnings
denied. Rendered phone and desktop screens were inspected, including the final
native simulator screenshot. See [TestFlight delivery](testflight.md) for the
signed build. On 2026-10-15 the user confirmed that background and locked-screen
playback work correctly on their iPhone with 0.2.0 (3.1). The other hardware
checks below remain pending; automated checks do not establish them.

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

The unsigned application cannot be installed directly on an iPhone. The manual
**TestFlight** workflow signs with a valid distribution identity (`iPhone
Distribution` in the current setup; `Apple Distribution` is also supported) and
an App Store Connect profile. Follow [the account setup and installation guide](testflight.md)
to register Surco, configure GitHub Secrets and install from TestFlight.
Simulator startup does not establish physical audio behavior. After installing a
signed build on an iPhone, validate:

- Open Surco while another app plays; its audio should continue until Surco plays.
- Play streamed and downloaded tracks, background the app, lock the screen, and
  let playback cross a track boundary. Confirm title, cover and elapsed time.
- Use Lock Screen/Control Center and headset play/pause, previous/next and seek.
  On a supported iPhone, inspect the system's expanded Dynamic Island controls.
- Receive/decline a call while playing, and repeat while already paused. A manual
  pause or different track during an interruption must cancel automatic resume.
- Open the connections sheet from player, mini-player and settings. Select paired
  Bluetooth headphones/speakers or an available AirPlay destination using the native
  button; also check route changes made in Control Center.
  Disconnect headphones: playback should pause and stay paused on the speaker.
- Type, paste, transfer focus and dismiss the keyboard in search, playlist creation
  and import. Check controls with keyboard visible, rotation, long titles, sheet
  dragging, stationary holds and scrolling without accidental playback. Cover the
  connections sheet with another modal and confirm its native button cannot intercept taps.
- Copy a song link or playlist text into another app. Use the visible Paste buttons
  to bring a public link into search/import, including with the keyboard open and
  the import body scrolled. Check iOS paste permission without losing existing text.
- Use iOS Settings > Developer > Reset Media Services: playback must stay paused
  until requested and then recover. No microphone access prompt should appear.

The iOS/TestFlight macOS workflow verifies native Objective-C compilation and
linking; Rust state tests and simulator screenshots do not replace these checks.
