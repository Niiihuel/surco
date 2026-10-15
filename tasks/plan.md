# iOS polish implementation

Contract and capability index: [docs/ios-polish](../docs/ios-polish/README.md).

Completed implementation: baseline PR #1; limn stable focus #46, bottom sheets #47 and context-aware touch holds/focus dismissal #48; Surco native media PR #2. Mobile screens, captured library/track action targets, 44-point sheet controls, keyboard-aware Host layout and the real native route-picker slot are implemented in the UI branch. [UI PR #3](https://github.com/Niiihuel/surco/pull/3) remains open.

Intermediate native evidence: [iOS run 38709439039](https://github.com/Niiihuel/surco/actions/runs/38709439039) passed simulator startup and unsigned device compilation/linking at `dd9ef703d76ddde1eee29dc3737e89d3d7db9490`, including the Host bridge. Later closure changes are outside that run.

Remaining order:

1. Finish review and rendered phone/desktop QA of the final UI head, including library overflow, discovery/search intent, forms with the keyboard open and covered native controls.
2. Run final desktop/native checks for that head and merge UI PR #3 when reviewed and green. Do not use the intermediate run as final-head evidence.
3. Build/sign/upload the next TestFlight beta, assign the existing internal group, record its exact build and clean merged branches/worktrees.
4. Ask the user to perform the physical-device checklist in [ios.md](../docs/ios.md#build-and-validation): background/locked playback, Bluetooth/AirPlay, calls and interruption recovery, headset controls, Dynamic Island, keyboard and gestures. Record observed results separately from automated checks.
