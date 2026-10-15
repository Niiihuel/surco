# iOS polish

- [x] Merge baseline PR #1 after final checks; clean merged branches.
- [x] limn stable IME/focus regression and reviewed PR #46.
- [x] Merge limn bottom sheets #47 and context-aware touch holds/focus dismissal #48.
- [x] Implement native playback session, interruption/route recovery, system commands and Now Playing; merge Surco audio PR #2.
- [x] Implement mobile Home/Search/Library/details/player/queue/lyrics/settings with shared Rust commands, explicit overflow/holds, captured action targets and touch-sized form sheets.
- [x] Integrate Host keyboard geometry/focus intent, real current-route device sheet and visible native route picker.
- [x] Intermediate iOS simulator/device-unsigned checks pass in [run 38709439039](https://github.com/Niiihuel/surco/actions/runs/38709439039), including Host integration at `dd9ef703d76ddde1eee29dc3737e89d3d7db9490`.
- [x] Complete final-head rendered visual QA and independent code review; [desktop CI](https://github.com/Niiihuel/surco/actions/runs/38711825798) and [iOS CI](https://github.com/Niiihuel/surco/actions/runs/38711825772) pass for `16bd78e`.
- [x] Merge [Surco UI PR #3](https://github.com/Niiihuel/surco/pull/3) after final checks (`main` merge `77ecb0d`).
- [x] Review, validate and merge [iOS clipboard/link PR #4](https://github.com/Niiihuel/surco/pull/4): system Copy/Paste, visible Paste controls and public URL opening; 189 local tests, Clippy and [desktop](https://github.com/Niiihuel/surco/actions/runs/38717476437)/[native](https://github.com/Niiihuel/surco/actions/runs/38717476331) CI pass at `787764e`; merge `e5c0e8d`.
- [x] Deliver signed [TestFlight 0.2.0 (3.1)](../docs/testflight.md) from `e5c0e8d`, verify **En pruebas** for the existing three-member **Surco interno** group, preserve tag `ios-v0.2.0-build3.1` and clean merged feature branches/worktrees.
- [x] User confirmed background and locked-screen playback on physical iPhone with TestFlight 0.2.0 (3.1), 2026-10-15.
- [ ] Remaining physical iPhone checks: Bluetooth/AirPlay, calls/interruption recovery, headset commands, Dynamic Island, keyboard/gestures and cross-app Copy/Paste.
