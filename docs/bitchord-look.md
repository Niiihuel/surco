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
- Radii: list art 8 (52) and 7 (48), cards 12, hero cards 18, search field 11, settings
  groups 14, player artwork 8. Every thumbnail has a hairline at white 15%.

## Phone shell

- Floating tab bar: a full pill 19 from each side, 6 above the bottom inset (at least 15),
  at most 440 wide, 4 inside. Tabs about 62 tall, 28 icon, 10 label. The chosen tab sits on
  white at 14%, which slides on a spring; its icon grows to 1.08.
- Mini player: a pill 56 tall floating 8 above the tab bar. 40 art (corners 8), title over
  artist, play/pause and next. Swipe sideways to skip, drag up or tap to open the player.
- Both bars are frosted. limn has no backdrop blur, so they are a dark tint at high opacity
  with a 0.5 hairline at white 10%.
- Top bar: status inset + 52, no pane. Floating 44 circles for back and actions, a 28 face.
  Page titles are large headings at the top of the list, not in the bar.
- Edges: content fades out under a 180 gradient at the foot (254 with the mini player).
- Pushed pages cross-fade over 180 ms; tabs switch without a transition.

## Phone screens

- Song row: 4 above and below, 52 art, 14 gap, title 16/600 over artist 14 secondary,
  duration 12/600, a ⋮ of 36. The playing row: accent title over accent at 14%.
- Home: the time-of-day greeting large; a lead shelf of hero cards 70% of the width (at most
  320, ratio 0.92, corners 18) with the title over a dark gradient; recents as pages of 4 rows
  (48 art); other shelves of 150 square cards, corners 12, 10 apart.
- Search: a 46 field, corners 11, on surface variant; a segmented control (track #767680 at 24%,
  chosen #636366); results grouped under titles; artists with round art.
- Library: "Library" as a display heading, then folder rows 54 tall with a chevron, then
  shelves; the grid at least 140 a column.
- Album and playlist: the page in the cover's colour, the sleeve centred at 76% of the width
  (at most half the height), corners 12, a shadow of 18. Title 22/700 centred, credit 19 in
  the secondary colour. Actions: a 50 shuffle circle, a white Play pill, a 50 add circle; the
  circles white at 7% with a rim at 10%.
- Artist: the photo full width (ratio 0.95, at most 60% of the height) fading into the page's
  colour; the name 34/800 with its followers 14 under it, one group at the photo's foot; a 70
  Play circle between 52 shuffle and follow circles.
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

- Background: the cover blurred over the whole screen under a black scrim of 0.34 → 0.64.
  limn has no blur: the cover is drawn tiny and stretched, which blurs it the same way.
- 30 gutters, at most 560 wide. Square artwork the column's width, corners 8, shrinking to 0.86
  while paused. Title 20 white over artist 20/500 at 55%, with a like and a ⋯ circle of 34.
- Scrubber: a capsule without a thumb, 7 tall (12 while dragged), played white at 92%, the
  rest at 26%. Times 12/600 at 55%.
- Transport: play 74 in a 92 target, skips 53. Lyrics and queue glyphs at the foot, around the
  output button.
- Lyrics and queue open in place: the artwork shrinks into a 54 thumbnail in a 60 header.
- Lyrics: 34/41 bold, left aligned, the line being sung 36 under the lyrics' top (BitChord's
  website demo: `bitchord.kushagrasingh.in`). Lines away from it fade to 0.8, 0.7, 0.58,
  0.46, blur and step back to 0.98; what was sung stays lit, what is to come is at 45%.
- The line being sung fills behind a 30-point soft edge as each word is sung, the word
  rising 2 and settling 700 ms after it ends. That needs word timing, which LyricsPlus
  (Apple Music, Musixmatch, QQ) gives and LRCLIB does not: both are asked at once and the
  word-timed answer wins. A line timed only as a whole lights at once, as BitChord's does.

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
