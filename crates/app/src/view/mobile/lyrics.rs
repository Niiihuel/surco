//! Full-screen lyrics chrome over the shared song and lyric state, and its lines as BitChord's.

use super::*;
use surco_core::lyrics::Line;

/// A line's ink `distance` lines from the one being sung, and how out of focus it is, as
/// BitChord's: the line being sung whole and sharp, the rest a step dimmer and softer each.
const FADE: [f32; 5] = [1.0, 0.8, 0.7, 0.58, 0.46];
const BLUR: [f32; 5] = [0.0, 1.08, 1.08, 1.48, 1.89];
/// The ink of what is still to be sung, the line being sung's included.
const UNSUNG: f32 = 0.45;
/// The soft edge the fill moves behind, in points.
const FEATHER: f32 = 30.0;
/// The faint light the line being sung gives off, and the lines either side of it fade from.
const GLOW: Color = Color::rgba(255, 255, 255, 56);
/// How far the word being sung rises at most, in points.
const RISE: f32 = 2.0;
/// A steady pace of singing, per character: a line before a long rest fills at it, rather
/// than across the rest.
const PER_CHAR_MS: f32 = 80.0;

/// BitChord's line (docs/bitchord-look.md): its words each their own text in a row that wraps
/// as the line would, so that the line being sung fills word by word behind a soft edge over
/// its unsung ink, glowing faintly. Away from it lines dim, blur and step back a little; what
/// was sung stays lit, what is to come does not.
pub(in crate::view) fn line(state: &State, lines: &[Line], index: usize, sung: i64, size: f32, line_height: f32, gap: f32, now: std::time::Instant) -> Element {
    let line = &lines[index];
    let distance = index as i64 - sung;
    let far = distance.unsigned_abs().min(4) as usize;
    let (fade, blur) = (FADE[far], BLUR[far]);
    let base = if distance < 0 { 1.0 } else { UNSUNG };
    let chars = line.text.chars().count().max(1) as f32;
    let (playhead, span) = (state.playhead_ms(now), sweep_ms(lines, index, chars));
    let filled = match distance {
        0 => (playhead.saturating_sub(line.at_ms) as f32 / span).min(1.0) * chars,
        ..0 => chars,
        _ => 0.0,
    };
    // The words of the line being sung rise as they are sung, and those of the line before
    // settle back; under reduced motion they stay.
    let moves = state.motion && !state.settings.reduce_motion && (-1..=0).contains(&distance);
    let rise = |from: f32, to: f32| {
        if !moves { return 0.0; }
        let at = |share: f32| line.at_ms as f32 + span * share / chars;
        -RISE * lift(at(from), at(to), playhead as f32)
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
                let text = match rise(at, at + len) { 0.0 => text, rise => text.translate(limn::Point::new(0.0, rise)) };
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

/// How long a line takes to fill: until the next starts, though no longer than its words take
/// at a steady pace.
// shortcut: lyrics come timed by line, not by word, so the fill moves evenly through a line;
// follow each word once the lyrics carry their timing.
fn sweep_ms(lines: &[Line], index: usize, chars: f32) -> f32 {
    let until = lines.get(index + 1).map_or(f32::MAX, |next| next.at_ms.saturating_sub(lines[index].at_ms) as f32);
    until.min(chars * PER_CHAR_MS + 400.0).max(1.0)
}

/// The shell adds the device's bottom safe-area inset after this footer.
pub(super) const FOOTER_HEIGHT: f32 = 212.0;
/// Below this height, as a phone on its side, the footer leaves its tools to the player so
/// that a wrapped line still fits between it and the header.
const SHORT: f32 = 500.0;

/// The cover beside the song's name in the header.
const ARTWORK: f32 = 40.0;

/// A compact row, as in a music app's lyrics: the way back, the song by its cover, and its
/// options. The song names the page; there is no title competing with it.
pub(super) fn header(state: &State, palette: ComponentPalette) -> Element {
    let close = icon("mobile-lyrics-close", Icon::ChevronDown, &t!("mobile-close-lyrics"), palette).dispatch_as("back");
    let row = match state.current() {
        Some(track) => vec![
            close,
            cover(state, &track.artwork_url, ARTWORK, palette).radius(6.0).centered_y(),
            Element::column(vec![
                Text::new(&track.title, palette).font_size(15.0).line_height(19.0).truncate().render()
                    .font_family(FontFamily::SansMedium),
                Text::new(&track.artist, palette).font_size(13.0).line_height(17.0).muted().truncate().render(),
            ]).gap(1.0).centered_y().flex(1.0),
            translate(state, palette),
            // BitChord's lyrics keep the song's heart beside its name.
            {
                let liked = state.is_liked(&track.id);
                icon("like", if liked { Icon::HeartFill } else { Icon::Heart }, &if liked { t!("mobile-unlike") } else { t!("common-like") }, palette)
            },
            icon("mobile-lyrics-more", Icon::More, &t!("common-song-options"), palette).dispatch_as("more:current"),
        ],
        None => vec![close, Element::spacer().flex(1.0)],
    };
    horizontal_inset(Element::row(row).gap(6.0).padding(6.0), 6.0, 6.0)
        .size(None, Some(HEADER_HEIGHT)).clip_children()
}

/// Translates the lyrics into the app's language, or takes the translation away; nothing
/// while there is nothing to translate. Lit while it is on.
fn translate(state: &State, palette: ComponentPalette) -> Element {
    if !state.translatable() { return Element::spacer().size(Some(0.0), Some(0.0)); }
    let on = state.settings.translate_lyrics;
    let label = if on { t!("common-original-lyrics") } else { t!("common-translate-lyrics") };
    // Round, so that a press lights a circle and not the square around it.
    let button = icon("lyrics-translate", Icon::Custom(include_str!("../../../assets/icons/languages.svg")), &label, palette).radius(22.0);
    if on { Element::panel(button).background(Color::rgba(255, 255, 255, 40)).radius(22.0).centered_y() } else { button.centered_y() }
}

pub(super) fn footer_height(window: f32) -> f32 {
    if window < SHORT { FOOTER_HEIGHT - TAP - 8.0 } else { FOOTER_HEIGHT }
}

/// As BitChord's, the player's own controls, its foot too, stay under the lyrics.
pub(super) fn footer(state: &State, window: f32, palette: ComponentPalette) -> Element {
    let height = footer_height(window);
    if state.current().is_none() { return Element::spacer().size(None, Some(height)); }
    let mut controls = vec![player::timeline(state, palette), player::transport(state, palette)];
    if window >= SHORT { controls.push(player::foot(state, palette)); }
    horizontal_inset(Element::column(controls).gap(8.0).padding(8.0), 16.0, 16.0).size(None, Some(height))
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
            for id in ["mobile-lyrics-close", "mobile-lyrics-more", "seek", "previous", "play-pause",
                "next", "mobile-connections", "do:share:current"] {
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

        for (id, action) in [("previous", "previous"), ("next", "next"), ("mobile-lyrics-more", "more:current")] {
            assert_eq!(click(&mut tree, id), vec![UiAction::Clicked { id: action.into() }]);
        }
        let close = click(&mut tree, "mobile-lyrics-close");
        assert_eq!(close, vec![UiAction::Clicked { id: "back".into() }]);
        state.update(&close[0]);
        assert_eq!(state.page, Page::Playing);
    }

    #[test]
    fn lyrics_route_and_share_use_the_current_track_and_existing_connection_surface() {
        let mut state = state();
        let mut tree = tree(&state, 390.0);
        let route = click(&mut tree, "mobile-connections");
        assert_eq!(route, vec![UiAction::Clicked { id: "mobile-connections".into() }]);
        state.update(&route[0]);
        assert!(state.mobile_ui.connections);
        let share = click(&mut tree, "do:share:current");
        assert_eq!(share, vec![UiAction::Clicked { id: "do:share:current".into() }]);
        state.update(&share[0]);
        assert_eq!(state.sharing.as_ref(), state.current());
    }

    #[test]
    fn the_line_being_sung_fills_word_by_word_behind_a_soft_edge() {
        let mut state = state();
        state.page = Page::Lyrics;
        state.show(PlayerView { playing: false, position: 10.6, ..state.player.clone() });
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![
            Line { at_ms: 1_000, text: "antes".into() },
            Line { at_ms: 10_000, text: "uno dos tres".into() },
            Line { at_ms: 20_000, text: "después".into() },
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
        // 0.6 s into a line of twelve characters that takes 1.36 s: past "uno", into "dos".
        // The glow's faint copies are left out.
        let sung = |word: &str| painted(1, word).into_iter().filter(|alpha| *alpha >= 40).collect::<Vec<_>>();
        assert_eq!(sung("uno"), [255], "sung");
        let edge = sung("dos");
        assert!(edge.len() > 1 && edge.first() > edge.last() && *edge.last().unwrap() == 115, "the edge crosses it, lit to unsung: {edge:?}");
        assert_eq!(sung("tres"), [115], "unsung, at 45%");
        // Blurred, each line is painted as faint copies: what was sung weighs more than what is to come.
        let weight = |index: usize, word: &str| painted(index, word).iter().map(|alpha| u32::from(*alpha)).sum::<u32>();
        assert!(weight(0, "antes") > 2 * weight(2, "después"), "what was sung stays lit");
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
    fn lyrics_header_is_a_compact_song_row_beside_its_cover() {
        let state = state();
        let frame = tree(&state, 390.0);
        let frame = frame.frame();
        let close = target(frame, "mobile-lyrics-close");
        let more = target(frame, "mobile-lyrics-more");
        let title = frame.commands.iter().find_map(|command| match command {
            DrawCommand::Text { text, position, font_family, .. } if text == "La canción actual" => Some((*position, *font_family)),
            _ => None,
        }).unwrap();
        let (title, face) = title;
        assert_eq!(face, FontFamily::SansMedium);
        assert!(title.x > close.x + close.width + 40.0, "the cover sits between the way back and the song");
        assert!(title.x < 390.0 / 3.0, "the song reads from the leading side, not centered: {title:?}");
        assert!(more.x + more.width > 390.0 - 16.0 && (more.y - close.y).abs() < 0.5);
        assert!(!frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Letra")));
    }

    #[test]
    fn lyrics_header_uses_real_metadata_and_unknown_duration_disables_seek() {
        let mut state = state();
        state.player.duration = 0.0;
        let shown = tree(&state, 390.0);
        assert!(shown.frame().access.iter().any(|node| node.id == "seek" && node.disabled));
        for expected in ["La canción actual", "Una artista"] {
            assert!(shown.frame().commands.iter().any(|command| matches!(command,
                DrawCommand::Text { text, .. } if text == expected)));
        }
        let mut empty_state = State::default();
        empty_state.mobile = true;
        empty_state.page = Page::Lyrics;
        let empty = tree(&empty_state, 390.0);
        assert!(empty.frame().access.iter().all(|node| node.id == "mobile-lyrics-close"));
    }
}
