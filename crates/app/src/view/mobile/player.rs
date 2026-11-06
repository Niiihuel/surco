//! Phone player content, as BitChord's: one screen and no page to scroll. The cover runs edge
//! to edge across the top and dissolves into the blurred cover the controls stand on. The
//! mobile shell owns its header, that ground and the banner in it, and its dismissal gesture.

use super::*;

/// The player's sides, and the widest its column grows. BitChord's are 30; 28 lets the
/// transport fit a 320-point phone.
pub(super) const GUTTER: f32 = 28.0;
const COLUMN_MAX: f32 = 560.0;
/// How much of the banner, at its foot, dissolves into the blurred cover, as BitChord's.
const BANNER_DISSOLVE: f32 = 0.42;
/// How far the line of the stream reaches up into the banner's dissolve.
const BANNER_OVERLAP: f32 = 40.0;
/// The least the controls take under the banner: the stream, the song, the line being sung,
/// the timeline, the transport, the volume, the foot and the device, with the gaps between.
pub(super) const CONTROLS: f32 = 400.0;
/// The artist and the clock, white at 55% over the blurred cover.
const SOFT: Color = Color::rgba(255, 255, 255, 140);
/// The stream's line over the song, as BitChord's at 65%.
const STREAM: Color = Color::rgba(255, 255, 255, 166);
/// The foot's glyphs, as BitChord's at 75% until one is lit.
const FOOT: Color = Color::rgba(255, 255, 255, 191);
/// The transport's glyphs, as BitChord's: skips 47 across, flattened, and play 37 tall.
const SKIP: f32 = 51.0;
const PLAY: f32 = 67.0;
const SKIP_TARGET: f32 = 53.0;
pub(super) const PLAY_TARGET: f32 = 84.0;
/// The queue's capsule of shuffle, repeat and autoplay, as BitChord's: 64 a segment, 24 glyphs,
/// the one that is on lit within it.
const MODE_WIDTH: f32 = 64.0;
const MODE_GLYPH: f32 = 24.0;
const MODE_PILL: Color = Color::rgba(255, 255, 255, 36);
const MODE_LIT: Color = Color::rgba(255, 255, 255, 46);
const MODE_DIVIDER: Color = Color::rgba(255, 255, 255, 40);
/// The foot's glyphs, as BitChord's 26, and how far in from the column's sides the outer two
/// stand, so the three sit at a fifth, the middle and four fifths across.
const FOOT_GLYPH: f32 = 26.0;
const FOOT_EDGE: f32 = 30.0;
const FOOT_EDGE_OPEN: f32 = 13.0;
/// A lit tool's tile, as BitChord's: 35 across, corners of 7, white at 62% with the glyph cut
/// out of it (here drawn dark over it).
const TILE: Color = Color::rgba(255, 255, 255, 158);
const TILE_SIDE: f32 = 35.0;
const TILE_CORNER: f32 = 7.0;
const CUT: Color = Color::rgba(28, 28, 30, 230);
/// How far the times rise into the seek bar's touch target, to stand 16 under the bar's middle
/// as BitChord's do.
const TIMES_RISE: f32 = 14.0;
/// The round buttons beside the song, white at 18%, and at 70% while what they say is on, as
/// BitChord's like once the song is liked.
const VEIL: Color = Color::rgba(255, 255, 255, 46);
const ON: Color = Color::rgba(255, 255, 255, 178);
/// A tool that is on, as shuffle while it shuffles.
const LIT: Color = Color::rgba(255, 255, 255, 40);
/// What a slider has gone through, and what it has left.
const FILLED: Color = Color::rgba(255, 255, 255, 235);
const UNFILLED: Color = Color::rgba(255, 255, 255, 66);

/// The most room a tall screen puts on each side of the transport, as BitChord's: past it the
/// controls stop reading as one group, and the rest goes to the banner.
const SPREAD: f32 = 24.0;

/// How tall the banner stands on a screen `window`, `bottom` of it the system's: square across
/// it, less where the controls need the room, and taller where they would only spread apart,
/// its cover then cut at the sides.
pub(super) fn banner_height(window: Size, bottom: f32) -> f32 {
    let room = window.height - bottom - CONTROLS + BANNER_OVERLAP;
    room.min(window.width).max(room - 2.0 * SPREAD).max(160.0)
}

/// The playing song's cover across `width`, `height` tall, its foot dissolving: the large
/// rendition once it has arrived. A song with no cover to be had shows the music's glyph in
/// its place; nothing stands there while one is on its way.
pub(super) fn banner(state: &State, width: f32, height: f32) -> Option<Element> {
    let track = state.current()?;
    let Some(image) = largest(state, track).and_then(|url| state.artwork.get(&url)) else {
        let coming = track.artwork_url.as_ref().is_some_and(|url| state.cover_coming(url));
        let note = Element::icon(Icon::MusicNote).large_icon(64.0).foreground(SOFT).centered();
        return (!coming).then(|| note.size(Some(width), Some(height)));
    };
    // Decoded whole: limn's thumbnails stop at 1024 pixels, short of a phone's 1206 across.
    let banner = Element::raster_image(image.clone().cover().full_resolution().dissolve_bottom(BANNER_DISSOLVE)).size(Some(width), Some(height));
    Some(banner.animate_enter(format!("mobile-player-banner:{}", track.id)))
}

/// The playing song's cover filling `width` by `height`, without the banner's dissolve: what
/// travels between the banner and the lyrics' or the queue's head.
pub(super) fn picture(state: &State, width: f32, height: f32) -> Option<Element> {
    let image = largest(state, state.current()?).and_then(|url| state.artwork.get(&url))?;
    Some(Element::raster_image(image.clone().cover().full_resolution()).size(Some(width), Some(height)))
}

/// The controls under the banner, in a screen `window` whose bars take `top`.
pub(super) fn screen(state: &State, width: f32, window: Size, top: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(blank(Icon::MusicNote, t!("mobile-no-music"), t!("mobile-no-music-detail"), None, palette));
    };
    let gutter = GUTTER.max((width - COLUMN_MAX) / 2.0);
    let bottom = state.mobile_ui.content_insets.bottom;
    let gap = |height: f32| Element::spacer().size(None, Some(height));
    // The stream's line starts where the banner dissolves; the padding below takes its 12.
    let above = (banner_height(window, bottom) - top - BANNER_OVERLAP - 12.0).max(0.0);
    let stream = state.format_of(&track.id).map_or_else(Element::spacer, |format| {
        Text::new(format, palette).font_size(12.0).ink(STREAM).truncate().render().font_family(FontFamily::SansMedium).centered_x()
    }).size(None, Some(16.0));
    let details = song(state, track, palette);
    let sung = match &state.player.error {
        Some(error) => Alert::new(t!("mobile-play-failed"), error, palette).id("mobile-player-error").render(),
        None => sung_line(state, palette),
    };
    // A screen taller than the controls need gives the room around the transport.
    let body = Element::column(vec![
        gap(above), stream, gap(18.0), details, gap(10.0), sung, gap(6.0), timeline(state, palette),
        gap(4.0), Element::spacer().flex(1.0), transport(state, palette), gap(4.0), Element::spacer().flex(1.0),
        volume(state, palette), gap(8.0), foot(state, palette), device(state, palette),
    ]).padding(12.0).flex(1.0);
    horizontal_inset(body, gutter - 12.0, gutter - 12.0)
}

/// What plays, named over its artist, with the like and ⋯ circles beside it.
pub(super) fn song(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    let liked = state.is_liked(&track.id);
    Element::row(vec![
        Element::column(vec![
            Text::new(&track.title, palette).font_size(20.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(&track.artist, palette).font_size(18.0).ink(SOFT).truncate().render().font_family(FontFamily::SansMedium),
        ]).gap(2.0).flex(1.0).centered_y(),
        circle("like", if liked { Icon::HeartFill } else { Icon::Heart }, &if liked { t!("mobile-unlike") } else { t!("common-like") }, liked, palette),
        circle("more:current", Icon::More, &t!("common-song-options"), false, palette),
    // The same row on the player, the bare video and the lyrics' or the queue's head: it
    // glides from one place to the other as the cover shrinks or grows.
    ]).gap(6.0).anchor("mobile-player-title").animate_move("mobile-player-song")
}

/// The player with its controls put away, as BitChord's over a bare Canvas: only the song,
/// at the foot of the video.
pub(super) fn bare(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else { return Element::spacer() };
    let gutter = GUTTER.max((width - COLUMN_MAX) / 2.0);
    let body = Element::column(vec![Element::spacer().flex(1.0), song(state, track, palette)]).padding(12.0).flex(1.0);
    horizontal_inset(body, gutter - 12.0, gutter - 12.0).animate_enter("mobile-player-bare")
}

/// A round button beside the song: a 34 disc on its 44 touch target, lighter while `on`.
pub(super) fn circle(id: &str, glyph: Icon, label: &str, on: bool, palette: ComponentPalette) -> Element {
    let disc = Element::panel(Element::icon(glyph).large_icon(18.0).foreground(palette.text).centered())
        .background(if on { ON } else { VEIL }).pressed_background(LIT).round().size(Some(34.0), Some(34.0));
    Element::clickable(id, disc.centered()).accessible(limn::Access::new(limn::Role::Button).name(label).selected(on))
        .radius(TAP / 2.0).size(Some(TAP), Some(TAP)).centered_y()
}

/// The line being sung, on one line under the song, as BitChord's strip, leading to the whole
/// lyrics; a line of room whether or not there is one, so the controls under it do not jump.
pub(super) fn sung_line(state: &State, palette: ComponentPalette) -> Element {
    let line = state.active_line().and_then(|index| state.lines().get(index).map(|line| (index, line)))
        .filter(|(_, line)| !line.text.trim().is_empty());
    let Some((index, line)) = line else { return Element::spacer().size(None, Some(24.0)) };
    let words = Text::new(&line.text, palette).font_size(15.0).ink(Color::rgba(255, 255, 255, 205)).truncate().render().font_family(FontFamily::SansMedium);
    let row = Element::row(vec![words.flex(1.0).centered_y(), Element::icon(Icon::ChevronRight).large_icon(14.0).foreground(SOFT).centered_y()]).gap(6.0);
    Element::clickable("sung-line", row).accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-show-lyrics")))
        .size(None, Some(24.0)).animate_enter(format!("mobile-sung-line:{index}"))
}

pub(super) fn timeline(state: &State, palette: ComponentPalette) -> Element {
    let duration = state.player.duration.max(0.0);
    let progress = state.progress();
    let elapsed = if duration > 0.0 { f64::from(progress) * duration } else { state.player.position.max(0.0) };
    // Ceiling keeps the countdown consistent with the elapsed clock's whole seconds,
    // including the tiny rounding error introduced by the slider's f32 fraction.
    let remaining = if duration > 0.0 { format!("-{}", clock((duration - elapsed).max(0.0).ceil())) } else { "–:––".into() };
    let time = |value: String| Text::new(value, palette).font_size(12.0).ink(SOFT).render().font_family(FontFamily::SansMedium);
    // BitChord's scrubber: a capsule, thicker while it is held, round at its ends and where
    // it is filled to: limn rounds a bar's ends by half its thumb, here as thick as the bar.
    let track = ComponentPalette { selected: UNFILLED, ..palette };
    Element::column(vec![
        Slider::new("seek", progress, track).enabled(duration > 0.0).thumb(7.0, 12.0).bar(7.0, 12.0).ink(FILLED)
            .accessibility_label(t!("mobile-playback-position")).render().size(None, Some(TAP)),
        Element::row(vec![time(clock(elapsed)), Element::spacer().flex(1.0), time(remaining)]).translate(limn::Point::new(0.0, -TIMES_RISE)),
    ])
}

pub(super) fn transport(state: &State, palette: ComponentPalette) -> Element {
    // Bare glyphs, as BitChord's: no disc behind play.
    let glyph = |id: &str, icon: Icon, label: &str, size: f32, target: f32| {
        Element::clickable(id, Element::icon(icon).large_icon(size).foreground(palette.text).centered())
            .accessible(limn::Access::new(limn::Role::Button).name(label))
            .background(Color::TRANSPARENT).pressed_background(LIT)
            .radius(target / 2.0).size(Some(target), Some(target)).centered_y()
    };
    let playing = state.player.playing;
    // The skips stand over the foot's outer glyphs; shuffle and repeat are the queue's.
    let edge = || Element::spacer().size(Some(FOOT_EDGE - (SKIP_TARGET - TAP) / 2.0), None);
    Element::row(vec![
        edge(),
        glyph("previous", Icon::Custom(crate::view::glyph::BACKWARD), &t!("mobile-previous-song"), SKIP, SKIP_TARGET),
        Element::spacer().flex(1.0),
        glyph("play-pause", if playing { Icon::PauseFill } else { Icon::PlayFill }, &if playing { t!("common-pause") } else { t!("common-play") }, PLAY, PLAY_TARGET),
        Element::spacer().flex(1.0),
        glyph("next", Icon::Custom(crate::view::glyph::FORWARD), &t!("common-next-song"), SKIP, SKIP_TARGET),
        edge(),
    ])
}

/// The queue's ways of going on, in one capsule at the foot's middle while it is open, as
/// BitChord's: shuffle, repeat and autoplay, each lit while it is on.
fn modes(state: &State, palette: ComponentPalette) -> Element {
    let (shuffle, shuffle_label) = shuffle_step(state);
    let repeat = if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat };
    let segment = |id: &str, glyph: Icon, label: &str, on: bool| {
        Element::clickable(id, Element::icon(glyph).large_icon(MODE_GLYPH).foreground(if on { palette.text } else { FOOT }).centered())
            .accessible(limn::Access::new(limn::Role::Button).name(label).selected(on))
            .background(if on { MODE_LIT } else { Color::TRANSPARENT }).pressed_background(LIT)
            .size(Some(MODE_WIDTH), Some(TAP))
    };
    let divider = || Element::spacer().background(MODE_DIVIDER).size(Some(1.0), Some(20.0)).centered_y();
    let row = Element::row(vec![
        segment("shuffle", shuffle, &shuffle_label, state.player.shuffle), divider(),
        segment("repeat", repeat, &t!("mobile-repeat"), state.player.repeat != Repeat::Off), divider(),
        segment("setting:autoplay", Icon::Infinity, &t!("mobile-autoplay"), state.settings.autoplay),
    ]);
    Element::panel(row).background(MODE_PILL).radius(TAP / 2.0).clip_children().centered_y()
}

/// How loud Surco plays, between its quiet and loud glyphs, as BitChord's: a thinner capsule
/// than the timeline's, its glyphs 20 at half.
pub(super) fn volume(state: &State, palette: ComponentPalette) -> Element {
    let glyph = |icon: &'static str| Element::icon(Icon::Custom(icon)).large_icon(20.0).foreground(Color::rgba(255, 255, 255, 128)).centered_y();
    let slider = Slider::new("volume", state.volume, ComponentPalette { selected: UNFILLED, ..palette }).thumb(6.0, 10.0).bar(6.0, 10.0).ink(FILLED)
        .accessibility_label(t!("mobile-volume")).render().size(None, Some(TAP)).flex(1.0);
    Element::row(vec![glyph(crate::view::glyph::VOLUME_LOW), slider, glyph(crate::view::glyph::VOLUME_HIGH)]).gap(10.0)
}

/// Where it plays, under the foot, as Apple Music names it; a line of room when it is not known.
fn device(state: &State, palette: ComponentPalette) -> Element {
    let named = state.mobile_ui.route.as_ref().map(|(name, _)| name.as_str()).filter(|name| !name.is_empty());
    named.map_or_else(Element::spacer, |name| Text::new(name, palette).font_size(12.0).ink(SOFT).truncate().render().centered_x()).size(None, Some(18.0))
}

/// Where it plays, as BitChord's: a pair of earbuds.
const EARBUDS: &str = include_str!("../../../assets/icons/earbuds.svg");
/// What comes next, as BitChord's: a list with its bullets.
const LIST: &str = include_str!("../../../assets/icons/list-bullet.svg");

/// The player's foot, the same under its cover, its lyrics and its queue, as BitChord's: the
/// lyrics' button, where it plays, and the queue's, the one that is open lit on a tile. While
/// the queue is open its ways of going on take the middle. The timer is in the song's ⋯.
pub(super) fn foot(state: &State, palette: ComponentPalette) -> Element {
    let lyrics_open = state.page == Page::Lyrics;
    let queue_open = state.page == Page::Playing && state.queue_open;
    let dim = ComponentPalette { text: FOOT, ..palette };
    let tool = |id: &str, glyph: &'static str, label: &str, lit: bool| {
        if !lit { return super::controls::action(id, Icon::Custom(glyph), label, FOOT_GLYPH, dim).radius(10.0); }
        // Lit as BitChord's, Apple's way: a light tile with the glyph cut out of it.
        let tile = Element::panel(Element::icon(Icon::Custom(glyph)).large_icon(FOOT_GLYPH).foreground(CUT).centered())
            .background(TILE).radius(TILE_CORNER).size(Some(TILE_SIDE), Some(TILE_SIDE));
        Element::clickable(id, tile.centered()).accessible(limn::Access::new(limn::Role::Button).name(label).selected(true))
            .radius(10.0).size(Some(TAP), Some(TAP))
    };
    // The capsule's room comes from the sides, as BitChord's foot opens around it.
    let edge = || Element::spacer().size(Some(if queue_open { FOOT_EDGE_OPEN } else { FOOT_EDGE }), None);
    let middle = if queue_open { modes(state, palette) } else { tool("mobile-connections", EARBUDS, &t!("mobile-connect-device"), false) };
    Element::row(vec![
        edge(), tool("lyrics", crate::view::LYRICS, &if lyrics_open { t!("mobile-close-lyrics") } else { t!("mobile-show-lyrics") }, lyrics_open), Element::spacer().flex(1.0),
        middle, Element::spacer().flex(1.0),
        tool("queue", LIST, &if queue_open { t!("mobile-back-to-song") } else { t!("mobile-show-queue") }, queue_open), edge(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{DrawCommand, Frame, UiAction, UiTree};
    use surco_core::lyrics::Line;

    fn state() -> State {
        let track = Track {
            id: "song".into(), title: "Una canción con un título largo y acentos para el teléfono".into(),
            artist: "Un artista".into(), artist_id: Some("artist".into()),
            album: None, album_id: None, artwork_url: None, duration_ms: 180_000,
        };
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Playing;
        state.show(PlayerView { current: Some(track), playing: true, position: 30.0, duration: 180.0, ..PlayerView::default() });
        state
    }

    fn frame(state: &State) -> Frame {
        // A 390 × 844 phone whose bars take 64 at the top.
        let mut tree = UiTree::new(screen(state, 390.0, Size::new(390.0, 844.0), 64.0, SCHEME.palette()));
        tree.set_reduced_motion(true);
        tree.layout(Size::new(390.0, 780.0))
    }

    fn painted(frame: &Frame, expected: &str) -> usize {
        frame.commands.iter().filter(|command| matches!(command, DrawCommand::Text { text, .. } if text == expected)).count()
    }

    #[test]
    fn the_line_being_sung_shows_under_the_song_only_while_it_is_sung() {
        let mut state = state();
        assert_eq!(painted(&frame(&state), "Segunda línea real"), 0);
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![
            Line { words: Vec::new(), at_ms: 5_000, text: "Primera línea real".into() },
            Line { words: Vec::new(), at_ms: 25_000, text: "Segunda línea real".into() },
        ])));
        let sung = frame(&state);
        assert_eq!(painted(&sung, "Segunda línea real"), 1);
        assert_eq!(painted(&sung, "Primera línea real"), 0, "one line, the one being sung");
        state.lyrics.as_mut().unwrap().0 = "previous-song".into();
        assert_eq!(painted(&frame(&state), "Segunda línea real"), 0, "a previous track's line never shows");
    }

    #[test]
    fn the_player_fits_one_screen_with_its_foot_and_toggles_the_lyrics_in_place() {
        let mut state = state();
        let ids = ["like", "more:current", "seek", "previous", "play-pause", "next", "volume", "lyrics", "mobile-connections", "queue"];
        for size in [Size::new(320.0, 568.0), Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            for page in [Page::Playing, Page::Lyrics] {
                state.page = page.clone();
                let mut tree = UiTree::new(crate::view::view(&state, size));
                tree.set_reduced_motion(true);
                let shown = tree.layout(size);
                for id in ids.iter().copied().filter(|id| page == Page::Playing || !matches!(*id, "like" | "more:current" | "volume")) {
                    let nodes: Vec<_> = shown.access.iter().filter(|node| node.id == id).collect();
                    assert_eq!(nodes.len(), 1, "one {id} on {page:?} at {size:?}");
                    assert_eq!(shown.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count(), 1, "one target for {id}");
                    let rect = nodes[0].rect;
                    assert!(rect.width >= 43.99 && rect.height >= 43.99, "{id}: {rect:?}");
                    assert!(rect.x >= 0.0 && rect.x + rect.width <= size.width + 0.01 && rect.y >= 0.0 && rect.y + rect.height <= size.height + 0.01,
                        "{id} on screen, not scrolled to: {rect:?}");
                }
            }
        }
        state.page = Page::Playing;
        state.update(&UiAction::Clicked { id: "lyrics".into() });
        assert_eq!(state.page, Page::Lyrics);
        state.update(&UiAction::Clicked { id: "lyrics".into() });
        assert_eq!(state.page, Page::Playing, "the lit button puts the cover back");
    }

    #[test]
    fn the_banner_is_square_until_the_controls_would_only_spread_apart() {
        let phone = |height: f32| banner_height(Size::new(390.0, height), 0.0);
        assert_eq!(phone(700.0), 340.0, "short: the controls take what they need");
        assert_eq!(phone(780.0), 390.0, "square across the screen");
        assert_eq!(phone(844.0), 436.0, "tall: 24 each side of the transport, the rest to the banner");
        assert_eq!(phone(400.0), 160.0, "never less than a strip");
    }

    #[test]
    fn under_the_banner_stand_the_stream_and_a_sung_line_that_opens_the_lyrics() {
        let mut state = state();
        state.apply(crate::backend::Event::Format { track_id: "song".into(), format: "Opus · 141 kbps · 48 kHz".into() });
        let texts = |frame: &Frame| frame.commands.iter().filter_map(|command| match command { DrawCommand::Text { text, .. } => Some(text.clone()), _ => None }).collect::<Vec<_>>();
        assert!(texts(&frame(&state)).iter().any(|text| text == "Opus · 141 kbps · 48 kHz"));
        assert!(!frame(&state).regions.iter().any(|region| region.id == "sung-line"), "no line, nothing to press");
        state.update(&UiAction::Clicked { id: "sung-line".into() });
        assert_eq!(state.page, Page::Lyrics, "pressed, it opens the lyrics as their button does");
    }

    #[test]
    fn audio_route_button_is_a_pair_of_earbuds() {
        let shown = frame(&state());
        let button = shown.regions.iter().find(|region| region.id == "mobile-connections").unwrap();
        let expected = Icon::Custom(EARBUDS);
        let glyphs: Vec<_> = shown.commands.iter().filter_map(|command| match command {
            DrawCommand::Icon { rect, icon, .. } if button.rect.contains(limn::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)) => Some(*icon),
            _ => None,
        }).collect();
        assert_eq!(glyphs, [expected]);
    }

    #[test]
    fn over_its_video_the_player_puts_its_controls_away_and_keeps_the_song() {
        let mut state = state();
        let track = state.current().unwrap().clone();
        let video = Track { id: "video".into(), ..track.clone() };
        let footage = std::sync::Arc::new(crate::backend::Footage { clip: surco_video::Clip { parameters: vec![0], samples: Vec::new() }, small: Default::default(), large: Default::default(), video });
        state.apply(crate::backend::Event::Clip { track_id: track.id.clone(), footage: Some(footage) });
        let size = Size::new(390.0, 844.0);
        let shown = |state: &State| {
            let mut tree = UiTree::new(crate::view::view(state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let has = |id: &str| frame.access.iter().any(|node| node.id == id);
            let surfaces = frame.commands.iter().filter(|command| matches!(command, DrawCommand::Surface { .. })).count();
            (has("mobile-canvas"), has("play-pause"), has("like"), surfaces)
        };
        assert_eq!(shown(&state), (false, true, true, 0), "the cover until the video's first frame");
        state.clip_ready = state.screening().map(|(name, ..)| name);
        assert_eq!(shown(&state), (true, true, true, 2), "then the video, and its foot blurred under the controls");
        state.update(&UiAction::Clicked { id: "mobile-canvas".into() });
        assert_eq!(shown(&state), (true, false, true, 1), "put away, only the song stays over the bare video");
    }

    #[test]
    fn the_timer_is_named_in_the_songs_menu_rather_than_a_moon_at_the_foot() {
        let mut state = state();
        assert!(!frame(&state).access.iter().any(|node| node.id == "sleep"));
        let track = state.current().unwrap().clone();
        assert!(crate::view::track_actions(&state, "current", &track).iter().flatten().any(|action| action.id == "do:sleep:current"));
        assert!(!crate::view::track_actions(&state, "page-track:0", &track).iter().flatten().any(|action| action.id.starts_with("do:sleep")), "only what plays");
        state.update(&UiAction::Clicked { id: "more:current".into() });
        state.update(&UiAction::Clicked { id: "do:sleep:current".into() });
        assert!(state.sleep_open && state.mobile_ui.context.is_none(), "the menu gives way to the timer's sheet");
    }

    #[test]
    fn the_queue_opens_in_the_player_with_shuffle_repeat_and_autoplay_at_the_foots_middle() {
        let mut state = state();
        let size = Size::new(390.0, 844.0);
        let shown = |state: &State| {
            let mut tree = UiTree::new(crate::view::view(state, size));
            tree.set_reduced_motion(true);
            tree.layout(size)
        };
        let has = |frame: &Frame, id: &str| frame.access.iter().filter(|node| node.id == id).count();
        let player = shown(&state);
        assert_eq!((has(&player, "shuffle"), has(&player, "repeat"), has(&player, "mobile-connections")), (0, 0, 1), "the transport is the skips and play alone");
        state.update(&UiAction::Clicked { id: "queue".into() });
        let queue = shown(&state);
        for id in ["shuffle", "repeat", "setting:autoplay", "play-pause", "volume", "queue", "mobile-lyrics-close"] {
            assert_eq!(has(&queue, id), 1, "{id} over the queue");
        }
        assert_eq!(has(&queue, "mobile-connections"), 0, "the capsule takes the output's place");
        let autoplay = queue.access.iter().find(|node| node.id == "setting:autoplay").unwrap();
        assert_eq!(autoplay.selected, Some(true), "lit while it is on");
        state.update(&UiAction::Clicked { id: "queue".into() });
        assert!(!state.queue_open, "the lit button puts the song back");
    }

    #[test]
    fn an_unknown_duration_disables_seek() {
        let mut state = state();
        state.player.duration = 0.0;
        assert!(frame(&state).access.iter().any(|node| node.id == "seek" && node.disabled));
    }
}

#[cfg(test)]
mod flight_tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{DrawCommand, UiTree};

    #[test]
    fn the_cover_is_drawn_on_its_way_into_the_lyrics_head() {
        let track = Track { id: "song".into(), title: "Canción".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None,
            artwork_url: Some("https://example.com/cover.jpg".into()), duration_ms: 180_000 };
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Playing;
        state.show(PlayerView { current: Some(track), playing: true, duration: 180.0, ..PlayerView::default() });
        let image = limn::RasterImage::new("ab".repeat(32), limn::RasterImageFormat::Png, 7, std::sync::Arc::from("aGVsbG8hIQ=="), "cover").unwrap();
        state.artwork.insert("https://example.com/cover.jpg".into(), image);
        let size = Size::new(390.0, 844.0);
        let images = |state: &State| {
            let mut tree = UiTree::new(crate::view::view(state, size));
            tree.set_reduced_motion(true);
            tree.layout(size).commands.iter().filter_map(|command| match command { DrawCommand::RasterImage { rect, .. } => Some(*rect), _ => None }).collect::<Vec<_>>()
        };
        assert_eq!(images(&state), [limn::Rect::new(0.0, 0.0, 390.0, 436.0)], "at rest, the banner across the top");
        state.mobile_ui.collapse = 0.5;
        assert_eq!(images(&state), [limn::Rect::new(14.0, 24.0, 222.0, 245.0)], "halfway back from the head, halfway in size and place");
        state.page = Page::Lyrics;
        state.mobile_ui.collapse = 1.0;
        assert_eq!(images(&state), [limn::Rect::new(28.0, 48.0, 54.0, 54.0)], "there, the head's thumbnail");
        // Coming down, the cover goes into the mini player's thumbnail at the player's pace,
        // and is not carried down with it.
        state.page = Page::Playing;
        state.mobile_ui.collapse = 0.0;
        state.mobile_ui.player_offset = 422.0;
        // BitChord's mini player: 48 tall, its 30.4 cover 16 in from its start.
        let mini = limn::Rect::new(19.0 + 16.0, 844.0 - 21.0 - 62.0 - 56.0 + (48.0 - 30.4) / 2.0, 30.4, 30.4);
        let halfway = limn::Rect::new(mini.x / 2.0, mini.y / 2.0, (390.0 + 30.4) / 2.0, (436.0 + 30.4) / 2.0);
        assert_eq!(images(&state), [halfway], "half of the way down, half of the way there");
    }
}
