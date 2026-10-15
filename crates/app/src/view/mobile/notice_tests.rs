//! Notices share the protected body viewport, including static lyric states.

use super::*;
use crate::{player::PlayerView, state::mobile::ContentInsets};
use limn::{DrawCommand, Frame, HitKind, Point, Rect, UiAction, UiEvent, UiTree};

fn phone() -> State {
    let mut state = State::default();
    state.mobile = true;
    state.player = PlayerView {
        current: Some(Track { id: "notice-song".into(), title: "Canción".into(), artist: "Artista".into(),
            artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 }),
        ..PlayerView::default()
    };
    state
}

fn target(frame: &Frame, id: &str) -> Rect {
    frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("visible {id}")).rect
}

fn header_bottom(frame: &Frame) -> f32 {
    let header = frame.regions.iter().find(|region| region.id == "mobile-header-bar").unwrap().rect;
    header.y + header.height
}

fn assert_close_inside_body(frame: &Frame, size: Size, insets: ContentInsets) -> Point {
    let close = target(frame, "mobile-notice-close");
    let footer = frame.regions.iter().find(|region| region.id == "mobile-footer-bar").unwrap().rect;
    assert_eq!((close.width, close.height), (44.0, 44.0));
    assert!(close.x >= insets.left && close.x + close.width <= size.width - insets.right + 0.01);
    assert!(close.y >= header_bottom(frame) && close.y + close.height <= footer.y + 0.01,
        "the notice action stays between the real chrome bars: {close:?}");
    let point = Point::new(close.x + 22.0, close.y + 22.0);
    assert_eq!(frame.press_region(point).unwrap().id, "mobile-notice-close");
    point
}

#[test]
fn notices_clear_the_safe_header_and_dismiss_through_their_real_touch_target() {
    let size = Size::new(402.0, 874.0);
    let insets = ContentInsets { top: 62.0, bottom: 34.0, ..ContentInsets::default() };
    for page in [Page::Library, Page::Playing, Page::Lyrics] {
        let mut state = phone();
        state.page = page;
        state.mobile_ui.content_insets = insets;
        state.mobile_ui.notice = Some("No pudimos completar esta acción.".into());
        state.lyrics = Some(("notice-song".into(), Lyrics::Loading));
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        let point = assert_close_inside_body(&frame, size, insets);
        tree.handle_event(UiEvent::PointerDown(point));
        let actions = tree.handle_event(UiEvent::PointerUp(point));
        assert_eq!(actions, vec![UiAction::Clicked { id: "mobile-notice-close".into() }]);
        for action in actions { assert!(state.update(&action).is_empty()); }
        assert!(state.mobile_ui.notice.is_none());
        tree.set_root(super::view(&state, size));
        assert!(!tree.layout(size).access.iter().any(|node| node.id == "mobile-notice-close"));
    }
}

#[test]
fn long_notices_keep_all_their_words_and_scroll_in_loading_missing_and_loaded_lyrics() {
    let message = (0..300).map(|index| format!("AVISO_{index:03}")).collect::<Vec<_>>().join(" ");
    for (size, insets) in [
        (Size::new(402.0, 874.0), ContentInsets { top: 62.0, bottom: 34.0, ..ContentInsets::default() }),
        (Size::new(874.0, 402.0), ContentInsets { left: 62.0, right: 34.0, bottom: 21.0, top: 0.0 }),
    ] {
        for lyrics in [Lyrics::Loading, Lyrics::Missing,
            Lyrics::Plain { lines: vec!["Letra real".into()], source: "Prueba".into() }] {
            let mut state = phone();
            state.page = Page::Lyrics;
            state.mobile_ui.content_insets = insets;
            state.mobile_ui.notice = Some(message.clone());
            state.lyrics = Some(("notice-song".into(), lyrics));
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let painted = frame.commands.iter().filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text.as_str()), _ => None,
            }).collect::<Vec<_>>().join(" ");
            let words: Vec<_> = painted.split_whitespace().filter(|word| word.starts_with("AVISO_")).collect();
            assert_eq!(words.join(" "), message, "the notice is wrapped, never truncated");
            let scrolls: Vec<_> = frame.regions.iter().filter(|region| region.kind == HitKind::Scroll).collect();
            assert_eq!(scrolls.len(), 1, "use the body scroll or one temporary scroll, never nest them");
            let scroll = scrolls[0].id.clone();
            tree.scroll_to_end(&scroll);
            tree.layout(size);
            assert!(tree.scroll_offset(&scroll) > 0.0, "all of a long notice remains reachable");
            tree.reveal_at_start("mobile-notice-close", insets.top + HEADER_HEIGHT + 20.0);
            tree.layout(size);
            let frame = tree.layout(size);
            assert_close_inside_body(&frame, size, insets);
        }
    }
}
