# Changelog

## 0.14.2

### Changed

- Replay's rankings play from their covers, as every list's rows do: under the pointer a
  cover shows play or pause, and the song that plays wears its own sound there.
- Text fields can be seen before they are used: a faint outline at rest, brighter under the
  pointer, whole while typing in them.

## 0.14.1

![Replay's story, its songs to play](https://raw.githubusercontent.com/Niiihuel/surco/v0.14.1/docs/releases/0.14.1/story-songs.jpg)

### Added

- Replay's story plays each song's chorus, as Wrapped does: where the line sung most is
  first sung, found from its synced lyrics. In its lists each song plays, or pauses, from
  its row.
- The story goes on by itself or waits: pause it from beside its close, or with Space.
  When it ends, what you were listening to comes back where it was.

### Changed

- Listen together is where the music is: at the top of the queue, beside the devices on
  a desktop and in a phone's sheet of them, with the room's faces once you are in one.
- Last.fm and ListenBrainz have a part of their own in Settings, Connected apps, rather
  than sitting under Library.

## 0.14.0

![Replay told as a story](https://raw.githubusercontent.com/Niiihuel/surco/v0.14.0/docs/releases/0.14.0/story.jpg)

### Added

- Replay as Wrapped: cards in coal, cream, lavender and yellow, the minutes huge, the songs
  and artists ranked large with framed covers, the artist heard most under stacked rings,
  a card that keeps the tops, and the highlights each a moment of its own, for the month,
  the year or all time.
- Watch your Replay: told as a story, a step at a time, each with a bar that fills while it
  is on screen and goes on by itself; the minutes count up and the rankings arrive one by
  one. A wide screen of its own on a desktop, the whole screen on a phone; a tap, the
  arrows or the keyboard go on and back, Escape leaves.
- When in the day is a stacked area with a gradient: the artists heard most each a band of
  their colour, everyone else on top.

![A field whose name rises onto its outline](https://raw.githubusercontent.com/Niiihuel/surco/v0.14.0/docs/releases/0.14.0/fields.jpg)

### Changed

- Every text field is a pill named by a label that rests inside it and rises onto its
  outline once it is used.
- Accepting, cancelling and the rest are the cards' own buttons: a white pill for what goes
  ahead, a clear one that turns white under the pointer for the rest.

### Fixed

- Replay's periods change without the page flickering, and its covers are fetched when it
  opens.
- Replay and the codes to move an account or join a room are set in the interface's own
  faces rather than a monospaced one.

## 0.13.0

![Replay: what you listened to, summed up](https://raw.githubusercontent.com/Niiihuel/surco/v0.13.0/docs/releases/0.13.0/replay.jpg)

### Added

- The app opens where it was closed: the song paused at its second and the queue after it,
  nothing fetched until it is played.
- A sleep timer, from the bar's moon on a desktop and the player on a phone: 15 to 90
  minutes or the end of the song, the volume easing away before it stops.
- SponsorBlock skips what is not music in a video (talk, intros, sponsors, like
  reminders), each category a switch in Settings; only four characters of the video's hash
  are sent.
- Skip silence, in Settings › Audio quality: the silence a song opens and closes with is
  passed by, lyrics and video keeping the song's own time.
- Replay, from the account's menu and the profile: minutes, plays, the songs and artists
  heard most, when in the day, the longest run of days and the song on repeat, for the
  month, the year or all time. A song counts after 30 seconds really heard.
- Scrobbling to Last.fm and ListenBrainz, in Settings › Library. The Last.fm password is
  used once and not kept; what could not be sent waits for the next time.

![Listen together](https://raw.githubusercontent.com/Niiihuel/surco/v0.13.0/docs/releases/0.13.0/together.jpg)

- Listen together: a room by its code, over public Nostr relays, sealed with a key that
  comes from the code. Guests hear what the host plays, in step, each on their own device;
  they suggest songs, or play and skip for everyone when the host lets them; and a chat.

![What's this song?](https://raw.githubusercontent.com/Niiihuel/surco/v0.13.0/docs/releases/0.13.0/recognize.jpg)

- What's this song?, from the search's start on a desktop: the microphone hears the song
  that plays nearby and Shazam names it from a fingerprint of the sound, never the
  recording. Not yet on an iPhone.

### Changed

- The code to move an account is shown in capitals and fours as it is typed, and what is
  brought takes the place of what is here in each part chosen, with a screen while it comes.
  A phone's sheet to bring data is taller and roomier.
- A card's picture stands in its grey, and its way out turns into a white pill under the
  pointer.
- What's new and the release notes fill the panel, what a release says in its middle.
- The lyrics page scrolls with the panels' bar, and the account's menu has its glyphs.

## 0.12.2

### Fixed

- On an iPhone, Settings › Move to another device and the profile's edit open again: they
  opened to nothing, as no sheet of the system's draws them. They stand over the header
  and the tab bar.

## 0.12.1

![Importing, on the card every dialog stands on](https://raw.githubusercontent.com/Niiihuel/surco/v0.12.1/docs/releases/0.12.1/import.jpg)

### Changed

- Every dialog stands on one card, as the offer to import does: a bold title and a quiet
  line in the middle, what it holds scrolling with the panels' bar, a round button for
  what goes ahead and the way out under the card. One stands at a time.
- An update that has to be taken stands alone on that card, over the first run too, and
  Escape does not put it away.
- The import's steps stand on a panel of their own, each number in a round badge.

![Moving an account, on the same card](https://raw.githubusercontent.com/Niiihuel/surco/v0.12.1/docs/releases/0.12.1/handoff.jpg)

### Fixed

- The side panels no longer flicker as they shut: the library folds into its rail from the
  first frame, and the right panel narrows to the strip that peeks instead of past it.

## 0.12.0

![Moving an account by a code or its QR](https://raw.githubusercontent.com/Niiihuel/surco/v0.12.0/docs/releases/0.12.0/handoff.jpg)

### Changed

- An account moves to another device by a code of twelve characters, typed in Settings ›
  Move to another device › Receive, or by its QR: on a computer or a phone, either way. The
  address it used to show is gone.
- A later version is offered over the window as the import is, with its first picture and
  a way to see what it brings, once no other dialog stands there.
- What's new shows a release's pictures at their own proportions, and its card as wide as
  its notes.
- Home's shelves show their bar under the pointer, as the panels do.

![The same on a phone](https://raw.githubusercontent.com/Niiihuel/surco/v0.12.0/docs/releases/0.12.0/handoff-phone.jpg)

### Fixed

- A song's dots among the recent searches open its options instead of closing the search.

### Removed

- Versions before 0.12.0 have to take this one in: they cannot read the code.

## 0.11.1

![The first run, in Figtree](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.1/docs/releases/0.11.1/welcome.jpg)

### Changed

- The interface is set in Figtree, round and geometric as Spotify's own face, in place of
  Ubuntu.

![Figtree across the library, the page and the panel](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.1/docs/releases/0.11.1/figtree.jpg)

- Each of the first run's steps sits under a colour of its own that gives way to black, and
  the welcome and the last step stand in the middle of the window.
- The button that shuts the right panel has a glyph of Surco's own, and over the song's
  video the panel's header no longer names the album.
- A playlist, an album, a mix or the liked songs are downloaded whole from their page, with
  Spotify's arrow in a circle.
- On an iPhone: Apple's own headers, a system navigation bar with a large title that
  scrolls under it and Search's own field; a playlist's share, export, picture and delete
  wait behind its dots, as on the desktop.

![Choosing genres](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.1/docs/releases/0.11.1/genres.jpg)

### Fixed

- Updating from a version before 0.11 goes through the first run's steps once: they were
  skipped for anyone who had played a song. Bringing an account in then joins the music
  already there instead of replacing it.
- Once the steps are gone through they never come back: what the app has is saved before
  an update starts the new version.
- The genres step's "Skip" no longer shows a white box.

## 0.11.0

![The first run](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.0/docs/releases/0.11.0/welcome.jpg)

### Added

- A first run in steps, as Spotify's sign-up: start fresh with a name, the genres you like
  and three or more artists picked from round faces, with a search and others like each
  one coming in after it. The artists picked are followed, so the home is yours from the
  first day. Settings open the genres and artists again.

![Choosing artists](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.0/docs/releases/0.11.0/artists.jpg)

- Or bring the account from another device in the first run's own step: the address the
  other device shows, or, on an iPhone, its code read with the camera inside Surco.

![Bringing your account](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.0/docs/releases/0.11.0/bring.jpg)

- On an iPhone: the account's face on Home and Library opens the profile, its name changes
  in a sheet, and its picture and a playlist's come from Photos.
- Importing is guided by the app a list comes from, Spotify, Apple Music or YouTube Music,
  with its way of copying the link; a link pasted chooses the app by itself. A fresh start
  ends on the offer to bring your music in.

![Importing from another app](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.0/docs/releases/0.11.0/import.jpg)

### Changed

- The profile shows the artists you follow before anything is played, and a card to make
  or bring a first playlist instead of an empty page.
- On a phone, the search tab is the genres to browse and what was searched lately shows
  once the field is in use; the library's chips are Spotify's and the header's edge is a
  short fade.
- The desktop's library no longer carries a card to import.

![On a phone](https://raw.githubusercontent.com/Niiihuel/surco/v0.11.0/docs/releases/0.11.0/phone.jpg)

## 0.10.2

![The panel of what plays and its menu](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.2/docs/releases/0.10.2/panel-menu.jpg)

### Changed

- A song's name too long for the right panel slides along it, as Spotify's: it rests, goes
  to its end, rests and comes back.
- The right panel's menu carries the icons a song's menu has elsewhere, and the button
  that shuts the panel an icon of Surco's own.

### Fixed

- A menu closes when the page is scrolled outside it.
- The right panel's like, fullscreen, share, album and artist no longer shared their
  names with the playback bar's own buttons.

## 0.10.1

![What plays, its video tall at the top of the right panel](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.1/docs/screenshots/playing.jpg)

### Changed

- A song among the recent searches plays or pauses from its cover, shows its wave while it
  sounds, and has its options behind its dots. Clearing the recent searches is a word in
  the middle.

### Fixed

- A song's video fades in over its cover instead of replacing it at once.
- The right panel keeps its corners round over the song's video.
- The cross inside a page's search no longer draws a disc over the field.

![Synced lyrics with their translation](https://raw.githubusercontent.com/Niiihuel/surco/v0.10.1/docs/screenshots/lyrics.jpg)

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
