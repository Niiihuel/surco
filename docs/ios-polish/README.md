# Surco iOS: finished playback and mobile interface

The user delegates implementation and product decisions, including reusable limn PRs. Preserve Surco/limn identity and the shared Rust engine/UI. The supplied Spotify mobile kit is interaction/layout evidence; use Surco content, fonts, icons and palette. Keep desktop behavior intact.

## Capability map and order

- `ios-media`: background/locked playback, system transport, Now Playing/Dynamic Island, interruptions and audio routes. Depends on the existing engine only.
- `mobile-primitives`: stable IME/focus, long press, action sheets, transitions and reduced motion. Generic behavior belongs in limn; media-specific behavior belongs in Surco.
- `mobile-screens`: Home, search, library, album/artist/playlist detail, player, queue, lyrics and settings. Depends on mobile-primitives and existing backend commands.
- `device-connections`: current real output and native Bluetooth/AirPlay route selection. Depends on ios-media and mobile-primitives.

Build ios-media and limn IME fixes first, in parallel. Then land reusable gestures/sheets and rebuild the mobile screens; connect the native route control to the finished device sheet. Validate each slice before combining it in a new TestFlight beta.

## Shared engineering contract

- Commands: `rtk nix-shell shell.nix --run 'rtk cargo test --workspace --locked'`; `rtk python3 -m unittest discover -s scripts -p 'test_*.py'`; `rtk git diff --check`. Native Objective-C/device packaging is verified by the iOS/TestFlight macOS workflows. limn uses its own workspace checks.
- Structure: app state/commands in `crates/app/src`; phone views under `view`; platform media bridge under `ios`; reusable components/adapters in the limn repository. Keep styles/components composed from existing limn tokens.
- Style: use explicit enums and small pure helpers for lifecycle/gestures; render with `Element::column(...).gap(...)` and semantic palette tokens. Avoid parallel app engines and SwiftUI screen rewrites.
- Tests: reproduce behavior bugs first; test state transitions, cancellation, focus retention and command delivery. Layout assertions at 320, 390 and 430 points; inspect actual rendered screenshots and native logs. Do not replace physical audio testing with a screenshot.
- Always: preserve working settings/queues/downloads, handle loading/error/empty states, use touch targets at least 44 points, keep native callbacks short, remove observers/targets on shutdown, keep credentials outside the repo, review before merge.
- Boundaries: no private iOS APIs, simulated devices, fake Spotify Connect/casting, new account requirements, microphone permission, or production App Store release. App-local Bluetooth pairing stays with system controls. Never drop failing checks or use ad-hoc focus/keyboard loops to hide symptoms.
- Completion: reviewed PRs, clean main/worktrees, passing meaningful checks, a signed TestFlight beta assigned to the existing internal group, and a concise record of device checks that still require the user.

## Reference

https://www.figma.com/design/h3crk4qRhavJSDKgFPxKWR/Spotify---Mobile-UI-Kit--Community-?node-id=0-1

Browser inspection confirmed Home (428×926, dark surface #111111), separate Home/Search/Library navigation, anchored compact player, search recents and an explicit searching/keyboard state. Inspect player, actions and Listening on before finalizing those screens. The Figma connector is not installed in this session; the file is readable in Chrome.
