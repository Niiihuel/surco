# Changelog

## 0.10.1

### Changed

- A song among the recent searches plays or pauses from its cover, shows its wave while it
  sounds, and has its options behind its dots. Clearing the recent searches is a word in
  the middle.

### Fixed

- A song's video fades in over its cover instead of replacing it at once.
- The right panel keeps its corners round over the song's video.
- The cross inside a page's search no longer draws a disc over the field.

## 0.10.0

![Home](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/home.jpg)

### Added

- Home is far fuller, and grows the more you listen: mixes made from the artists you play
  most, a card for the time of day, what you play again and again, picks inspired by what
  you heard lately, shelves for fans of an artist, more of another, news from the artists
  you follow, genres to explore and your own albums and artists.
- A local profile, from the account's menu: its face and name, which you can change, a
  picture of your own, your most played artists and songs, and your playlists.

![Profile](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/profile.jpg)

- Move to another device: your profile, playlists, likes, library, settings, equalizer
  and, if you want, what you played, to your phone or another computer on the same
  network. A phone's camera reads the code; another computer types the address.

![Move to another device](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/handoff.jpg)

- A playlist or your profile can have a picture of your own, chosen from your files.
- Search shows results as you type: the top result beside its songs, then artists and
  albums. What you open from it is remembered with its picture.
- A playlist's recommendations are drawn as Spotify's, and play the ten shown from the one
  you pick.

![Search](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/search.jpg)

### Changed

- The desktop is laid out as Spotify's: the top bar with home, search and browse, a
  library that folds into a rail of covers, and a right panel that, shut, leaves a strip
  that peeks at what plays and pushes the page aside under the pointer.
- The right panel is Spotify's Now Playing view: the song's video stands tall at its top
  and keeps playing behind the shut panel's strip, followed by its lyrics, the artist and
  what comes next.
- Surco draws its own icons, in the manner of Spotify's: thin strokes, filled only when
  something is on. A song is liked with a circled plus that fills with a check.
- Menus, tooltips and dialogs share one look, without outlines; menus scroll with the
  pages' bar, and a song's lists to add it to open beside its menu.
- A page's actions are larger, and a playlist's share, export and delete wait behind its
  dots. Shelves move by dragging or with the touchpad, and fade at their edge.
- Settings take the whole page, and the playback bar keeps Spotify's order.

![Settings](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/settings.jpg)

![A playlist and its menu](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.0/docs/releases/0.10.0/playlist.jpg)

### Fixed

- Sound follows the system's output on Windows, macOS and Linux when headphones or
  speakers come and go.
- Opening an older Surco after this one keeps your playlists and likes.

## 0.9.2

### Fixed

- On iOS, the import, new playlist and Listen on sheets open again.
- On the desktop, the recent searches stay open under the search field instead of
  flickering away when it is clicked.
- The desktop's settings are drawn at the size of its other pages.
- On iOS the player runs clear to the bottom, with no fade over its last card.
- While the phone searches, rows the shape of its results pulse instead of "Buscando…".

## 0.9.1

### Fixed

- On iOS, the lyrics' Translate button lights a circle when pressed, not a square.
- A sheet's rows on iOS light a faint pad with room around them when pressed.
- On iOS a song's own page no longer fades out at its bottom, and the fade behind the tab
  bar is darker.

## 0.9.0

### Added

- Settings are a list of their parts, each a page of its own, as in Spotify.
- A playlist ends in ten recommended songs, to play or add; Refresh brings the next ten.
- The lyrics can carry a translation into the app's language under each line.
- A first start offers to bring in your playlists from Spotify, Apple Music or YouTube
  Music.
- Release notes have a page of their own, every version's, with their pictures and videos.
- On the desktop, recent searches drop down from the search field.
- On iOS, a song is shared with the system's share sheet, previewed with its cover.

### Changed

- Covers that are on their way pulse where they will be, and the home page's picks show
  their shelves while they load.
- Browse all's genres each carry a record out of its sleeve.
- iOS sheets are translucent, size themselves to their content and have less text.
- On Windows the app has its own name and mark: in search, on the taskbar, where it can
  be pinned, and in its notifications, which no longer come from PowerShell.

### Fixed

- A long context menu no longer scrolls in a window with room for it, and the songs
  under it no longer light up as the pointer crosses it.
- A release's notes keep each wrapped item whole.

## 0.8.0

### Added

- On the desktop, album and artist cards with a play button that rises under the
  pointer. It plays without leaving the page.
- A page's play button and name stay at its top once you scroll past them, with the
  song table's column names pinned beneath.
- Music follows the system's output: connecting headphones or a speaker moves the song
  there, and unplugging the device in use carries on on the next one instead of
  stopping.

### Changed

- The seek and volume bars show their thumb only under the pointer.
- In a song table the number turns into play under the pointer. Rows are tighter and
  page titles larger.
- The library's filters are pills on the desktop too.
- The window's buttons line up with the top bar's icons, with a rule between them.

### Fixed

- A page's colour wash reaches the panel's edge instead of stopping at the scrollbar.

## 0.7.0

### Added

- The whole interface in Spanish and English, following the system's language by
  default, with a choice of language in Settings.
- On the desktop, a page to browse from beside the search field: recent searches
  that can be forgotten one by one, and genre cards.
- Thirty-five genres to browse on the phone, in vivid colours.

### Changed

- On Windows, macOS and Linux without Wayland the window's buttons sit in the top
  bar, on the side each system puts them, instead of in a row of their own.

### Fixed

- On iPhone, the search header collapses in sight as the keyboard rises instead
  of jumping.

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
