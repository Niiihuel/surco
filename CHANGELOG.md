# Changelog

## 0.2.0

### Added

- An iPhone interface with a new Home, focused search with recent queries and
  filters, a full-screen player, and touch-sized library, album and artist pages.
- Hold a song or a library item to open its actions. The same actions are available
  from the visible options button, and sheets can be dismissed with a swipe.
- Native iOS playback sessions, lock-screen and Control Center metadata and
  controls, background audio, and recovery for interruptions and audio route changes.
- A device sheet with the current audio output and Apple's native AirPlay and
  Bluetooth output picker.
- iOS system clipboard support, with explicit Paste controls in search and playlist
  import. Clipboard reads happen only after a paste action.

### Changed

- Mobile settings, playlist forms, sharing, confirmations and the equalizer use
  scrollable sheets with accessible controls and fixed action buttons.
- Search keeps keyboard focus while results arrive, ignores stale responses, and
  adjusts the available space when the iOS keyboard changes height.
- Mobile motion, long presses and sheets use the reviewed limn framework changes
  from PRs #46, #47 and #48, preserving Surco's visual theme.

### Fixed

- Removing a queued song cannot remove a different occurrence if playback advances
  while its action sheet is open.
- Paused iOS audio outputs are released and rebuilt safely when playback resumes.
- Copied song links and playlist text reach the iPhone's system clipboard, and
  external links open in the system browser.

## 0.1.4

### Added

- While the app is open, a later version is looked for every 15 minutes and announced in a notice over the corner of the window, which leads to what it brings.

### Changed

- Built with limn 0.1.39.

## 0.1.3

### Added

- Spotify playlists longer than 100 tracks import whole. When Spotify's web player
  service cannot be reached, the first 100 still import.
- An import shows how many of its songs have been looked up, out of how many.
- After an import, the playlist lists the songs that were not found, ready to copy.
- The bell shows how many things it has not shown yet.
- The notice of a later version shows that release's notes, with their links and
  pictures. A required update shows a progress bar.

### Changed

- An imported song is searched for in up to three ways before it is given up, which
  finds recent releases the catalog did not return at first.

### Fixed

- The library's search field fills its row.
- The search field's cross button clears what was typed, and a pasted link no longer
  stays in the field after its list comes in.

## 0.1.2

### Added

- Import public playlists and albums from Spotify, YouTube Music and Apple Music
  through the library, settings or search bar. Spotify imports read up to 100 tracks.
- Copy a playlist as `Artist - Title` text for use in playlist transfer services.
- Clear the search field with its new cross button.
- Windows window controls in Surco's own title bar: drag, minimize, maximize,
  restore and close.

### Changed

- When playback is the only animation, reduce window drawing to about 30 frames
  per second. User interaction keeps the display's normal refresh rate.

### Fixed

- Public YouTube Music links import correctly, including album playlists whose
  response has no playlist header.
