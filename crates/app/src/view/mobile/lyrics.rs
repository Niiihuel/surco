//! Full-screen lyrics chrome over the shared song and lyric state, and its lines as BitChord's.

use super::*;
use surco_core::lyrics::Line;

/// A line's ink `distance` lines from the one being sung, and how out of focus it is, as
/// BitChord's: the line being sung whole and sharp, the rest a step dimmer and softer each.
/// Its blurs are CSS's, 1.08 to 1.89; limn's soft focus reads lighter at the same radius, so
/// these are half again as wide.
const FADE: [f32; 5] = [1.0, 0.8, 0.7, 0.58, 0.46];
const BLUR: [f32; 5] = [0.0, 1.6, 1.6, 2.2, 2.8];
/// The ink of what is still to be sung, the line being sung's included.
const UNSUNG: f32 = 0.45;
/// The soft edge the fill moves behind, in points.
const FEATHER: f32 = 30.0;
/// The faint light the line being sung gives off, and the lines either side of it fade from.
const GLOW: Color = Color::rgba(255, 255, 255, 56);
/// How far the word being sung rises at most, in points.
const RISE: f32 = 2.0;

/// BitChord's line (docs/bitchord-look.md): its words each their own text in a row that wraps
/// as the line would, so that the line being sung fills behind a soft edge over its unsung
/// ink as each word is sung, glowing faintly, the word being sung rising. A line timed only as
/// a whole lights at once, as BitChord's does. Away from it lines dim, blur and step back a
/// little; what was sung stays lit, what is to come does not.
// The lyrics' own measures and the frame's instant, passed as the desktop's lines take them.
#[allow(clippy::too_many_arguments)]
pub(in crate::view) fn line(state: &State, lines: &[Line], index: usize, sung: i64, size: f32, line_height: f32, gap: f32, now: std::time::Instant) -> Element {
    let line = &lines[index];
    let distance = index as i64 - sung;
    let far = distance.unsigned_abs().min(4) as usize;
    let (fade, blur) = (FADE[far], BLUR[far]);
    let base = if distance < 0 { 1.0 } else { UNSUNG };
    let playhead = state.playhead_ms(now);
    let filled = match distance {
        0 => surco_core::lyrics::revealed(line, playhead),
        ..0 => line.text.chars().count() as f32,
        _ => 0.0,
    };
    // The words of the line being sung rise as they are sung, and those of the line before
    // settle back; under reduced motion they stay.
    let moves = state.motion && !state.settings.reduce_motion && (-1..=0).contains(&distance);
    let rise = |number: usize| match line.words.get(number).filter(|_| moves) {
        Some(word) => -RISE * lift(word.start_ms as f32, word.end_ms as f32, playhead as f32),
        None => 0.0,
    };
    // The edge is so many points wide; a character of these lines is about 0.55 of their size.
    let feather = FEATHER / (size * 0.55);
    let ink = |at: f32| {
        let lit = base + (1.0 - base) * ((filled - at) / feather).clamp(0.0, 1.0);
        Color::rgba(255, 255, 255, (255.0 * lit).round() as u8)
    };
    let shade = Color::rgba(255, 255, 255, (255.0 * fade).round() as u8);
    // The words, as they are inked or, the same words laid out the same, as their glow.
    let words = |glow: bool| {
        let mut words = Vec::new();
        let mut at = 0.0;
        for (number, word) in line.text.split(' ').enumerate() {
            let len = word.chars().count() as f32;
            if len > 0.0 {
                let text = Element::text(word).font_family(FontFamily::SansBold).font_size(size).line_height(line_height);
                let text = match rise(number) { 0.0 => text, rise => text.translate(limn::Point::new(0.0, rise)) };
                // The glow comes up with the line and goes with it, rather than at once.
                words.push(if glow {
                    text.foreground(if distance == 0 { GLOW } else { Color::TRANSPARENT }).animate_foreground(format!("lyric-glow:{index}:{number}"))
                        .soft_focus(format!("lyric-glow-focus:{index}:{number}"), 4.0)
                } else {
                    let stops = (2.0 * len).clamp(2.0, 16.0) as usize;
                    let gradient: Vec<Color> = (0..stops).map(|stop| ink(at + len * stop as f32 / (stops - 1) as f32)).collect();
                    let text = if gradient.iter().all(|stop| *stop == gradient[0]) {
                        // limn paints a gradient as two dozen bands: only the word the edge
                        // crosses needs one. A lit word takes a spring of its own, so it does
                        // not dim back to its unsung ink as the edge leaves it.
                        let kind = if gradient[0].a == 255 { "lit" } else { "ink" };
                        text.foreground(Color::rgba(255, 255, 255, (fade * f32::from(gradient[0].a)).round() as u8))
                            .animate_foreground(format!("lyric-{kind}:{index}:{number}"))
                    } else { text.foreground(shade).foreground_gradient(gradient) };
                    text.soft_focus(format!("lyric:{index}:{number}"), blur)
                });
            }
            at += len + 1.0;
        }
        Element::row(words).wrap().gap(size * 0.25).line_gap(0.0)
    };
    let lit = if distance.abs() <= 1 { Element::stack(vec![words(true), words(false)]) } else { words(false) };
    // Its translation goes under it, smaller, and dims and blurs with it.
    let lit = match state.translated(index, &line.text) {
        Some(translation) => Element::column(vec![lit, Element::text(translation).font_family(FontFamily::Sans)
            .font_size(size * TRANSLATION_SCALE).line_height(size * TRANSLATION_SCALE * 1.25)
            .foreground(Color::rgba(255, 255, 255, (255.0 * fade * base).round() as u8)).animate_foreground(format!("lyric-translation-ink:{index}"))
            .soft_focus(format!("lyric-translation:{index}"), blur)]).gap(gap / 3.0),
        None => lit,
    };
    // Paint only, from the leading edge: the lines do not re-wrap as they step back.
    lit.animate_scale(format!("lyric-scale:{index}"), if distance == 0 { 1.0 } else { 0.98 })
}

/// How far a word stands lifted, from 0 to 1, as BitChord's: it rises as it is sung, a long
/// word further, and settles 700 ms after it ends.
fn lift(start: f32, end: f32, at: f32) -> f32 {
    let smooth = |x: f32| x * x * (3.0 - 2.0 * x);
    let length = end - start;
    let rising = ((at - start) / length.clamp(250.0, 700.0)).clamp(0.0, 1.0);
    let settling = (1.0 - (at - end) / 700.0).clamp(0.0, 1.0);
    (0.25 + 0.75 * smooth(((length - 150.0) / 750.0).clamp(0.0, 1.0))) * smooth(rising.min(settling))
}


/// The shell adds the device's bottom safe-area inset after this footer: the timeline and its
/// times, the transport, the volume and the foot, 8 apart and 8 inside.
pub(super) const FOOTER_HEIGHT: f32 = TAP + 16.0 + player::PLAY_TARGET + TAP + TAP + 3.0 * 8.0 + 16.0;
/// Below this height, as a phone on its side, the footer leaves its tools to the player so
/// that a wrapped line still fits between it and the header.
pub(super) const SHORT: f32 = 500.0;

/// The cover beside the song's name in the head, as BitChord's 54, and the grip over it; a
/// screen too short for both, as a phone on its side, has the way back beside a smaller one.
const ARTWORK: f32 = 54.0;
const GRIP: f32 = TAP;
const SHORT_ARTWORK: f32 = 40.0;

/// Where the head's cover stands on a tall screen whose safe edges are `insets`: where the
/// player's cover shrinks to.
pub(super) fn thumb(insets: crate::state::mobile::ContentInsets) -> limn::Rect {
    limn::Rect::new(insets.left + player::GUTTER, insets.top + GRIP + 4.0, ARTWORK, ARTWORK)
}

/// How tall the head over the lyrics and the queue stands in a screen `window` tall.
pub(super) fn head_height(window: f32) -> f32 {
    if window < SHORT { HEADER_HEIGHT } else { GRIP + ARTWORK + 8.0 }
}

/// BitChord's head over the lyrics and the queue: the grip that puts them away, and under it
/// the song as the player names it, beside its cover. The song names the page; there is no
/// title competing with it.
pub(super) fn header(state: &State, window: f32, palette: ComponentPalette) -> Element {
    // Over the queue the way back is to the song, as its lit button at the foot is.
    let queue = state.page == Page::Playing && state.queue_open;
    let label = if queue { t!("mobile-back-to-song") } else { t!("mobile-close-lyrics") };
    let back = if queue { "queue" } else { "back" };
    let short = window < SHORT;
    let close = if short {
        icon("mobile-lyrics-close", Icon::ChevronDown, &label, palette).dispatch_as(back)
    } else {
        let bar = Element::panel(Element::spacer()).background(Color::rgba(255, 255, 255, 150)).round().size(Some(36.0), Some(5.0)).centered();
        Element::clickable("mobile-lyrics-close", bar).dispatch_as(back).accessible(limn::Access::new(limn::Role::Button).name(label))
            .size(Some(120.0), Some(GRIP)).centered_x()
    };
    let Some(track) = state.current() else { return Element::column(vec![close, Element::spacer().flex(1.0)]) };
    let side = if short { SHORT_ARTWORK } else { ARTWORK };
    // While the player's cover is on its way here, its place waits for it.
    let picture = if super::cover_flight(state, window).is_some() { Element::spacer().size(Some(side), Some(side)) }
        // The player's own rendition of the cover, so that the one shrinking into this place
        // and this one are the same picture, decoded once.
        else { cover(state, &largest(state, track), side, palette).radius(8.0) };
    let song = [picture.centered_y(), player::song(state, track, palette).flex(1.0)];
    if short {
        let row = Element::row(std::iter::once(close.centered_y()).chain(song).collect()).gap(12.0);
        return horizontal_inset(row, 6.0, player::GUTTER - 6.0).size(None, Some(HEADER_HEIGHT));
    }
    let row = horizontal_inset(Element::row(song.into()).gap(12.0), player::GUTTER, player::GUTTER - 6.0).size(None, Some(side + 8.0));
    Element::column(vec![close, row])
}

/// Translates the lyrics into the app's language, or takes the translation away, as a round
/// button at the lines' foot as BitChord's; nothing while there is nothing to translate. Lit
/// while it is on.
fn translate(state: &State, palette: ComponentPalette) -> Element {
    if !state.translatable() { return Element::spacer().size(None, Some(TAP)); }
    let on = state.settings.translate_lyrics;
    let label = if on { t!("common-original-lyrics") } else { t!("common-translate-lyrics") };
    let button = player::circle("lyrics-translate", Icon::Custom(include_str!("../../../assets/icons/languages.svg")), &label, on, palette);
    Element::row(vec![Element::spacer().flex(1.0), button])
}

/// The footer's height in a screen `window` tall: what leads it, the lyrics' translation or
/// the queue's line being sung, over the player's controls.
pub(super) fn footer_height(state: &State, window: f32) -> f32 {
    // A short screen keeps the timeline and the transport alone.
    if window < SHORT { return FOOTER_HEIGHT - 2.0 * (TAP + 8.0); }
    FOOTER_HEIGHT + if state.page == Page::Lyrics { TAP + 8.0 } else { SUNG + 8.0 }
}

/// The line being sung's room over the queue's controls.
const SUNG: f32 = 24.0;

/// As BitChord's, the player's own controls, its volume and foot too, stay under the lyrics
/// and the queue.
pub(super) fn footer(state: &State, window: f32, palette: ComponentPalette) -> Element {
    let height = footer_height(state, window);
    if state.current().is_none() { return Element::spacer().size(None, Some(height)); }
    // As BitChord's: the translation's button over the lyrics' controls, the line being sung over the queue's.
    let lead = if state.page == Page::Lyrics { translate(state, palette) } else { player::sung_line(state, palette) };
    let controls = if window < SHORT { vec![player::timeline(state, palette), player::transport(state, palette)] }
        else { vec![lead, player::timeline(state, palette), player::transport(state, palette), player::volume(state, palette), player::foot(state, palette)] };
    horizontal_inset(Element::column(controls).gap(8.0).padding(8.0), player::GUTTER - 8.0, player::GUTTER - 8.0).size(None, Some(height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::Command, player::PlayerView};
    use limn::{DrawCommand, Frame, Point, Rect, UiAction, UiEvent, UiTree};
    use surco_core::lyrics::Line;

    fn state() -> State {
        let mut state = State::default();
        state.mobile = true;
        state.mobile_ui.content_insets = crate::state::mobile::ContentInsets {
            top: 59.0, bottom: 34.0, ..Default::default()
        };
        state.page = Page::Playing;
        state.show(PlayerView { current: Some(Track {
            id: "song".into(), title: "La canción actual".into(), artist: "Una artista".into(), artist_id: None,
            album: None, album_id: None, artwork_url: None, duration_ms: 180_000,
        }), playing: true, position: 30.0, duration: 180.0, ..PlayerView::default() });
        state.update(&UiAction::Clicked { id: "lyrics".into() });
        state
    }

    fn tree(state: &State, width: f32) -> UiTree {
        let mut tree = UiTree::new(super::super::view(state, Size::new(width, 844.0)));
        tree.set_reduced_motion(true);
        tree.layout(Size::new(width, 844.0));
        tree
    }

    fn target(frame: &Frame, id: &str) -> Rect {
        let nodes: Vec<_> = frame.access.iter().filter(|node| node.id == id).collect();
        assert_eq!(nodes.len(), 1, "one {id}");
        nodes[0].rect
    }

    fn click(tree: &mut UiTree, id: &str) -> Vec<UiAction> {
        let rect = target(tree.frame(), id);
        let point = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        tree.handle_event(UiEvent::PointerDown(point));
        tree.handle_event(UiEvent::PointerUp(point))
    }

    #[test]
    fn lyrics_chrome_keeps_full_touch_targets_and_a_centered_primary_circle_inside_safe_areas() {
        let state = state();
        for width in [320.0, 390.0, 430.0] {
            let tree = tree(&state, width);
            let frame = tree.frame();
            for id in ["mobile-lyrics-close", "more:current", "seek", "previous", "play-pause",
                "next", "mobile-connections", "queue"] {
                let rect = target(frame, id);
                assert!(rect.width >= TAP - 0.01 && rect.height >= TAP - 0.01, "{id}: {rect:?}");
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width);
                assert!(rect.y >= 59.0 && rect.y + rect.height <= 810.0, "{id} stays inside device insets");
            }
            let play = target(frame, "play-pause");
            assert!(play.width >= 72.0 && (play.x + play.width / 2.0 - width / 2.0).abs() < 0.5, "the bare play glyph leads, centred: {play:?}");
            for id in ["home", "mobile-search", "mobile-library", "mobile-playing", "settings"] {
                assert!(!frame.access.iter().any(|node| node.id == id));
            }
        }
    }

    #[test]
    fn lyrics_controls_dispatch_existing_play_seek_and_back_actions() {
        let mut state = state();
        let mut tree = tree(&state, 390.0);
        let play = click(&mut tree, "play-pause");
        assert_eq!(play, vec![UiAction::Clicked { id: "play-pause".into() }]);
        assert_eq!(state.update(&play[0]), vec![Command::PlayPause]);

        let seek = target(tree.frame(), "seek");
        let midpoint = Point::new(seek.x + seek.width / 2.0, seek.y + seek.height / 2.0);
        tree.handle_event(UiEvent::PointerDown(midpoint));
        let seek = tree.handle_event(UiEvent::PointerUp(midpoint));
        let committed = seek.iter().find(|action| matches!(action,
            UiAction::SliderChanged { id, committed: true, .. } if id == "seek")).unwrap();
        assert_eq!(state.update(committed), vec![Command::Seek(90.0)]);

        for (id, action) in [("previous", "previous"), ("next", "next"), ("more:current", "more:current")] {
            assert_eq!(click(&mut tree, id), vec![UiAction::Clicked { id: action.into() }]);
        }
        let close = click(&mut tree, "mobile-lyrics-close");
        assert_eq!(close, vec![UiAction::Clicked { id: "back".into() }]);
        state.update(&close[0]);
        assert_eq!(state.page, Page::Playing);
    }

    #[test]
    fn lyrics_route_and_queue_use_the_existing_connection_surface_and_the_players_queue() {
        let mut state = state();
        let mut tree = tree(&state, 390.0);
        let route = click(&mut tree, "mobile-connections");
        assert_eq!(route, vec![UiAction::Clicked { id: "mobile-connections".into() }]);
        state.update(&route[0]);
        assert!(state.mobile_ui.connections);
        let queue = click(&mut tree, "queue");
        assert_eq!(queue, vec![UiAction::Clicked { id: "queue".into() }]);
        state.update(&queue[0]);
        assert!(state.page == Page::Playing && state.queue_open, "the queue opens in the player, as BitChord's");
    }

    #[test]
    fn the_line_being_sung_fills_word_by_word_behind_a_soft_edge() {
        let mut state = state();
        state.page = Page::Lyrics;
        state.show(PlayerView { playing: false, position: 10.7, ..state.player.clone() });
        let words = surco_core::lyrics::words_from_runs(&[(10_000, 10_400, "uno "), (10_500, 11_000, "dos "), (11_100, 11_600, "tres")]);
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![
            Line { words: Vec::new(), at_ms: 1_000, text: "antes".into() },
            Line { words, at_ms: 10_000, text: "uno dos tres".into() },
            Line { words: Vec::new(), at_ms: 20_000, text: "después".into() },
        ])));
        let lines = state.lines().to_vec();
        let painted = |index: usize, word: &str| {
            let mut tree = UiTree::new(line(&state, &lines, index, 1, 34.0, 41.0, 25.0, std::time::Instant::now()));
            tree.set_reduced_motion(true);
            let frame = tree.layout(Size::new(360.0, 200.0));
            frame.commands.iter().filter_map(|command| match command {
                DrawCommand::Text { text, color, .. } if text == word => Some(color.a),
                _ => None,
            }).collect::<Vec<_>>()
        };
        // 0.2 s into "dos", sung from 10.5 s to 11 s. The glow's faint copies are left out.
        let sung = |word: &str| painted(1, word).into_iter().filter(|alpha| *alpha >= 40).collect::<Vec<_>>();
        assert_eq!(sung("uno"), [255], "sung");
        let edge = sung("dos");
        assert!(edge.len() > 1 && edge.first() > edge.last() && *edge.last().unwrap() == 115, "the edge crosses it, lit to unsung: {edge:?}");
        assert_eq!(sung("tres"), [115], "unsung, at 45%");
        // Blurred, each line is painted as faint copies: what was sung weighs more than what is to come.
        let weight = |index: usize, word: &str| painted(index, word).iter().map(|alpha| u32::from(*alpha)).sum::<u32>();
        assert!(weight(0, "antes") > 2 * weight(2, "después"), "what was sung stays lit");
        // Timed only as a whole, the line being sung is lit at once, as BitChord's.
        let mut tree = UiTree::new(line(&state, &lines, 0, 0, 34.0, 41.0, 25.0, std::time::Instant::now()));
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(360.0, 200.0));
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, color, .. } if text == "antes" && color.a == 255)));
    }

    #[test]
    fn a_word_rises_as_it_is_sung_and_settles_after() {
        assert_eq!(lift(1_000.0, 1_400.0, 900.0), 0.0, "not yet sung");
        let sung = lift(1_000.0, 1_400.0, 1_400.0);
        assert!(sung > 0.3 && sung <= 1.0, "{sung}");
        assert!(lift(1_000.0, 1_400.0, 1_700.0) < sung, "settling");
        assert_eq!(lift(1_000.0, 1_400.0, 2_100.0), 0.0, "settled 700 ms after");
        assert!(lift(1_000.0, 2_000.0, 2_000.0) > sung, "a longer word rises further");
    }

    #[test]
    fn lines_dim_and_blur_progressively_on_both_sides_of_the_line_being_sung() {
        assert_eq!(lyric_focus(0), (1.0, 0.0));
        for side in [1, -1] {
            let steps: Vec<_> = (1..12).map(|distance| lyric_focus(side * distance)).collect();
            for pair in steps.windows(2) {
                assert!(pair[1].0 <= pair[0].0 && pair[1].1 >= pair[0].1, "{side}: {pair:?}");
            }
            assert!(steps[0].0 < 1.0 && steps[0].0 > steps[10].0 && steps[10].0 > 0.1, "{steps:?}");
        }
        assert!(lyric_focus(1).0 > lyric_focus(-1).0, "what is coming reads ahead of what was sung");
    }

    #[test]
    fn the_lyrics_head_is_the_grip_over_the_players_song_beside_its_cover() {
        let state = state();
        let frame = tree(&state, 390.0);
        let frame = frame.frame();
        let grip = target(frame, "mobile-lyrics-close");
        let more = target(frame, "more:current");
        let title = frame.commands.iter().find_map(|command| match command {
            DrawCommand::Text { text, position, font_family, .. } if text.starts_with("La canción") => Some((*position, *font_family)),
            _ => None,
        }).unwrap();
        let (title, face) = title;
        assert_eq!(face, FontFamily::SansBold, "named as the player names it");
        assert!((grip.x + grip.width / 2.0 - 195.0).abs() < 0.5 && grip.y + grip.height <= more.y + 0.5, "the grip, centred over the song: {grip:?} {more:?}");
        assert!(title.x > 28.0 + 54.0, "the cover sits before the song: {title:?}");
        assert!(title.x < 390.0 / 3.0, "the song reads from the leading side, not centered: {title:?}");
        assert!(more.x + more.width > 390.0 - 28.0, "the ⋯ at the end: {more:?}");
        assert!(!frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Letra")));
    }

    #[test]
    fn lyrics_header_uses_real_metadata_and_unknown_duration_disables_seek() {
        let mut state = state();
        state.player.duration = 0.0;
        let shown = tree(&state, 390.0);
        assert!(shown.frame().access.iter().any(|node| node.id == "seek" && node.disabled));
        for expected in ["La canción", "Una artista"] {
            assert!(shown.frame().commands.iter().any(|command| matches!(command,
                DrawCommand::Text { text, .. } if text.starts_with(expected))), "{expected}");
        }
        let mut empty_state = State::default();
        empty_state.mobile = true;
        empty_state.page = Page::Lyrics;
        let empty = tree(&empty_state, 390.0);
        assert!(empty.frame().access.iter().all(|node| node.id == "mobile-lyrics-close"));
    }
}
