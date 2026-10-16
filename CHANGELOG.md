# Changelog

## 0.6.1

### Changed

- On the desktop, the library's filters use the pill track of Settings and scroll
  sideways when the panel is narrow.
- Every icon button's hover is round, and so is the lyrics card's button.
- The library search closes from a cross inside the field, as the main search does.

## 0.6.0

### Added

- An equaliser drawn as the response curve the audio actually applies, with a
  vertical slider per band, ready-made curves and the preamp in its own card; on
  iPhone it opens as a Settings page.
- A native iPhone header and tab bar with a soft blurred scroll edge, Liquid Glass
  buttons on iOS 26 and a system Paste control beside the import link.
- Unsigned IPA builds and a configurable bundle ID, so a fork can build and
  sideload Surco.
- The desktop announces a new version through the system's notifications.

### Changed

- Phone Settings grouped in monochrome inset cards with capsule buttons and
  shorter notes; red only for what cannot be undone.
- Lyrics follow the sung line with a spring a third down the screen, dimming and
  blurring by distance, with larger lines and previous/next beside Play.
- The player sheet opens, closes and follows a released drag with a spring; a
  fast flick dismisses it sooner than a slow pull.
- A search page after the iPhone kit: recent searches that can be forgotten one by
  one, bold section headings and genre cards.
- Rounded artist cards, true capsule controls and a slightly larger sung lyric.
- On the desktop: library filters as pills, round search fields, Settings in the
  phone's grouped cards, the phone's import card, and the lyrics card in the
  playing panel springing its lines as the phone's does.
- Long lists build only the rows around the view, and covers and sessions are
  reused, so scrolling and starting a song are quicker.

### Fixed

- The search bar keeps its size and position while the keyboard opens.
- Covers are fetched at a size iPhones and high-density screens draw sharply.

## 0.5.0

### Added

- A native iPhone search field with the system editing menu, clear and Cancel
  controls, stable keyboard focus and improved recent searches and result filters.
- Full-screen lyrics with the song's title, artist, seek, playback, sharing and
  audio-route controls, without the library tabs or mini player.

### Changed

- Continuous cover-colored iPhone backgrounds through the status bar, player and
  lyrics, with shorter header/tab fades and controls inside the system safe area.
- Lower compact navigation and a tinted floating mini player with independent
  route, like and playback actions.
- Home keeps quick access to eight recent items so listening shelves appear sooner.
- Larger system symbols and white circular Play/Pause controls across the mobile
  player, compact headers, mini player and detail pages.
- Rounded search fields and filter capsules using limn's existing round controls.
- A shorter import form with the link, supported services and explicit actions.

### Fixed

- Import uses the native text field editing menu for Paste; its card also accepts
  URL and Unicode text representations from other apps.
- The home-indicator inset masks scrolling content and blocks touches to hidden
  rows without covering the real transport and navigation controls.
- Search retains its results and scroll position while updating, and preserves
  native editing through rotation without reopening a dismissed keyboard.
- Song search keeps tracks whose album has no link, including collaborations,
  instead of rejecting the entire result list.
- Mixed song searches accept the provider's video rows while preserving their
  view counts, duration and artist credits.
- Search recovery shows a concise message and Retry without provider internals.

## 0.4.0

### Added

- Native iOS context menus on touch and hold, system playlist paste controls, and
  UIKit sheets with interactive dismissal, scrolling, keyboard handling and animation.
- SF Symbols from the iPhone system for navigation, transport and actions.
- Retryable lyrics errors and exact-video YouTube Music lyrics when the primary
  provider has no result or is unavailable; untimed text keeps its attribution.

### Fixed

- Successive horizontal shelf pans retain their position after touch handoff.
- Scroll-edge fades are visible outside the opaque header and bottom bars,
  including the tinted player header.
- Home cards offer their actions through touch and hold without overflow overlays.
- Transient lyrics failures no longer become permanent missing results; request
  deadlines, rate-limit cooldowns and a bounded successful-result cache apply.

## 0.3.0

### Added

- Native iOS launch screen using the Surco mark and matching dark background.
- Interactive player dismissal: pull down from the header or from content at its
  top, release early to return, or complete the gesture to close without pausing.
- Compact player and detail headers retain titles and playback controls while
  scrolling. The player includes current lyrics and available artist information.
- iOS Reduce Motion support, including changes made while the app is open.

### Changed

- Mobile content continues beneath stable headers and transport bars with edge
  fades, transient scroll indicators and reachable first and last rows.
- Page entrances leave navigation stable; Search's header moves when editing,
  and the player follows the finger even when its entrance is interrupted.
- Library's import card scrolls with the list.

### Fixed

- Changing tracks resets the lyrics viewport, including before the first timed
  line and when returning to a previously played song.
- Opening the mobile player requests matching lyrics and artist information.
- Covered songs cannot receive taps through the floating bars.

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
