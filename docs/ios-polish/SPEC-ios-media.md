# ios-media

Objective: music continues with the app backgrounded or the screen locked, and iOS exposes accurate media controls and Now Playing/Dynamic Island. Keep the Rust engine and current MediaKeys/backend boundary. Shared commands, style, structure and boundaries: [contract](README.md).

Acceptance:
- Configure AVAudioSession Playback/Default/LongFormAudio and activate on an actual play request, reporting errors. Do not request microphone permission or hands-free Bluetooth mode.
- Publish title, artist, album, artwork, duration, elapsed position and playback rate; clear stale metadata. Use the system's extrapolation and avoid a foreground UI timer dependency.
- Handle play/pause/toggle/previous/next/seek through existing backend commands; unregister targets safely.
- Pause on old-device-unavailable; preserve intent through interruption and resume only with Apple's shouldResume and no intervening user decision. Recover output after media-server reset without unsolicited playback.
- Test interruption/route intent and metadata diffing as pure Rust state, plus native compilation/linking. Document hardware tests for locked/background playback, remote controls, calls, route changes and system Now Playing.

Files: `ios/SurcoMedia.m`, Xcode project, `ios_media.rs`, `media_keys.rs`, backend and module declaration. Native integration is app-specific; do not put Apple dependencies in limn.
