# iOS polish

- [x] Merge baseline PR #1 after final checks; clean merged branches.
- [x] limn stable IME/focus regression and reviewed PR #46.
- [x] Merge limn bottom sheets #47 and context-aware touch holds/focus dismissal #48.
- [x] Implement native playback session, interruption/route recovery, system commands and Now Playing; merge Surco audio PR #2.
- [x] Implement mobile Home/Search/Library/details/player/queue/lyrics/settings with shared Rust commands, explicit overflow/holds, captured action targets and touch-sized form sheets.
- [x] Integrate Host keyboard geometry/focus intent, real current-route device sheet and visible native route picker.
- [x] Intermediate iOS simulator/device-unsigned checks pass in [run 38709439039](https://github.com/Niiihuel/surco/actions/runs/38709439039), including Host integration at `dd9ef703d76ddde1eee29dc3737e89d3d7db9490`.
- [ ] Complete final-head rendered visual QA, code review and desktop/native CI after closure changes.
- [ ] Merge [Surco UI PR #3](https://github.com/Niiihuel/surco/pull/3) after final checks.
- [ ] Signed next TestFlight beta, existing tester group, and clean repositories.
- [ ] User checks on physical iPhone: background/locked audio, Bluetooth/AirPlay, calls/interruption recovery, headset commands, Dynamic Island and keyboard/gestures.
