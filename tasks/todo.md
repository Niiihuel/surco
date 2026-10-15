# iOS polish

## Delivered baseline · 0.2.0 (3.1)

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

## Motion iteration · 0.3.0 source

- [x] Implement source `ef6dbbb36c1fccfaec749072453226bfd7f86d9e` in the reviewed mobile iteration: stable scrolling chrome/fades, compact detail/player controls, interactive player dismissal, transient vertical/horizontal indicators and full-size Library import actions.
- [x] Fix lyrics viewport reset across track changes/A→B→A while preserving same-track manual scroll and active-line follow; let the mobile player request its own lyrics/artist data.
- [x] Add a static native Surco launch screen with matching opaque Rust background and no artificial delay.
- [x] Honor app and native iOS Reduce Motion for autonomous transitions while preserving direct manipulation.
- [x] Merge reusable [limn #51](https://github.com/Niiihuel/limn/pull/51), [#52](https://github.com/Niiihuel/limn/pull/52), [#53](https://github.com/Niiihuel/limn/pull/53) and [#54](https://github.com/Niiihuel/limn/pull/54); pin `f8cabafc7910e95ad800d63f76eb71490ab39ca4` in all workflows.
- [x] Pass `4c76716` and `ef6dbbb` local validation: 216 workspace tests, 3 existing network tests ignored, Clippy with warnings denied and 12 Python tests.
- [x] Record 18 candidate `acba1b1` preview captures with binary/fixture/image hashes and their limitations; candidate [desktop](https://github.com/Niiihuel/surco/actions/runs/38739629818)/[iOS](https://github.com/Niiihuel/surco/actions/runs/38739629874) CI passed. See [motion validation](../docs/ios-polish/MOTION-VALIDATION.md).
- [x] Pass `4c76716` [desktop CI 38742102688](https://github.com/Niiihuel/surco/actions/runs/38742102688) and [iOS CI 38742103117](https://github.com/Niiihuel/surco/actions/runs/38742103117).
- [x] Inspect three focused `4c76716` recaptures and real normal/reduced software-rendered clips, retaining hashes and monotonic timestamps. The old 430-point window capture had a capped 390-point canvas; only layout tests establish 430-point coverage.
- [x] Fix the fourth simulator finding, empty-Home text visible behind tabs, with an opaque navigation background in `ef6dbbb`; verify four separate Linux recaptures for empty Home at 390×780/844, final scroll position and miniplayer state, retaining exact hashes.
- [x] Pass `ef6dbbb` [desktop CI 38744477519](https://github.com/Niiihuel/surco/actions/runs/38744477519) and [iOS CI 38744477042](https://github.com/Niiihuel/surco/actions/runs/38744477042); approve the final simulator screenshot and eight-second no-crash launch smoke check.
- [x] Merge [PR #11](https://github.com/Niiihuel/surco/pull/11) to `main` as `7e76a02a7ffd3552246ce6d3b13399c4b12b1373`; verify its tree matches tested `ef6dbbb`.
- [x] Pass main [desktop CI 38746377744](https://github.com/Niiihuel/surco/actions/runs/38746377744) on Linux, macOS and Windows.
- [x] Validate [CI-only PR #13](https://github.com/Niiihuel/surco/pull/13) at `5420763`: [iOS 38755079773](https://github.com/Niiihuel/surco/actions/runs/38755079773) passed simulator/device-unsigned and [desktop 38755079488](https://github.com/Niiihuel/surco/actions/runs/38755079488) passed all platforms. Keep 14 Python tests, successful capture return, both liveness probes and smoke required. Preserve both original main install timeouts, `65561cf` readiness failure and `940a4b1` incomplete capture; their cause is not inferred from the final success.
- [x] Accept final native artifacts (clean PNG 1206×2622, SDK 26.5/Xcode 26.6, screenshot exit 0, both probes, smoke PID 14188/eight seconds and no crash reports) and merge PR #13 as `50b692d11236c14aaea0755b7e23f0aa51610235`, 15:08:59 UTC.
- [x] Complete [signing/upload run 38746396955](https://github.com/Niiihuel/surco/actions/runs/38746396955) from `main` source `7e76a02`; verify downloaded IPA 0.3.0 (4.1), signing/profile/background audio/LaunchScreen and push tag `ios-v0.3.0-build4.1`.
- [x] Verify Apple processing **Finalizado**, Spanish test notes and standard-crypto/outside-France answers; assign 0.3.0 (4.1) only to existing **Surco interno** (3 testers), confirming **En pruebas**/90 days at 14:08 UTC. Build UUID `5f142d11-cc15-4b39-a55e-cbbf81cb91a3`; see [delivery](../docs/testflight.md).
- [x] Preserve QA at `/home/nihuel/Downloads/Surco-UI-QA-2026-10-15/` with verified checksums, copy manifest, source provenance, `delivery-0.3.0-4.1/` records and explicitly failed modern-CI archives indexed by `ci-modern-failed-attempts.json`. Retire only scoped inactive work, including delivered motion, with cleanup execution reports and rescue bundles outside the repos; preserve docs/CI-toolchain and unrelated work.
- [ ] Validate 0.3.0 player gestures, cancelled/interrupted motion, keyboard/safe areas and the native Reduce Motion switch on a physical iPhone; recheck playback and the remaining hardware checklist.

- [x] Document the final scoped docs/CI cleanup procedure and supplemental rescue backup. After both merges, external `cleanup-final-execution.json` is the definitive record of actual backup/removal operations.
