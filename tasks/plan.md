# iOS polish implementation

Contract and capability index: [docs/ios-polish](../docs/ios-polish/README.md).

Completed implementation: baseline PR #1; limn stable focus #46, bottom sheets #47 and context-aware touch holds/focus dismissal #48; Surco native media PR #2 and mobile UI [PR #3](https://github.com/Niiihuel/surco/pull/3). Mobile screens, captured library/track action targets, 44-point sheet controls, keyboard-aware Host layout and the real native route-picker slot are merged into `main`.

Final code evidence: [iOS run 38711825772](https://github.com/Niiihuel/surco/actions/runs/38711825772) passed simulator startup and unsigned device compilation/linking for code head `16bd78e21c67a2563ff6999eb7030ef9303ff502`. [Desktop CI](https://github.com/Niiihuel/surco/actions/runs/38711825798) passed Linux/macOS/Windows. Independent review and rendered phone/desktop QA are complete. PR #3 merged as `77ecb0d6ec9aaed61fbdfb4db7528b78d0b5a7df`.

The final native clipboard/link fix is merged in [PR #4](https://github.com/Niiihuel/surco/pull/4). Reviewed code head `787764e5d4aed7b899fdadf8d4629e30d57a7ab7` passed [iOS CI](https://github.com/Niiihuel/surco/actions/runs/38717476331), [desktop CI](https://github.com/Niiihuel/surco/actions/runs/38717476437), local tests/Clippy and rendered Paste-control QA. Its `main` merge `e5c0e8df3a2a2ee356f104d261b4f97aca77c54b` is the source of the next signed beta.

Remaining order:

1. Finish the signed 0.2.0 TestFlight delivery from reviewed `main`, assign the existing internal group, record the exact build and clean merged branches/worktrees.
2. User performs the physical-device checklist in [ios.md](../docs/ios.md#build-and-validation): background/locked playback, Bluetooth/AirPlay, calls and interruption recovery, headset controls, Dynamic Island, keyboard, gestures and cross-app Copy/Paste. Record observed results separately from automated checks.
