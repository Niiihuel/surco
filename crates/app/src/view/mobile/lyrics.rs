//! Full-screen lyrics chrome over the shared song and lyric state.

use super::*;

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
