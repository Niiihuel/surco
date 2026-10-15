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

## Reference evidence

Inspected the supplied Figma Community kit in Chrome, read-only (the Figma connector is available but was not connected). Frames use 428×926 and a dark #111111 surface. Relevant nodes:
- Search, [1:223](https://www.figma.com/design/h3crk4qRhavJSDKgFPxKWR/Spotify---Mobile-UI-Kit--Community-?node-id=1-223): distinct idle/editing states, recents, compact field with Cancel and no tabs above the keyboard.
- Full player, [12:1946](https://www.figma.com/design/h3crk4qRhavJSDKgFPxKWR/Spotify---Mobile-UI-Kit--Community-?node-id=12-1946): cover wash, large artwork, touch transport, device/queue access and no tab navigation.
- Listening on, [27:1596](https://www.figma.com/design/h3crk4qRhavJSDKgFPxKWR/Spotify---Mobile-UI-Kit--Community-?node-id=27-1596): bottom sheet, current route, native AirPlay/Bluetooth entry. Surco must not display Spotify Connect devices or group sessions it cannot implement.

Home and Library were also inspected: horizontal shelves, compact quick access, pill filters and cover rows. Use Surco's actual history, library and catalog; keep limn typography, palette, icons and motion rather than copying Spotify branding or demonstration content.
