# mobile-screens

Objective: a cohesive, polished phone music app using limn's visual identity and the useful Spotify interaction patterns. Shared commands, style, structure and boundaries: [contract](README.md).

Acceptance:
- Distinct Home and Search headers; search idle/typing/loading/results/error states, recents and useful result grouping. Keyboard remains stable, dismisses deliberately and never overlaps required controls.
- Cohesive Home/Library/detail navigation, compact player above the bottom navigation and an immersive full player with large artwork, touch transport, seek/time, queue, lyrics and device access.
- Track/album/playlist actions are reachable by hold and explicit overflow; actions invoke real current commands (play/next/queue/save/download/playlist/artist/share when applicable).
- Settings are spacious with real options, grouped hierarchy and readable descriptions. Existing library sort/view/pins, quality, EQ, crossfade and downloads remain usable.
- Use a consistent spacing/type/control scale, safe areas and long-text truncation. Avoid oversized repeated cards, fake features and unrelated gradients. Animate navigation/player/sheets using limn primitives.
- Verify layout and interaction states at 320/390/430 points and desktop regression screenshots; native simulator/device compilation; ship a signed beta for physical keyboard/gesture verification.

Files: phone view modules, state/action handling and small shared view helpers. Keep platform media hooks separate from screen rendering.
