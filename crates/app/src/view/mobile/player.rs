//! Phone player content, as BitChord's: one screen and no page to scroll, the cover in all the
//! room the controls leave. The mobile shell owns its header, the blurred cover it stands on
//! and its dismissal gesture.

use super::*;

/// The player's sides, and the widest its column grows. BitChord's are 30; 28 lets the
/// transport fit a 320-point phone.
const GUTTER: f32 = 28.0;
const COLUMN_MAX: f32 = 560.0;
/// What stands under the cover: the song's row, the line being sung, the timeline, the
/// transport and the foot, with the gaps between them.
pub(super) const CONTROLS: f32 = 330.0;
/// The artist and the clock, white at 55% over the blurred cover.
const SOFT: Color = Color::rgba(255, 255, 255, 140);
/// The round buttons beside the song, white at 18%.
const VEIL: Color = Color::rgba(255, 255, 255, 46);
/// A tool that is on, as shuffle while it shuffles.
const LIT: Color = Color::rgba(255, 255, 255, 40);

pub(super) fn screen(state: &State, width: f32, height: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(blank(Icon::MusicNote, t!("mobile-no-music"), t!("mobile-no-music-detail"), None, palette));
    };
    let gutter = GUTTER.max((width - COLUMN_MAX) / 2.0);
    let side = (width - 2.0 * gutter).min(height - CONTROLS).max(120.0);
    // It steps back while paused, as BitChord's.
    let rest = side * state.cover_rest(std::time::Instant::now());
    let cover = sole_cover(state, &largest(state, track), rest, palette).radius(8.0)
        .animate_enter(format!("mobile-player-cover:{}", track.id));
    let stage = Element::column(vec![Element::spacer().flex(1.0), cover.centered_x(), Element::spacer().flex(1.0)])
        .size(None, Some(side)).flex(1.0);
    let liked = state.is_liked(&track.id);
    let details = Element::row(vec![
        Element::column(vec![
            Text::new(&track.title, palette).font_size(20.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(&track.artist, palette).font_size(18.0).ink(SOFT).truncate().render().font_family(FontFamily::SansMedium),
        ]).gap(2.0).flex(1.0).centered_y(),
        circle("like", if liked { Icon::HeartFill } else { Icon::Heart }, &if liked { t!("mobile-unlike") } else { t!("common-like") }, palette),
        circle("more:current", Icon::More, &t!("common-song-options"), palette),
    ]).gap(6.0).anchor("mobile-player-title");
    let mut controls = vec![details];
    match &state.player.error {
        Some(error) => controls.push(Alert::new(t!("mobile-play-failed"), error, palette).id("mobile-player-error").render()),
        None => controls.push(sung_line(state, palette)),
    }
    controls.extend([timeline(state, palette), transport(state, palette), foot(state, palette)]);
    let body = Element::column(vec![stage, Element::column(controls).gap(14.0)]).gap(20.0).padding(12.0).flex(1.0);
    horizontal_inset(body, gutter - 12.0, gutter - 12.0)
}

/// A round button beside the song: a 34 disc on its 44 touch target.
fn circle(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    let disc = Element::panel(Element::icon(glyph).large_icon(18.0).foreground(palette.text).centered())
        .background(VEIL).pressed_background(LIT).round().size(Some(34.0), Some(34.0));
    Element::clickable(id, disc.centered()).accessible(limn::Access::new(limn::Role::Button).name(label))
        .radius(TAP / 2.0).size(Some(TAP), Some(TAP)).centered_y()
}

/// The line being sung, on one line under the song, as BitChord's strip; a line of room
/// whether or not there is one, so the controls under it do not jump.
fn sung_line(state: &State, palette: ComponentPalette) -> Element {
    let line = state.active_line().and_then(|index| state.lines().get(index).map(|line| (index, line)))
        .filter(|(_, line)| !line.text.trim().is_empty());
    let words = line.map_or_else(Element::spacer, |(index, line)| Text::new(&line.text, palette).font_size(15.0).ink(SOFT).truncate().render()
        .font_family(FontFamily::SansMedium).animate_enter(format!("mobile-sung-line:{index}")));
    words.size(None, Some(20.0))
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
    let track = ComponentPalette { selected: Color::rgba(255, 255, 255, 66), ..palette };
    Element::column(vec![
        Slider::new("seek", progress, track).enabled(duration > 0.0).thumb(0.0, 0.0).bar(7.0, 12.0).ink(Color::rgba(255, 255, 255, 235))
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
        glyph("previous", Icon::SkipBack, &t!("mobile-previous-song"), 34.0, 52.0, false),
        Element::spacer().flex(1.0),
        glyph("play-pause", if playing { Icon::PauseFill } else { Icon::PlayFill }, &if playing { t!("common-pause") } else { t!("common-play") }, 54.0, 72.0, false),
        Element::spacer().flex(1.0),
        glyph("next", Icon::SkipForward, &t!("common-next-song"), 34.0, 52.0, false),
        Element::spacer().flex(1.0),
        glyph("repeat", if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, &t!("mobile-repeat"), 22.0, TAP, state.player.repeat != Repeat::Off),
    ])
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
        let mut tree = UiTree::new(screen(state, 390.0, 780.0, SCHEME.palette()));
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
        let ids = ["like", "more:current", "seek", "shuffle", "previous", "play-pause", "next", "repeat", "lyrics", "mobile-connections", "sleep"];
        for size in [Size::new(320.0, 568.0), Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            for page in [Page::Playing, Page::Lyrics] {
                state.page = page.clone();
                let mut tree = UiTree::new(crate::view::view(&state, size));
                tree.set_reduced_motion(true);
                let shown = tree.layout(size);
                let last = if page == Page::Playing { "queue" } else { "do:share:current" };
                for id in ids.iter().copied().filter(|id| page == Page::Playing || !matches!(*id, "like" | "more:current")).chain([last]) {
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

        let later = std::time::Instant::now() + std::time::Duration::from_secs(3);
        assert_eq!(state.cover_rest(later), 1.0, "whole while it plays");
        let paused = PlayerView { playing: false, ..state.player.clone() };
        state.show(paused);
        assert!((state.cover_rest(later) - 0.82).abs() < 0.01, "stepped back while paused");
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
