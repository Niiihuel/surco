# Phone and TV look, after BitChord

Surco's phone, Android TV and Apple TV layouts take their look from
[BitChord](https://github.com/kushagrasinghx/BitChord) (GPL-3.0, Kotlin/Compose). Only the
look is taken: the code stays Rust and limn, and BitChord's source is read as a reference,
never copied. The desktop layout does not change.

Order: phone (Android and iPhone share the view) → Android TV → Apple TV. The iPhone draws
the same bars as Android in limn; UIKit's own header and tab bar are not used for them.

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
  colour; the name 34/800; a 70 Play circle between 52 shuffle and follow circles.
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
- Lyrics: 34/41 bold, left aligned, the current line near the top. Lines away from it fade to
  1, 0.74, 0.6, 0.48, 0.4, 0.34. (BitChord also blurs them; limn cannot.)

## TV

- No side rail: a centred capsule (white at 8%, 18 from the top) with Home, Library, Now
  Playing, and round Search and Settings. It hides once the page scrolls 32.
- Focus lifts instead of outlining: cards scale 1.08 with a shadow, rows 1.03, buttons 1.1;
  rows and pills turn into a white platter with dark text.
- Safe margins 38 × 22, cards 16 apart, 131 lockups (corners 10), 120 round artists.
- Canvas #26272C → #101012 with the cover's colour washed over the top.
- Player: the blurred cover, the artwork centred at 62% of the height, gliding left when there
  are lyrics, which appear on the right centred on the current line. The transport hides
  after 5 s.
- D-pad: in the player, left/right skip and the rest shows the transport; on the scrubber
  left/right seek 10 s. Back closes the innermost thing first, then goes Home.
- limn moves focus with Tab only. The TV needs arrow keys to move focus to the nearest
  control in their direction: that is a limn change.
