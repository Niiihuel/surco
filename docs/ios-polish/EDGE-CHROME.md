# Continuous mobile chrome

References: the user's four iPhone screenshots, plus Figma file
`h3crk4qRhavJSDKgFPxKWR`: Home `1:346`, Album View `5:1634`, Track View `12:1946`.
Read through the Figma connector on 2026-10-15, with rendered design contexts.

- Paint the background across the full physical viewport, including the status
  bar and home indicator. Keep actual controls inside UIKit's current safe area;
  account for a docked keyboard once. Preserve canvas/native input coordinates.
- Use the cover's color continuously behind the detail header and hero. Playing
  and Lyrics retain that color to the bottom. Keep Surco colors, typography,
  catalog artwork and Apple SF Symbols; reference song/brand assets stay dynamic.
- Compact navigation chrome: 56-point header, 52-point tabs, 64-point floating
  mini player, 26-point tab symbols. Maintain 44-point control targets. Reference
  Home uses a 59-point mini player with cover, metadata, route and transport.
- Shorten scroll edge transitions to 20 points above and 24 below. They soften
  the overlap beside the bars without veiling an entire song row or hero.
- Give the mini player a cover-tinted surface, title/artist, route, like and play
  actions and a narrow progress line. Keep actions independent of opening it.
- Import uses the native text field's editing menu, selection and paste shortcuts;
  remove the redundant paste button from the sheet. The card's native paste control
  accepts both system URL and text representations, including UTF-16.
- Search uses a stable native UISearchBar with its own clear, Cancel and editing
  menu. Preserve the editor, caret and marked text through results and rotation;
  retain existing results and their scroll position during debounce/loading.

The second iOS kit `1BzqhExmVONm3WMDZBMqPj` supplies more precise 390-point
references: Search idle `604:9087`, focused recents `604:9150`, results `604:9171`,
scrolled player `620:22495`, Lyrics `574:5917` and playlist `605:21542`.

Verify short/tall and landscape phone geometry, scroll endpoints, keyboard and
rotation, native paste with text and URL payloads, and rendered screenshots.
Do not reproduce Figma's decorative status bar or home-indicator artwork: iOS owns
those. Existing desktop layouts and the Rust playback engine retain their behavior.
