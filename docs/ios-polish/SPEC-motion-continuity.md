# Mobile motion and continuity · 2026-10-15

The user confirmed that TestFlight 0.2.0 (3.1) plays correctly in the background
and with the iPhone locked. Remaining work is visual continuity, native-feeling
gestures, launch presentation and the reported lyrics scroll reset.

## Evidence

Eight supplied Spotify screenshots were extracted from `WhatsApp Unknown
2026-10-15 at 09.10.17.zip`, SHA-256
`f5638394f5d31ac7b8154c1a06156083a9a7170da1b3e0e3a7d051301fbafac6`.
REA inventory evidence `ev_7609ed23d3bbb1f7a439e61c1507305e40a93dac8a56c0ed5373670d72226596`
contains eight regular image resources; inspection evidence
`ev_49c3d220db7e62d2362e0072cc9c922414b6e3596834238b6cab52ac8fe2b770`.
All eight images were visually inspected. The user supplied three further player
screenshots and described downward drag dismissal as observed behavior.

Observed: content continues under fixed headers and the bottom tab bar; playlist
art/title condenses into a compact header with Play; Home starts with two-column
shortcuts then larger horizontal shelves. Player content scrolls from cover and
transport into lyrics/artist information, retaining compact transport above.
Static images do not establish exact animation curves, timing or blur algorithms.
Use Surco assets, existing palette and real backend data throughout.

## Acceptance and slices

1. Lyrics scope: a different current track resets the lyrics viewport, including
   before its first timed line and A→B→A. Ordinary same-track ticks preserve manual
   position; active-line following still works. The mobile player requests its
   own lyrics/artist data rather than depending on desktop panels.
2. Stable shell: persistent navigation/miniplayer do not replay page entrance
   animations. One transition per route/surface; no animation restarted by query
   typing or playback ticks. Header/footer edge fades keep controls legible and
   reserve enough final scroll space for every row to be reachable.
3. Scrolling detail and player headers reflect actual scroll offset. Home retains
   compact recent shortcuts and real recommendation shelves. No invented credits,
   biographies, statistics or actions.
4. Shared limn behavior: opt-in transient scroll indicators, and interactive
   downward dismissal with axis ownership, cancellation and distance/velocity
   completion. Header pulls dismiss; content pulls dismiss only when starting at
   its top. Sliders, text controls and horizontal shelves retain their gestures.
   Visual translation, pointer regions and clips stay aligned. Reduced motion
   removes autonomous travel/fades while preserving direct manipulation/feedback.
5. Launch: an OS-owned static dark screen with the existing Surco mark, followed
   immediately by the Rust UI; no artificial delay, duplicate splash or network gate.
6. Validate state/gesture regression tests, 320/390/430-point layouts and short
   keyboard viewports, rendered motion samples, independent review, native SDK
   builds and signed TestFlight delivery. Preserve working audio lifecycle.

## Primary guidance

- [Apple Motion](https://developer.apple.com/design/human-interface-guidelines/motion):
  purposeful, brief, interruptible motion that follows the gesture.
- [Apple Scroll views](https://developer.apple.com/design/human-interface-guidelines/scroll-views):
  transient indicators, expected scrolling gestures, legible floating bars and one
  coherent edge effect per view. Avoid nested vertical scrolling.
- [Apple Tab bars](https://developer.apple.com/design/human-interface-guidelines/tab-bars).

SwiftUI and React are references for behavior, not replacement implementations.
Rendering, commands and playback remain Rust/limn; only Apple platform services
use the existing native bridge.
