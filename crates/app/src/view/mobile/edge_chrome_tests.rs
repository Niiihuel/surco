//! Regressions for full-window paint with controls inside the phone's safe edges.

use super::*;
use crate::player::PlayerView;
use crate::state::mobile::ContentInsets;
use limn::{DrawCommand, Frame, HitKind, Point, Rect, UiAction, UiEvent, UiTree};

fn phone() -> State {
    let track = Track {
        id: "current".into(),
        title: "Canción actual".into(),
        artist: "Artista de prueba".into(),
        artist_id: None,
        album: None,
        album_id: None,
        artwork_url: Some("edge-cover".into()),
        duration_ms: 180_000,
    };
    let mut state = State::default();
    state.mobile = true;
    state.player = PlayerView {
        current: Some(track),
        playing: true,
        position: 30.0,
        duration: 180.0,
        ..PlayerView::default()
    };
    state
        .tints
        .insert("edge-cover".into(), Color::rgb(210, 105, 45));
    state
}

fn screens() -> [(Size, ContentInsets); 3] {
    [
        (
            Size::new(402.0, 874.0),
            ContentInsets {
                top: 62.0,
                bottom: 34.0,
                ..ContentInsets::default()
            },
        ),
        (
            Size::new(874.0, 402.0),
            ContentInsets {
                left: 62.0,
                right: 34.0,
                bottom: 21.0,
                top: 0.0,
            },
        ),
        (
            Size::new(874.0, 402.0),
            ContentInsets {
                left: 34.0,
                right: 62.0,
                bottom: 21.0,
                top: 0.0,
            },
        ),
    ]
}

fn layout(state: &State, size: Size) -> (UiTree, Frame) {
    let mut tree = UiTree::new(super::view(state, size));
    tree.set_reduced_motion(true);
    let frame = tree.layout(size);
    (tree, frame)
}

fn target(frame: &Frame, id: &str) -> Rect {
    frame
        .access
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("visible control {id}"))
        .rect
}

fn surface(frame: &Frame, id: &str) -> Rect {
    frame
        .regions
        .iter()
        .find(|region| region.id == id && region.kind == HitKind::Surface)
        .unwrap_or_else(|| panic!("visible chrome surface {id}"))
        .rect
}

fn center(rect: Rect) -> Point {
    Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x - 0.01
        && inner.y >= outer.y - 0.01
        && inner.x + inner.width <= outer.x + outer.width + 0.01
        && inner.y + inner.height <= outer.y + outer.height + 0.01
}

#[test]
fn safe_edges_keep_header_tabs_and_mini_controls_touchable_in_both_orientations() {
    for (size, insets) in screens() {
        for page in [Page::Search, Page::Library, Page::Settings, Page::Playing] {
            let mut state = phone();
            state.mobile_ui.content_insets = insets;
            // Settings is reached through its real navigation action, including Back.
            if page == Page::Settings {
                state.update(&UiAction::Clicked {
                    id: "settings".into(),
                });
            } else {
                state.page = page.clone();
            }
            let (_, frame) = layout(&state, size);
            let safe = Rect::new(
                insets.left,
                insets.top,
                size.width - insets.left - insets.right,
                size.height - insets.top - insets.bottom,
            );
            let ids = match page {
                Page::Playing => vec!["mobile-player-dismiss", "mobile-player-header-more"],
                Page::Settings => vec![
                    "back",
                    "home",
                    "mobile-search",
                    "mobile-library",
                    "mobile-mini-like",
                    "play-pause",
                    "mobile-connections",
                ],
                _ => vec![
                    "settings",
                    "home",
                    "mobile-search",
                    "mobile-library",
                    "mobile-mini-like",
                    "play-pause",
                    "mobile-connections",
                ],
            };
            for id in ids {
                let rect = target(&frame, id);
                assert!(
                    rect.width >= 44.0 && rect.height >= 44.0,
                    "{id} remains touch sized on {page:?}: {rect:?}"
                );
                assert!(
                    inside(rect, safe),
                    "{id} clears {insets:?} on {page:?}: {rect:?}"
                );
                let hit = frame
                    .press_region(center(rect))
                    .expect("the visible control receives its own tap");
                assert_eq!(hit.kind, HitKind::Button, "{id} is not covered by chrome");
                assert_eq!(hit.id, id, "another control must not steal {id}'s tap");
            }
            if page != Page::Playing {
                let tabs: Vec<_> = ["home", "mobile-search", "mobile-library"]
                    .into_iter()
                    .map(|id| target(&frame, id))
                    .collect();
                for tab in &tabs {
                    assert!(
                        (tab.y + tab.height - (size.height - insets.bottom)).abs() < 0.1,
                        "navigation finishes immediately above the home-indicator inset: {tab:?}"
                    );
                    assert!(
                        tab.height <= 60.0,
                        "a compact tab bar must not grow into a second bottom inset"
                    );
                }
                for pair in tabs.windows(2) {
                    assert!(
                        pair[0].x + pair[0].width <= pair[1].x + 0.01,
                        "tabs do not overlap"
                    );
                }
                assert!(
                    target(&frame, "play-pause").y + target(&frame, "play-pause").height
                        <= tabs[0].y,
                    "mini transport has its own row above navigation"
                );
            }
        }
    }
}

#[test]
fn tapping_the_mini_heart_likes_the_current_song_without_opening_the_player() {
    let mut state = phone();
    state.collection.heard = vec![Track {
        id: "other-visible-song".into(),
        ..state.current().unwrap().clone()
    }];
    state.mobile_ui.content_insets = screens()[0].1;
    let size = screens()[0].0;
    let original_page = state.page.clone();
    for liked in [true, false] {
        let (mut tree, frame) = layout(&state, size);
        let point = center(target(&frame, "mobile-mini-like"));
        tree.handle_event(UiEvent::PointerDown(point));
        let actions = tree.handle_event(UiEvent::PointerUp(point));
        assert_eq!(actions, vec![UiAction::Clicked { id: "like".into() }]);
        for action in actions {
            assert!(state.update(&action).is_empty());
        }
        assert_eq!(state.is_liked("current"), liked);
        assert!(
            !state.is_liked("other-visible-song"),
            "the shelf's first card is not the playing track"
        );
        assert_eq!(
            state.page, original_page,
            "Like never dispatches mobile-playing"
        );
        assert!(state.player.playing);
    }
}

fn full_window_gradient(frame: &Frame, size: Size) -> (Color, Color) {
    frame
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::FillVerticalGradient { rect, top, bottom }
                if *rect == Rect::new(0.0, 0.0, size.width, size.height) =>
            {
                Some((*top, *bottom))
            }
            _ => None,
        })
        .expect("a painted surface covers the entire canvas, including status/home-indicator areas")
}

#[test]
fn player_and_every_lyrics_result_paint_the_cover_color_through_both_safe_edges() {
    let results = [
        Lyrics::Loading,
        Lyrics::Plain {
            lines: vec!["Palabras sin sincronizar".into()],
            source: "Prueba".into(),
        },
        Lyrics::Missing,
        Lyrics::Lines(vec![surco_core::lyrics::Line {
            at_ms: 0,
            text: "Una línea sincronizada".into(),
        }]),
    ];
    for (size, insets) in screens() {
        for page in [Page::Playing, Page::Lyrics] {
            for lyrics in &results {
                let mut state = phone();
                state.page = page.clone();
                state.mobile_ui.content_insets = insets;
                state.lyrics = Some(("current".into(), lyrics.clone()));
                let (_, frame) = layout(&state, size);
                let colors = full_window_gradient(&frame, size);
                assert_eq!(
                    colors,
                    page_surface(&state),
                    "the shell owns the surface in {page:?}/{lyrics:?}"
                );
                assert_ne!(
                    colors.0, SCHEME.canvas,
                    "the status area keeps the cover tone"
                );
                assert_ne!(
                    colors.1, SCHEME.canvas,
                    "the bottom safe area must not turn into a black band"
                );
                assert_eq!((colors.0.a, colors.1.a), (255, 255));
                assert!(frame.commands.iter().any(|command| matches!(command,
                    DrawCommand::PushClip(rect) if *rect == Rect::new(0.0, 0.0, size.width, size.height))),
                    "the canvas clip includes both safe edges");
            }
        }
    }
}

#[test]
fn scrolled_song_pages_join_the_header_tone_with_short_fades_outside_the_bars() {
    let size = Size::new(402.0, 700.0);
    for page in [Page::Playing, Page::Lyrics] {
        let mut state = phone();
        state.page = page.clone();
        state.mobile_ui.content_insets = screens()[0].1;
        state.lyrics = Some((
            "current".into(),
            Lyrics::Plain {
                lines: (0..60)
                    .map(|index| format!("Línea de prueba {index}"))
                    .collect(),
                source: "Prueba".into(),
            },
        ));
        let (mut tree, initial) = layout(&state, size);
        let scroll_id = if page == Page::Playing {
            "mobile-playing"
        } else {
            "lyrics-list"
        };
        let scroll = initial
            .regions
            .iter()
            .find(|region| region.id == scroll_id && region.kind == HitKind::Scroll)
            .expect("the song page has a real scroll area")
            .rect;
        tree.handle_event(UiEvent::ScrollDirect {
            position: center(scroll),
            delta_y: -120.0,
        });
        let frame = tree.layout(size);
        let header = surface(&frame, "mobile-header-bar");
        let header_bottom = header.y + header.height;
        let header_ink = frame
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::FillVerticalGradient { rect, bottom, .. } if *rect == header => {
                    Some(*bottom)
                }
                _ => None,
            })
            .expect("the opaque header is part of the page wash");
        let (fade, ink) = frame
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::FillVerticalGradient { rect, top, bottom }
                    if (rect.y - header_bottom).abs() < 0.1 && top.a == 255 && bottom.a == 0 =>
                {
                    Some((*rect, *top))
                }
                _ => None,
            })
            .expect("scrolling exposes the full fade immediately after the header");
        assert_eq!(
            ink, header_ink,
            "a warm header must not meet a black scroll-edge seam"
        );
        assert_ne!(ink, SCHEME.canvas);
        assert_eq!(
            fade.height, 20.0,
            "the header transition stays short instead of darkening a large content band"
        );
        let footer_top = if page == Page::Playing {
            size.height - state.mobile_ui.content_insets.bottom
        } else {
            surface(&frame, "mobile-mini-bar").y
        };
        let fade = frame
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::FillVerticalGradient { rect, top, bottom }
                    if (rect.y + rect.height - footer_top).abs() < 0.1
                        && top.a == 0
                        && bottom.a == 255 =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("the bottom fade finishes beside, rather than behind, the footer");
        assert_eq!(fade.height, 24.0);
    }
}

fn luminance(color: Color) -> f32 {
    let linear = |channel: u8| {
        let channel = f32::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

#[test]
fn bright_cover_colors_keep_secondary_copy_readable_including_the_mini_player() {
    for tint in [
        Color::rgb(255, 255, 255),
        Color::rgb(255, 220, 0),
        Color::rgb(0, 255, 0),
        Color::rgb(255, 0, 0),
        Color::rgb(0, 0, 255),
    ] {
        let mut state = phone();
        state.tints.insert("edge-cover".into(), tint);
        for page in [Page::Playing, Page::Lyrics] {
            state.page = page;
            let (top, bottom) = page_surface(&state);
            for background in [top, bottom, cover_tone(tint, 0.32)] {
                let ratio = (luminance(SCHEME.muted) + 0.05) / (luminance(background) + 0.05);
                assert!(
                    ratio >= 4.5,
                    "secondary text remains legible over {tint:?}: {background:?}, contrast {ratio}"
                );
            }
        }
    }
}

#[test]
fn missing_artwork_uses_the_music_placeholder_and_never_a_previous_songs_tint() {
    for artwork in [None, Some("unavailable-cover".into())] {
        let mut state = phone();
        state.page = Page::Playing;
        state.player.current.as_mut().unwrap().artwork_url = artwork;
        let size = screens()[0].0;
        state.mobile_ui.content_insets = screens()[0].1;
        let (_, frame) = layout(&state, size);
        assert_eq!(
            full_window_gradient(&frame, size),
            (SCHEME.canvas, SCHEME.canvas)
        );
        assert!(
            !frame
                .commands
                .iter()
                .any(|command| matches!(command, DrawCommand::RasterImage { .. })),
            "an unavailable cover is not replaced with an invented or stale image"
        );
        assert!(
            frame.commands.iter().any(|command| matches!(
                command,
                DrawCommand::Icon {
                    icon: Icon::MusicNote,
                    ..
                }
            )),
            "missing artwork keeps the honest music placeholder"
        );
    }
}
