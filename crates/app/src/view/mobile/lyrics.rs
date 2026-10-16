//! Full-screen lyrics chrome over the shared song and lyric state.

use super::*;

/// The shell adds the device's bottom safe-area inset after this footer.
pub(super) const FOOTER_HEIGHT: f32 = 156.0;

pub(super) fn header(state: &State, palette: ComponentPalette) -> Element {
    let copy = match state.current() {
        Some(track) => Element::column(vec![
            Text::new(&track.title, palette).font_size(17.0).line_height(20.0).truncate().render().centered_x(),
            Text::new(&track.artist, palette).font_size(12.0).line_height(16.0).muted().truncate().render().centered_x(),
        ]).gap(2.0),
        None => Text::new("Letra", palette).font_size(17.0).line_height(20.0).render().centered_x(),
    };
    horizontal_inset(Element::row(vec![
        icon("mobile-lyrics-close", Icon::ChevronDown, "Cerrar letra", palette).dispatch_as("back"),
        copy.centered_y().flex(1.0),
        Element::spacer().size(Some(TAP), None),
    ]).gap(8.0).padding(6.0), 10.0, 10.0)
        .size(None, Some(HEADER_HEIGHT)).clip_children()
}

pub(super) fn footer(state: &State, palette: ComponentPalette) -> Element {
    if state.current().is_none() { return Element::spacer().size(None, Some(FOOTER_HEIGHT)); }
    let transport = Element::row(vec![
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), "Conectar a un dispositivo", palette).centered_y(),
        Element::spacer().flex(1.0),
        controls::playback("mobile-lyrics-play", state.player.playing, 64.0, 28.0, palette).dispatch_as("play-pause"),
        Element::spacer().flex(1.0),
        icon("do:share:current", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), "Compartir canción", palette).centered_y(),
    ]);
    horizontal_inset(Element::column(vec![player::timeline(state, palette), transport]).gap(8.0).padding(8.0), 16.0, 16.0)
        .size(None, Some(FOOTER_HEIGHT))
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
            for id in ["mobile-lyrics-close", "seek", "mobile-lyrics-play", "mobile-connections", "do:share:current"] {
                let rect = target(frame, id);
                assert!(rect.width >= TAP && rect.height >= TAP, "{id}: {rect:?}");
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width);
                assert!(rect.y >= 59.0 && rect.y + rect.height <= 810.0, "{id} stays inside device insets");
            }
            let play = target(frame, "mobile-lyrics-play");
            assert_eq!((play.width, play.height), (64.0, 64.0));
            assert_eq!(play.x + play.width / 2.0, width / 2.0);
            assert!(frame.commands.iter().any(|command| matches!(command,
                DrawCommand::FillRect { rect, color, radius } if *color == Color::WHITE
                && rect.width == 64.0 && rect.height == 64.0 && *radius == 32.0)));
            for id in ["home", "mobile-search", "mobile-library", "mobile-playing", "settings"] {
                assert!(!frame.access.iter().any(|node| node.id == id));
            }
        }
    }

    #[test]
    fn lyrics_controls_dispatch_existing_play_seek_and_back_actions() {
        let mut state = state();
        let mut tree = tree(&state, 390.0);
        let play = click(&mut tree, "mobile-lyrics-play");
        assert_eq!(play, vec![UiAction::Clicked { id: "play-pause".into() }]);
        assert_eq!(state.update(&play[0]), vec![Command::PlayPause]);

        let seek = target(tree.frame(), "seek");
        let midpoint = Point::new(seek.x + seek.width / 2.0, seek.y + seek.height / 2.0);
        tree.handle_event(UiEvent::PointerDown(midpoint));
        let seek = tree.handle_event(UiEvent::PointerUp(midpoint));
        let committed = seek.iter().find(|action| matches!(action,
            UiAction::SliderChanged { id, committed: true, .. } if id == "seek")).unwrap();
        assert_eq!(state.update(committed), vec![Command::Seek(90.0)]);

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
