# Phone and TV look, after BitChord

Surco's phone, Android TV and Apple TV layouts take their look from
[BitChord](https://github.com/kushagrasinghx/BitChord) (GPL-3.0, Kotlin/Compose). Only the
look is taken: the code stays Rust and limn, and BitChord's source is read as a reference,
never copied. The desktop layout does not change.

Order: phone (Android and iPhone share the view) → Android TV → Apple TV. The iPhone draws
the same bars as Android in limn; UIKit's own header and tab bar are not used for them.

Apple TV is reached by AirPlay from the iPhone, not by an app of its own: winit 0.30, which
eframe's iPhone window stands on, has no tvOS backend, and Rust's tvOS targets are tier 3
(nightly `-Zbuild-std`). The iPhone's player picks the Apple TV in its devices sheet (iOS's
own route picker); the audio session is long-form, so AirPlay 2 takes it, and Now Playing
hands the Apple TV the cover, title, artist, album and times. AirPlay plays the sound a
while after the player's clock: iOS's `outputLatency` for the route is taken off the
position the lyrics and their fill follow, so they stay with what is heard.

Numbers are in points (BitChord's dp). Paths below are BitChord's: `A` is
`app/src/main/java/com/music/bitchord`, `S` is `sharedUi/src/jvmSharedMain/kotlin/com/music/bitchord/ui`,
`T` is `tv/src/main/java/com/music/bitchord/ui/tv`.

## Theme

- Dark only on the phone and TV: canvas #000000, surface #0D0D0F, surface variant #1C1C1E,
  secondary text #8E8E93, outline #2C2C2E, primary white (`A/ui/theme/Theme.kt:26-48`).
- Album, playlist and artist pages are painted from their cover (`S/theme/ArtworkPalette.kt`):
  background at HSL lightness 0.13, accent at 0.62–0.78, secondary text at 0.80; covers
  with saturation under 0.12 stay grey. Colours cross over 260 ms.
- Type: display 34/800, headline 30/800 and 22/700, title 20/700 and 16/600, body 16 and 14,
  label 12/600 and 11/600. limn's faces stand in for SF Pro: Bold for 700–800, SemiBold for 600.
- Gutter 16. Shelves 14 apart with 26 under each. Row dividers 0.5 in outline, inset 84.
- Radii: list art 8 (52) and 7 (48), cards 12, hero cards 18, search fields round as the bars, settings
  groups 14, player artwork 8. Every thumbnail has a hairline at white 15%.

## Phone shell

- Floating tab bar: Home, Explore and Library in a full pill, and Search a 62 circle of its
  own 8 beside it; the row 19 from each side, 6 above the bottom inset (at least 15), at most
  440 wide, the pill 4 inside. Tabs about 62 tall, 24 icon, 10 label. The chosen tab sits on
  white at 14%, which slides on a spring.
- Once the page has gone 50 down the bar folds into one row: the chosen tab's circle, the mini
  player as a pill between, and Search, gliding to their places; 50 back up, or near the top,
  it opens again. The mini player's room under the page stays, so nothing jumps.
- Explore is a tab of its own: YouTube Music's moods and genres, read from its own browse so
  each keeps the stripe colour the service gives it (a muted one of eight by its name when it
  has none), two to a row, 16 from the sides and 12 apart, width over height 1.72, corners 18.
  Each card is its colour's stripe (darkened 8%) down the first 28%, then its first list's cover
  redrawn as a duotone from the colour darkened 68% to it lightened 32%, its name 17 bold
  white over a shade of black 24% fading out by 62%. The lists come one mood after another,
  so the covers fill in from the top. Pressed, a card opens its lists two to a row; a list
  opens as the home's mixes do, to be played, not kept. Without the service, Surco's own
  genres, each a search.
- Mini player: a pill 48 tall floating 8 above the tab bar, as BitChord's measured off Apple's:
  a 30.4 cover (corners 5.7) 16 in, title 13 bold over artist 12 at 70%, play/pause and next.
  Swipe sideways to skip, drag up or tap to open the player.
- The bar folds as any page, the search's results too, goes 50 down, and opens as it comes
  50 back up or reaches the top, on Compose's spring for shared elements (stiffness 400, no
  bounce): the tab capsule narrows into the chosen tab's circle as the other tabs and the
  names fade, and the mini player comes down between it and Search, losing its next button.
  Folded it is one row 48 tall, 28 in from the screen's sides, its parts 14 apart, on the
  open tabs' centre line; the tab circle opens it again. The tabs' glyphs are solid, the
  chosen one white and the others white at 65%.
- Both bars, the header's circles, a list's tools, the song's menu and the sheets are aurora's
  regular glass, dark, as the user's shell draws it: a 48% veil of (38, 38, 40) over the page
  rendered again beneath them, blurred σ 8 in the middle and bent 32 sharp at the rim (14 for
  circles), saturated 2.2, with aurora's shadow 0 10 40 at 20%. The circles are aurora's
  interactive glass: pressed, they grow to 1.075 on a spring of 0.30 s with a bounce of 0.35
  and light up around the press.
  The chosen tab's lit platter slides to the next, a clear lens while it travels, standing as
  tall as the whole bar and 9 past it, then deflating where it lands, as the video shows.
- Top bar: status inset + 52, no pane: iOS 26's soft scroll edge instead, as BitChord's, the page
  going up under it blurred (σ 8) and a little darkened over the bar's whole room, the status
  bar and the row of its buttons, giving way to the sharp page over 28 below it; it takes no
  press, and no fade into a colour. Floating 44 circles for back
  and actions, a 28 face.
  On a tab (Home, Explore, Library) the bar holds its circles and the face at its end, as
  BitChord's, and the tab's name stands under it, 34 bold, as the page's first row, scrolling
  up with it. Page titles are large headings at the top of the list, not in the bar.
- Edges: rows pass seen beneath the glass, as BitChord's, deepening only by the home
  indicator.
- Pushed pages cross-fade over 180 ms; tabs switch without a transition.

## Phone screens

- Song row: 4 above and below, 52 art, 14 gap, title 16/600 over artist 14 secondary,
  duration 12/600, a ⋮ of 36. The playing row: accent title over accent at 14%.
- Home: the time-of-day greeting large; "Top picks for you" leads, BitChord's cards 60% of the
  width (at most 280, three wide to four tall, corners 12, 14 apart), the time of day's mix
  among them, what it is (13) over its name (17 bold) and who it plays (13 at 86%) over a
  shade clear to 45%, black at 45% by 72% and 82% at the foot; then recents as pages of 4
  rows (48 art); then "Find your mood", the catalog's moods as 150 squares of their colour
  with their first list's cover drawn in it and their name under, its "See all" Explore;
  other shelves of 150 square cards, corners 12, 10 apart.
- Search: a 46 field, a capsule as the tabs and the segments, on surface variant; a segmented control (track #767680 at 24%,
  chosen #636366) whose knob moves as aurora's: on to the one chosen on a snappy spring, lifted
  1.3 into a clear lens over the labels for half its way, landing with a squash; results
  grouped under titles; artists with round art.
- Library: the tab opens BitChord's overview. "Your listening experience" across the top:
  cards 78% of the width (at most 340), three tall to five wide, corners 22, nearly black
  with a breath of plum (40, 34, 38) on a soft shadow: small capitals 11 bold, spaced apart
  by hair spaces as BitChord's tracking, the year's minutes 30 bold over them, the account's
  name in a typewriter's bold 15 over the year's plays, and "Listening since" with the first
  play kept, flush right, its date in the same bold (Surco has no account to have joined).
  Then its song and its artist on cards of their own. Each opens Replay.
  Then a row 55 tall with a chevron for the downloads (Surco keeps no local folders or cache
  to list), then shelves with "See all": Playlists, after New, Import and Liked (pink into
  violet), and the artists kept, round. "See all" is the whole list of that kind, the grid
  at least 140 a column.
- Album and playlist: the page in the cover's colour, the sleeve centred at 76% of the width
  (at most half the height), corners 12, a shadow of 18. Title 22/700 centred, credit 19 in
  the secondary colour. Actions: a 50 shuffle circle, a white Play pill, a 50 add circle; the
  circles white at 7% with a rim at 10%.
- Artist: the photo across the whole screen from its top edge (ratio 0.95), under a clear
  header until the name has gone up under it, dissolving over its lower half into the page's
  colour; the name centred over its foot in Display, 44 down to 28 to stay on one line (where
  BitChord has the artist's logo, which Surco does not have); a 70 Play circle between 52
  shuffle and follow (a star) circles of aurora's interactive glass. At the bar's end, a
  capsule of glass with a share, which copies the artist's link, and the account's face. Then the newest release on a card (96 cover, corners 12, what
  it is and its year over its name in 20 bold, on white at 6%, corners 20), "Top songs" 24
  bold, and the albums and singles shelves. While it comes, its photo and name stand where
  they are known from where the artist was chosen, and the rest pulses in its places.
- A photo picked for the profile or a playlist is cropped before it is kept, on the desktop,
  the phone and the TV alike: the whole photo with the square over it (round for a face),
  the rest dimmed, moved by a press, a drag or the arrows, its size by a slider; the square
  is kept at 640. The profile's page is washed in its photo's colour, as a list's in its
  cover's; without a photo, in its name's.
- A home with no picks to show nor on their way ends by saying how it fills, with the way to
  choose artists (the TV's goes to the search: the first run's picker is not a TV's).
- Settings: inset groups on surface variant, corners 14, 16 from the sides, uppercase 11/600
  headers; rows at least 52 with a 22 icon and a chevron.

## Phone player

- The song's video behind the player, as BitChord's over a Spotify Canvas: the whole of it
  where it lasts what the song does (720 lines, in time with it), else an 8-second stretch
  turning over, cut to fill the screen. From its first frame it fades in over the cover, under
  a black of 50% (measured: white reads 128); its lower 56% is drawn again blurred, fading in
  over the first 28% of that, for the controls to stand on (BitChord's 48-point haze; here the
  frame's foot 24 pixels across, stretched). A tap on the bare video puts the controls, the
  black and the blur away, leaving the song at its foot over a shade; another tap, or the next
  song, brings them back. Videos follow the setting that turns them off on the desktop.
- Background without a video: the cover blurred over the whole screen under BitChord's black
  scrim of 0.34 → 0.48 → 0.64, which gives its screens' greys over the same cover (measured
  within 3 of 255). The lyrics, the queue and a near-white cover's player keep a deeper
  0.47 → 0.80 for white words. limn has no blur: the cover is drawn tiny and stretched.
- The cover runs edge to edge across the top, under the status bar, as BitChord's full-bleed
  banner: square, taller where the controls would only spread more than 24 each side of the
  transport (its sides then cut), shorter where they need the room. Its bottom 42% dissolves
  into the blurred ground along smootherstep (limn's `RasterImage::dissolve_bottom`). A shade
  over its top keeps the handle (a 36 × 5 grip over "Playing from …": where it was started, or
  else its album) and the system's bar legible.
- The player comes down from anywhere on it but a list that has scrolled.
- 30 gutters, at most 560 wide. Under the banner, the stream as the backend picked it
  (`Opus · 141 kbps · 48 kHz`) 12 at 65%; title 20 white over artist 18/500 at 55%, with a like
  and a ⋯ circle of 34 (the like lit at 70% once liked); the line being sung, 15 at 80% with a
  chevron, opening the lyrics. No quality badge between the times: BitChord shows none under
  320 kbps, and Surco's streams are lossy.
- Scrubber: a capsule 7 tall (12 while dragged), round at both ends and where it is filled to,
  played white at 92%, the rest at 26%. Times 12/600 at 55%, 16 under the bar's middle.
- Transport: two-triangle skips flattened to 47 × 26 in 53 targets, play 37 tall in 84, over
  the foot's outer glyphs. Volume under it, a capsule 6 tall (10 dragged) between quiet and
  loud glyphs of 20 at 50%. The foot: lyrics, output (a pair of earbuds) and queue, 26 at 75%,
  at a fifth, the middle and four fifths across; the one open lit Apple's way, a 35 tile white
  at 62% (corners 7) with the glyph dark on it. The device's name under them where it is known.
  The sleep timer is in what plays's ⋯, named.
- Lyrics and queue open in the player on one shell: the grip over the song as the player names
  it beside its 54 cover, and the player's controls below (the queue's line being sung, or the
  lyrics' translation button, over the scrubber, the transport, the volume and the foot). A
  screen under 500 tall keeps a way back beside a 40 cover, and the scrubber and transport.
- The queue: its name 22 bold, "Now playing" (17, secondary) over what plays with its sound and
  a cross that skips it, then what was queued by hand and what follows under "AutoPlay" (when
  the catalog picked it) or "Next up"; 58 rows of a 44 cover, a grip that moves the row where
  it is let go and a cross that takes it out. While it is open the foot's middle is a capsule
  of shuffle, repeat and autoplay, 64 a segment, the ones on lit; the foot's outer glyphs open
  to 13 from the sides around it.
- Motion: opening the lyrics or the queue shrinks the player's cover from its banner into the
  head's 54 thumbnail in 330 ms (out-cubic), its corners rounding to 8, while the song's row
  glides up; closing grows it back. Coming down, the player takes its cover into the mini
  player's 40 thumbnail at the pace it moves, a finger's too; rising, it grows from there.
  The video gives way to the still cover while it travels.
- The output sheet, as BitChord's: cards 64 tall, 16 round, white at 6% (10% for where it
  plays, ticked): this phone, the route iOS names when it is elsewhere, AirPlay and Bluetooth
  where the system has routes, listening together, the volume (a 6 capsule), and "Audio
  Pipeline", the device's format under it.
- Audio Pipeline: a grey (#2C2C2E) card 340 at most, its name and what it is centred, then
  stations on a line a light runs down every 2.4 s (still in a screenshot or with movement
  reduced): the track (where it comes from, the stream), the decoder (libopus or symphonia,
  its rate and channels), the resampler (rodio, or passthrough where the rates agree), DSP
  (equaliser, normalisation, mono, crossfade, volume) and the output (the device as the
  system names it, the audio API, its format, and iOS's route), all as the engine reports
  them. "Done" under a hairline.
- The song's menu, grouped as BitChord's: like, dislike and share across the top; add to a
  playlist (which offers the lists); radio, play next, add to queue, download; album, artist,
  song info; and for what plays, the timer and the lyrics' step (half a second each way,
  back to 0 with the next song). A song disliked is skipped when it comes and kept out of
  the picks. A pane of glass 262 wide, corners 32, that pops out of the ⋯ (or the row held)
  as aurora's menus do: a small drop for 60 ms, then a spring for each edge (0.26 s, bounce
  0.30) into the menu, its corners and veil forming after a faint bloom, the rows focusing in
  through it from 0.233 to 0.40 s; closing, it shrinks into the drop and melts into the
  button. Every sheet of the phone (output, timer, lists, song info, lyrics' step, Audio
  Pipeline, importing, sharing, a new list, the profile's details, moving to another device,
  cropping a photo, confirmations) is aurora's glass and pops the same way out of whatever
  opened it. Tiles light no box under a pointer or a press.
- Profile: the face, the name 28 bold and what the account holds centred over the page's wash,
  which starts at the screen's top, under the bar,
  then its actions as the Library's rows: the name, Replay, listening together and moving to
  another device. Its right
  edge is the button's; it runs down from the button, or ends at the foot and covers it when
  that leaves too little room, as BitChord's does.
- Lyrics: 34/41 bold, left aligned, the line being sung 36 under the lyrics' top (BitChord's
  website demo: `bitchord.kushagrasingh.in`). Lines away from it fade to 0.8, 0.7, 0.58,
  0.46, blur and step back to 0.98; what was sung stays lit, what is to come is at 45%.
- The line being sung fills behind a 30-point soft edge as each word is sung, the word
  rising 2 and settling 700 ms after it ends. That needs word timing, which LyricsPlus
  (Apple Music, Musixmatch, QQ) and BetterLyrics (Apple Music's TTML) give and LRCLIB does
  not: all three are asked at once and the word-timed answer wins, LyricsPlus's first. A line timed only as a whole lights at once, as BitChord's does.

## TV

- No side rail: a centred capsule (white at 8%, 18 from the top) with Home, Library, Now
  Playing, and round Search and Settings, floating over the page. It slides away once the
  page scrolls 32, and the page goes back to its top when the focus goes up to it. Home and
  Library open as the focus rests 140 ms on their tab; the rest on select. Pages show no
  scroll bar.
- Focus lifts instead of outlining: cards scale 1.08 with a shadow, rows 1.03, buttons 1.1;
  rows and pills turn into a white platter with dark text.
- Safe margins 38 × 22, cards 16 apart, 131 lockups (corners 10), 120 round artists.
- Library in two panes under the tabs, which stay: its categories down the left, 240 wide
  (what was heard, liked songs, playlists, albums, artists, each only with something in, and
  Downloads with a chevron), 42 rows corners 11, the shown one on white at 12%; on the right
  what the focused one holds, songs as rows or a grid of 120 lockups. The focus going down
  the categories previews each.
- Settings as Apple's grouped lists, not split in halves: one column at most 600 wide under
  a 30/36 bold title; each group's name upper case 12/600 over a card (white at 7%, corners
  14) of 48 rows, hairlines from the words' start and none beside the focused row; what
  the focused row is for as its group's footnote, 13/18, in two lines of room that stay
  whether it speaks or not. Values are written out and change in place on select.
- Bringing the account from a phone, as BitChord's TV signs in with one: Settings › Move to
  another device › Bring from your phone shows a QR (on white, 200, corners 14; pulsing until
  it is made) beside three numbered steps. The phone reads it in Receive › Scan code, chooses
  what goes and sends it; the TV then lists what came as a settings card to bring in.
- What is on its way stands in its own shape and pulses (limn's `Skeleton`, in step with the
  covers coming): the home's pick shelves, the search's shelves until the first results, a
  song list opening (cover, title, Play and rows), the lyrics' lines on the phone and the TV,
  whose player keeps the lyrics' side while they come. A failed search says so with a Retry.
- Search field in the tabs' capsule shape: 576 × 44 on the capsule's white at 8%, fully
  round ends, no outline, 16 medium; the white platter with dark words under the focus.
- Album, playlist and artist pages scroll as one: a 232 cover (corners 10) beside the title
  31/36 (two lines at most), the artist 16/600 in Apple's red #FA2D48, "N songs, M min" 13,
  a 43 shuffle circle and a 58 red Play; then the songs, 48 tall with a 34 cover (an album's
  42 with its number), title 13 over artist 11, the length at the end, a hairline at white
  10% between them, red while one plays. The page's cover washes the top of the canvas.
- Canvas #26272C → #101012 with the cover's colour washed over the top.
- Player: the blurred cover, the artwork centred at 62% of the height (at most 320, 21 above
  the middle), or 54% (at most 272) 51 from the left when there are lyrics, which appear on
  the right centred on the current line. The transport rises from the foot over a shade that
  darkens to it: title 19 over artist — album 14 on the left, 37 glass buttons (like, lyrics)
  on the right, the scrubber under them, 4 thick, 6 with a knob under the focus, the times
  11 below. It steps away 5 s after the last press while the music plays, and the song is
  then named under the cover.
- Up Next opens under the scrubber on Down, or by its button beside the lyrics': "Up Next"
  16/600 with 32 shuffle, repeat and autoplay circles, then what follows as 99 lockups. The
  stage dims behind it and the transport stays up while it is open.
- D-pad: in the player, left/right skip and the rest shows the transport; on the scrubber
  left/right seek 10 s. Back closes the innermost thing first (the transport, a pushed page),
  then goes up to the tab bar, then Home, then out of the app.
- limn moved focus with Tab only; `UiTree::set_spatial_focus` (Niiihuel/limn#178) lets the
  arrows move it to the nearest control in their direction, keeping to its row.
