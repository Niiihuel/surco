//! Phone player content, as BitChord's: one screen and no page to scroll. The cover runs edge
//! to edge across the top and dissolves into the blurred cover the controls stand on. The
//! mobile shell owns its header, that ground and the banner in it, and its dismissal gesture.

use super::*;

/// The player's sides, and the widest its column grows. BitChord's are 30; 28 lets the
/// transport fit a 320-point phone.
const GUTTER: f32 = 28.0;
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
/// The round buttons beside the song, white at 18%.
const VEIL: Color = Color::rgba(255, 255, 255, 46);
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
        Text::new(format, palette).font_size(12.0).ink(SOFT).truncate().render().font_family(FontFamily::SansMedium).centered_x()
    }).size(None, Some(16.0));
    let liked = state.is_liked(&track.id);
    let details = Element::row(vec![
        Element::column(vec![
            Text::new(&track.title, palette).font_size(20.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(&track.artist, palette).font_size(18.0).ink(SOFT).truncate().render().font_family(FontFamily::SansMedium),
        ]).gap(2.0).flex(1.0).centered_y(),
        circle("like", if liked { Icon::HeartFill } else { Icon::Heart }, &if liked { t!("mobile-unlike") } else { t!("common-like") }, palette),
        circle("more:current", Icon::More, &t!("common-song-options"), palette),
    ]).gap(6.0).anchor("mobile-player-title");
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

/// A round button beside the song: a 34 disc on its 44 touch target.
fn circle(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    let disc = Element::panel(Element::icon(glyph).large_icon(18.0).foreground(palette.text).centered())
        .background(VEIL).pressed_background(LIT).round().size(Some(34.0), Some(34.0));
    Element::clickable(id, disc.centered()).accessible(limn::Access::new(limn::Role::Button).name(label))
        .radius(TAP / 2.0).size(Some(TAP), Some(TAP)).centered_y()
}

/// The line being sung, on one line under the song, as BitChord's strip, leading to the whole
/// lyrics; a line of room whether or not there is one, so the controls under it do not jump.
fn sung_line(state: &State, palette: ComponentPalette) -> Element {
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
    // BitChord's scrubber: a capsule with no thumb, thicker while it is held.
    let track = ComponentPalette { selected: UNFILLED, ..palette };
    Element::column(vec![
        Slider::new("seek", progress, track).enabled(duration > 0.0).thumb(0.0, 0.0).bar(7.0, 12.0).ink(FILLED)
            .accessibility_label(t!("mobile-playback-position")).render().size(None, Some(TAP)),
        Element::row(vec![time(clock(elapsed)), Element::spacer().flex(1.0), time(remaining)]),
    ])
}

pub(super) fn transport(state: &State, palette: ComponentPalette) -> Element {
    // Bare glyphs, as BitChord's: no disc behind play.
    let glyph = |id: &str, icon: Icon, label: &str, size: f32, target: f32, on: bool| {
        Element::clickable(id, Element::icon(icon).large_icon(size).foreground(palette.text).centered())
            .accessible(limn::Access::new(limn::Role::Button).name(label))
            .background(if on { LIT } else { Color::TRANSPARENT }).pressed_background(LIT)
            .radius(target / 2.0).size(Some(target), Some(target)).centered_y()
    };
    let playing = state.player.playing;
    let (shuffle, shuffle_label) = shuffle_step(state);
    Element::row(vec![
        glyph("shuffle", shuffle, &shuffle_label, 22.0, TAP, state.player.shuffle),
        Element::spacer().flex(1.0),
        glyph("previous", Icon::Custom(crate::view::glyph::BACKWARD), &t!("mobile-previous-song"), 42.0, 52.0, false),
        Element::spacer().flex(1.0),
        glyph("play-pause", if playing { Icon::PauseFill } else { Icon::PlayFill }, &if playing { t!("common-pause") } else { t!("common-play") }, 56.0, 72.0, false),
        Element::spacer().flex(1.0),
        glyph("next", Icon::Custom(crate::view::glyph::FORWARD), &t!("common-next-song"), 42.0, 52.0, false),
        Element::spacer().flex(1.0),
        glyph("repeat", if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, &t!("mobile-repeat"), 22.0, TAP, state.player.repeat != Repeat::Off),
    ])
}

/// How loud Surco plays, between its quiet and loud glyphs, as the timeline is drawn.
fn volume(state: &State, palette: ComponentPalette) -> Element {
    let glyph = |icon: &'static str| Element::icon(Icon::Custom(icon)).large_icon(16.0).foreground(SOFT).centered_y();
    let slider = Slider::new("volume", state.volume, ComponentPalette { selected: UNFILLED, ..palette }).thumb(0.0, 0.0).bar(7.0, 12.0).ink(FILLED)
        .accessibility_label(t!("mobile-volume")).render().size(None, Some(TAP)).flex(1.0);
    Element::row(vec![glyph(crate::view::glyph::VOLUME_LOW), slider, glyph(crate::view::glyph::VOLUME_HIGH)]).gap(10.0)
}

/// Where it plays, under the foot, as Apple Music names it; a line of room when it is not known.
fn device(state: &State, palette: ComponentPalette) -> Element {
    let named = state.mobile_ui.route.as_ref().map(|(name, _)| name.as_str()).filter(|name| !name.is_empty());
    named.map_or_else(Element::spacer, |name| Text::new(name, palette).font_size(12.0).ink(SOFT).truncate().render().centered_x()).size(None, Some(18.0))
}

/// The player's foot, the same under its cover and its lyrics: the lyrics' button, lit while
/// they are open, where it plays, the timer, and the queue (the lyrics share the song there).
pub(super) fn foot(state: &State, palette: ComponentPalette) -> Element {
    let open = state.page == Page::Lyrics;
    let lyrics = icon("lyrics", Icon::Custom(crate::view::LYRICS), &if open { t!("mobile-close-lyrics") } else { t!("mobile-show-lyrics") }, palette).radius(10.0);
    let last = if open {
        icon("do:share:current", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), &t!("common-share-song"), palette)
    } else { icon("queue", Icon::Playlist, &t!("mobile-show-queue"), palette) };
    let tools = [
        if open { lyrics.background(LIT) } else { lyrics },
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), &t!("mobile-connect-device"), palette),
        icon("sleep", Icon::Custom(crate::view::glyph::SLEEP), &crate::view::sleep_hint(state), palette),
        last,
    ];
    let mut row = vec![Element::spacer().flex(1.0)];
    for tool in tools { row.extend([tool, Element::spacer().flex(1.0)]); }
    Element::row(row)
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
        let ids = ["like", "more:current", "seek", "shuffle", "previous", "play-pause", "next", "repeat", "volume", "lyrics", "mobile-connections", "sleep"];
        for size in [Size::new(320.0, 568.0), Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            for page in [Page::Playing, Page::Lyrics] {
                state.page = page.clone();
                let mut tree = UiTree::new(crate::view::view(&state, size));
                tree.set_reduced_motion(true);
                let shown = tree.layout(size);
                let last = if page == Page::Playing { "queue" } else { "do:share:current" };
                for id in ids.iter().copied().filter(|id| page == Page::Playing || !matches!(*id, "like" | "more:current" | "volume")).chain([last]) {
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
    fn audio_route_button_uses_the_connection_glyph() {
        let shown = frame(&state());
        let button = shown.regions.iter().find(|region| region.id == "mobile-connections").unwrap();
        let expected = Icon::Custom(include_str!("../../../assets/icons/airplay-audio.svg"));
        let glyphs: Vec<_> = shown.commands.iter().filter_map(|command| match command {
            DrawCommand::Icon { rect, icon, .. } if button.rect.contains(limn::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)) => Some(*icon),
            _ => None,
        }).collect();
        assert_eq!(glyphs, [expected]);
    }

    #[test]
    fn an_unknown_duration_disables_seek() {
        let mut state = state();
        state.player.duration = 0.0;
        assert!(frame(&state).access.iter().any(|node| node.id == "seek" && node.disabled));
    }
}
