# Surco iOS: playback and mobile interface

The user delegates implementation and product decisions, including reusable limn PRs. Preserve Surco/limn identity and the shared Rust engine/UI. The supplied Spotify mobile kit is interaction/layout evidence; use Surco content, fonts, icons and palette. Keep desktop behavior intact.

## Capability map and order

- `ios-media`: background/locked playback, system transport, Now Playing/Dynamic Island, interruptions and audio routes. Depends on the existing engine only.
- `mobile-primitives`: stable IME/focus, long press, action sheets, transitions and reduced motion. Generic behavior belongs in limn; media-specific behavior belongs in Surco.
- `mobile-screens`: Home, search, library, album/artist/playlist detail, player, queue, lyrics and settings. Depends on mobile-primitives and existing backend commands.
- `device-connections`: current real output and native Bluetooth/AirPlay route selection. Depends on ios-media and mobile-primitives.

Build ios-media and limn IME fixes first, in parallel. Then land reusable gestures/sheets and rebuild the mobile screens; connect the native route control to the finished device sheet. Validate each slice before combining it in a new TestFlight beta.

## Implementation status · 2026-10-15

- Playback integration is merged in [Surco PR #2](https://github.com/Niiihuel/surco/pull/2): AVAudioSession activation, interruption/route recovery, system commands and asynchronous Now Playing artwork, with the existing Rust engine.
- Reusable framework changes are merged: [limn #46](https://github.com/Niiihuel/limn/pull/46) keeps platform text focus stable, [#47](https://github.com/Niiihuel/limn/pull/47) adds controlled bottom sheets, and [#48](https://github.com/Niiihuel/limn/pull/48) adds context-aware touch holds and focus dismissal.
- [Surco UI PR #3](https://github.com/Niiihuel/surco/pull/3) is merged. Its implementation includes Home/Search/Library, detail pages, player/queue/lyrics/settings, debounced search and explicit keyboard intent, real command actions, 44-point controls, and scrollable form sheets with fixed action footers. Library list/compact/grid overflow and holds share a sheet with a captured stable key; track contexts preserve the selected track, and queue removal rejects a changed queue.
- The iOS Host integrates native keyboard geometry and the visible AVRoutePickerView slot. The connections sheet shows the real current output and points initial Bluetooth pairing to iOS Settings. Native controls hide when covered by another modal. See [native UI integration](../ios-ui-native.md).
- UI code head `16bd78e21c67a2563ff6999eb7030ef9303ff502` passed independent review, rendered phone/desktop QA, [iOS simulator/device CI](https://github.com/Niiihuel/surco/actions/runs/38711825772) and [Linux/macOS/Windows CI](https://github.com/Niiihuel/surco/actions/runs/38711825798). It merged into `main` as `77ecb0d6ec9aaed61fbdfb4db7528b78d0b5a7df`.
- [Clipboard/link PR #4](https://github.com/Niiihuel/surco/pull/4) adds explicit system copy/paste and public UIKit URL opening. Reviewed head `787764e5d4aed7b899fdadf8d4629e30d57a7ab7` passed [native CI](https://github.com/Niiihuel/surco/actions/runs/38717476331), [desktop CI](https://github.com/Niiihuel/surco/actions/runs/38717476437), 189 local workspace tests (3 existing network tests ignored), Clippy with warnings denied and rendered Paste-control QA. It merged as `e5c0e8df3a2a2ee356f104d261b4f97aca77c54b`.
- Signed **0.2.0 (3.1)** from that merge is processed and **En pruebas** for the existing **Surco interno** group. Source tag `ios-v0.2.0-build3.1`, exact workflow/IPA validation and update instructions are in [TestFlight delivery](../testflight.md). Merged feature branches/worktrees are cleaned.

On 2026-10-15 the user confirmed that background and locked-screen playback work correctly on their iPhone with TestFlight 0.2.0 (3.1). Bluetooth/AirPlay switching, interruption/call recovery, headset commands and Dynamic Island presentation still require physical checks. The signed beta and tester assignment are recorded separately in [TestFlight delivery](../testflight.md). The device checklist is in [ios.md](../ios.md#build-and-validation).

## Shared engineering contract

- Commands: `rtk nix-shell shell.nix --run 'rtk cargo test --workspace --locked'`; `rtk python3 -m unittest discover -s scripts -p 'test_*.py'`; `rtk git diff --check`. Native Objective-C/device packaging is verified by the iOS/TestFlight macOS workflows. limn uses its own workspace checks.
- Structure: app state/commands in `crates/app/src`; phone views under `view`; platform media bridge under `ios`; reusable components/adapters in the limn repository. Keep styles/components composed from existing limn tokens.
- Style: use explicit enums and small pure helpers for lifecycle/gestures; render with `Element::column(...).gap(...)` and semantic palette tokens. Avoid parallel app engines and SwiftUI screen rewrites.
- Tests: reproduce behavior bugs first; test state transitions, cancellation, focus retention and command delivery. Layout assertions at 320, 390 and 430 points; inspect actual rendered screenshots and native logs. Do not replace physical audio testing with a screenshot.
- Always: preserve working settings/queues/downloads, handle loading/error/empty states, use touch targets at least 44 points, keep native callbacks short, remove observers/targets on shutdown, keep credentials outside the repo, review before merge.
- Retained identity: simultaneous controls need unique stable IDs, including a Home discovery CTA and a Search tab that dispatch related commands. Validate hit-region IDs as well as accessibility nodes, which can deduplicate collisions.
- Boundaries: no private iOS APIs, simulated devices, fake Spotify Connect/casting, new account requirements, microphone permission, or production App Store release. App-local Bluetooth pairing stays with system controls. Never drop failing checks or use ad-hoc focus/keyboard loops to hide symptoms.
- Completion: reviewed PRs, clean main/worktrees, passing meaningful checks, a signed TestFlight beta assigned to the existing internal group, and a concise record of device checks that still require the user.

## Reference

https://www.figma.com/design/h3crk4qRhavJSDKgFPxKWR/Spotify---Mobile-UI-Kit--Community-?node-id=0-1

Read-only Chrome inspection covered Home/Library, Search (428×926, dark surface #111111), full player, action patterns and Listening on. The implementation uses Surco's real content and existing limn tokens. Specific frame links and interaction decisions are recorded in [SPEC-mobile-screens.md](SPEC-mobile-screens.md).
