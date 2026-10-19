//! Mobile detail pages use the same catalog, library and track actions as desktop.

use super::*;

const GUTTER: f32 = 16.0;

fn cover_side(width: f32) -> f32 {
    (width - 64.0).clamp(176.0, 300.0)
}

fn title(value: &str, palette: ComponentPalette) -> Element {
    Text::new(value, palette)
        .role(TextRole::Heading)
        .font_size(28.0)
        .line_height(34.0)
        .render()
        .anchor("mobile-detail-title")
}

fn duration(tracks: &[Track]) -> String {
    let minutes = tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000;
    if minutes >= 60 {
        t!("common-hours-minutes", hours = minutes / 60, minutes = minutes % 60).into_owned()
    } else {
        t!("common-minutes", minutes = minutes).into_owned()
    }
}

fn keep(state: &State, before: &str, after: &str, palette: ComponentPalette) -> Element {
    let kept = state.keeps_page();
    icon(
        "keep",
        if kept { Icon::Check } else { Icon::Add },
        if kept { after } else { before },
        palette,
    )
}

fn tracks(state: &State, palette: ComponentPalette) -> Element {
    let rows = state.page_rows();
    if rows.is_empty() {
        return Empty::new(
            t!("mobile-no-songs"),
            if state.page_search.trim().is_empty() {
                t!("mobile-no-songs-catalog")
            } else {
                t!("mobile-no-songs-filter")
            },
            palette,
        )
        .icon(Icon::MusicNote)
        .render()
        .padding(16.0);
    }
    let count = rows.len();
    let range = crate::view::track_range(count, crate::view::MOBILE_TRACK_PITCH, state.track_window);
    let built = rows[range.clone()]
        .iter()
        .map(|(place, track)| track_row(state, format!("page-track:{place}"), track, &track.artist, palette))
        .collect();
    crate::view::windowed(built, range, count, crate::view::MOBILE_TRACK_PITCH, 4.0)
}

fn scroll(id: &str, sections: Vec<Element>) -> Element {
    ScrollArea::new(
        id,
        Element::column(sections).gap(24.0).padding(GUTTER),
    )
    .render()
    .flex(1.0)
}

pub(in crate::view) fn album(
    state: &State,
    album: &Album,
    width: f32,
    palette: ComponentPalette,
) -> Element {
    let side = cover_side(width);
    let metadata = [
        album.year.clone(),
        songs(album.tracks.len()),
        duration(&album.tracks),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let artist = Element::clickable(
        "page-artist",
        Text::new(&album.artist, palette)
            .font_size(15.0)
            .truncate()
            .render()
            .centered_y(),
    )
    .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-view-album-artist", artist = album.artist.as_str())))
    .size(None, Some(TAP));
    let about = Element::column(vec![
        title(&album.title, palette),
        artist,
        Text::new(metadata, palette)
            .font_size(13.0)
            .muted()
            .render(),
    ])
    .gap(4.0);
    let kept = keep(
        state,
        &t!("mobile-save-album"),
        &t!("mobile-remove-album"),
        palette,
    );
    let actions = if album.tracks.is_empty() {
        Element::row(vec![kept])
    } else {
        page_actions(state, vec![kept], palette)
    };
    scroll(
        "mobile-album",
        vec![
            sole_cover(state, &album.artwork_url, side, palette)
                .radius(8.0)
                .centered_x(),
            about,
            actions,
            tracks(state, palette),
        ],
    )
}

pub(in crate::view) fn artist(
    state: &State,
    artist: &Artist,
    width: f32,
    palette: ComponentPalette,
) -> Element {
    let side = cover_side(width);
    let mut about = vec![title(&artist.name, palette)];
    if let Some(subscribers) = artist
        .subscribers
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        about.push(
            Text::new(subscribers, palette)
                .font_size(14.0)
                .muted()
                .render(),
        );
    }
    let followed = keep(state, &t!("mobile-follow-artist"), &t!("mobile-unfollow-artist"), palette);
    let actions = if artist.songs.is_empty() {
        Element::row(vec![followed])
    } else {
        page_actions(state, vec![followed], palette)
    };
    let mut sections = vec![
        sole_cover(state, &artist.artwork_url, side, palette)
            .radius(side / 2.0)
            .centered_x(),
        Element::column(about).gap(8.0),
        actions,
    ];
    if !artist.songs.is_empty() {
        sections.push(
            Element::column(vec![heading(&t!("common-songs-heading"), palette), tracks(state, palette)]).gap(12.0),
        );
    }
    if !artist.albums.is_empty() {
        sections.push(shelf(
            state,
            "artist-albums",
            &t!("common-albums"),
            release_tiles(state, "page-album", &artist.albums, palette),
            palette,
        ));
    }
    if !artist.singles.is_empty() {
        sections.push(shelf(
            state,
            "artist-singles",
            &t!("mobile-singles-eps"),
            release_tiles(state, "page-single", &artist.singles, palette),
            palette,
        ));
    }
    scroll("mobile-artist", sections)
}

/// A mix the home made, as a playlist reads: its picture, what it is and who it plays, and
/// its songs.
pub(in crate::view) fn mix(state: &State, mix: &Mix, width: f32, palette: ComponentPalette) -> Element {
    let about = Element::column(vec![
        Text::eyebrow(t!("desktop-mix"), palette).render(),
        title(&crate::view::mix_name(mix), palette),
        Text::new(crate::view::mix_detail(mix), palette).font_size(13.0).muted().render(),
        Text::new(format!("{} · {}", songs(mix.tracks.len()), duration(&mix.tracks)), palette).font_size(13.0).muted().render(),
    ])
    .gap(4.0);
    let cover = crate::view::mix_cover(state, mix, cover_side(width), true, palette).centered_x();
    scroll("mobile-mix", vec![cover, about, page_actions(state, Vec::new(), palette), tracks(state, palette)])
}

pub(in crate::view) fn playlist(
    state: &State,
    playlist: &Playlist,
    width: f32,
    palette: ComponentPalette,
) -> Element {
    let side = cover_side(width);
    let artwork = playlist.cover().cloned();
    let picture = if artwork.is_none() {
        Element::panel(
            Element::icon(Icon::Playlist)
                .large_icon(48.0)
                .foreground(palette.muted)
                .centered(),
        )
        .background(SCHEME.raised)
        .size(Some(side), Some(side))
        .radius(8.0)
    } else {
        sole_cover(state, &artwork, side, palette).radius(8.0)
    };
    let mut about = vec![
        Text::eyebrow(
            if playlist.collaborative {
                t!("mobile-collaborative-playlist-eyebrow")
            } else {
                t!("mobile-playlist-eyebrow")
            },
            palette,
        )
        .render(),
        title(&playlist.name, palette),
        Text::new(
            format!(
                "{} · {}",
                songs(playlist.tracks.len()),
                duration(&playlist.tracks)
            ),
            palette,
        )
        .font_size(13.0)
        .muted()
        .render(),
    ];
    if let Some((_, missed)) = state
        .imported
        .as_ref()
        .filter(|(id, missed)| *id == playlist.id && !missed.is_empty())
    {
        let label = t!("mobile-songs-not-found", count = missed.len());
        about.push(
            Button::new("import-missed", label, palette)
                .variant(ButtonVariant::Ghost)
                .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() })
                .round()
                .render(),
        );
    }
    let mut sections = vec![picture.centered_x(), Element::column(about).gap(8.0)];
    let delete = icon("playlist-delete", Icon::Trash, &t!("mobile-delete-playlist"), palette);
    if playlist.tracks.is_empty() {
        sections.push(Element::row(vec![delete]));
        sections.push(
            Empty::new(
                t!("mobile-playlist-empty"),
                t!("mobile-playlist-empty-detail"),
                palette,
            )
            .icon(Icon::MusicNote)
            .render()
            .padding(16.0),
        );
    } else {
        sections.push(page_actions(state, Vec::new(), palette));
        let copied = state.list_copied == Some(playlist.id);
        let exported = state.list_exported == Some(playlist.id);
        let share = icon(
            "playlist-share",
            if copied {
                Icon::Check
            } else {
                Icon::Custom(include_str!("../../../assets/icons/share-2.svg"))
            },
            &if copied {
                t!("common-link-copied")
            } else {
                t!("mobile-share-playlist")
            },
            palette,
        );
        let export = icon(
            "playlist-export",
            if exported {
                Icon::Check
            } else {
                Icon::Custom(include_str!("../../../assets/icons/upload.svg"))
            },
            &if exported {
                t!("mobile-songs-copied")
            } else {
                t!("mobile-copy-songs")
            },
            palette,
        );
        let mut tools = vec![Element::row(vec![share, export, delete]).gap(8.0)];
        if copied {
            tools.push(
                Text::new(t!("common-link-copied"), palette)
                    .font_size(13.0)
                    .muted()
                    .render(),
            );
        }
        if exported {
            tools.push(
                Text::new(t!("mobile-songs-copied"), palette)
                    .font_size(13.0)
                    .muted()
                    .render(),
            );
        }
        sections.push(Element::column(tools).gap(8.0));
        sections.push(tracks(state, palette));
    }
    sections.extend(crate::view::suggested(state, playlist, width, palette));
    scroll("mobile-playlist", sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Command;
    use limn::{DrawCommand, Frame, UiAction, UiTree};

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("Una canción de nombre extenso hasta el amanecer {id}"),
            artist: "Artista de un nombre largo que también debe entrar".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: None,
            duration_ms: 180_000,
        }
    }

    fn pages() -> (Album, Artist, Playlist) {
        let tracks = vec![track("first"), track("second")];
        let release = AlbumRef {
            id: "release".into(),
            title: "Un lanzamiento real del artista".into(),
            artist: None,
            year: "2026".into(),
            artwork_url: None,
        };
        (
            Album {
                id: "album".into(),
                title: "Canciones que cruzan la ciudad hasta el amanecer".into(),
                artist: tracks[0].artist.clone(),
                artist_id: None,
                year: "2026".into(),
                artwork_url: None,
                tracks: tracks.clone(),
            },
            Artist {
                id: "artist".into(),
                name: "Un artista de nombre extenso que llega hasta el amanecer".into(),
                subscribers: Some("12 mil suscriptores".into()),
                artwork_url: None,
                songs: tracks.clone(),
                albums: vec![release.clone()],
                singles: vec![release],
            },
            Playlist {
                id: 7,
                name: "La playlist de nuestros viajes hasta el amanecer".into(),
                collaborative: false,
                picture: None,
                tracks,
            },
        )
    }

    fn frame(state: &State, width: f32) -> Frame {
        let content = match &state.page {
            Page::Album(value) => album(state, value, width, SCHEME.palette()),
            Page::Artist(value) => artist(state, value, width, SCHEME.palette()),
            Page::Playlist(id) => playlist(
                state,
                state.collection.playlist(*id).unwrap(),
                width,
                SCHEME.palette(),
            ),
            _ => unreachable!(),
        };
        let mut tree = UiTree::new(content);
        tree.set_text_measurer(|text, font_size, _| text.chars().count() as f32 * font_size * 0.5);
        tree.layout(Size::new(width, 2400.0))
    }

    fn touch_controls(frame: &Frame, width: f32, ids: &[&str]) {
        for id in ids {
            let nodes: Vec<_> = frame.access.iter().filter(|node| node.id == *id).collect();
            assert_eq!(nodes.len(), 1, "one {id} at {width}px");
            let rect = nodes[0].rect;
            assert!(
                rect.width + 0.01 >= TAP && rect.height + 0.01 >= TAP,
                "{id}: {rect:?}"
            );
            assert!(
                rect.x >= 0.0 && rect.x + rect.width <= width + 0.1,
                "{id} fits {width}px: {rect:?}"
            );
        }
        let titles: Vec<_> = frame
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text {
                    text,
                    position,
                    font_size,
                    ..
                } if *font_size >= 24.0 => Some((text, position, font_size)),
                _ => None,
            })
            .collect();
        assert!(
            titles.iter().any(|(text, _, _)| text.contains("amanecer")),
            "long title remains readable"
        );
        for (text, position, font_size) in titles {
            let measured = text.chars().count() as f32 * font_size * 0.5;
            assert!(
                position.x >= 0.0 && position.x + measured <= width + 0.1,
                "{text:?} fits {width}px: {position:?}"
            );
        }
    }

    #[test]
    fn long_details_and_touch_actions_fit_phone_widths() {
        let (album, artist, playlist) = pages();
        let mut state = State::default();
        state.mobile = true;
        state.collection.playlists.push(playlist.clone());
        state.page = Page::Playlist(playlist.id);
        state.ask_suggested();
        state.apply(crate::backend::Event::Suggested { playlist: playlist.id, tracks: vec![track("recommended")] });
        for width in [320.0, 390.0, 430.0] {
            for page in [
                Page::Album(album.clone()),
                Page::Artist(artist.clone()),
                Page::Playlist(playlist.id),
            ] {
                state.page = page;
                let mut ids = vec![
                    "page-play",
                    "page-shuffle",
                    "page-search",
                    "page-sort",
                    "more:page-track:0",
                    "more:page-track:1",
                ];
                if matches!(state.page, Page::Playlist(_)) {
                    ids.extend(["playlist-share", "playlist-export", "playlist-delete", "suggested:0", "suggested-add:0", "suggested-refresh"]);
                } else {
                    ids.push("keep");
                }
                touch_controls(&frame(&state, width), width, &ids);
            }
        }
    }

    #[test]
    fn detail_rows_keep_the_real_page_order_and_phone_context() {
        let (album, _, playlist) = pages();
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Album(album.clone());
        state.page_search = "second".into();
        let rendered = frame(&state, 320.0);
        assert!(rendered
            .access
            .iter()
            .any(|node| node.id == "more:page-track:1"));
        assert!(!rendered.access.iter().any(|node| node.id == "page-track:0"));
        assert!(
            matches!(state.update(&UiAction::Clicked { id: "page-track:1".into() }).as_slice(), [Command::PlayList { tracks, index: 1 }] if tracks == &album.tracks)
        );
        state.collection.playlists.push(playlist.clone());
        state.page = Page::Playlist(playlist.id);
        state.page_search.clear();
        assert!(state
            .update(&UiAction::Clicked {
                id: "more:page-track:1".into()
            })
            .is_empty());
        let context = state.mobile_ui.context.as_ref().unwrap();
        assert_eq!(context.track.id, "second");
        assert_eq!(context.playlist, Some(playlist.id));
    }

    #[test]
    fn playlist_feedback_and_artist_shelves_use_existing_data() {
        let (_, artist, playlist) = pages();
        let mut state = State::default();
        state.mobile = true;
        state.collection.playlists.push(playlist.clone());
        state.page = Page::Playlist(playlist.id);
        state.list_copied = Some(playlist.id);
        state.list_exported = Some(playlist.id);
        let rendered = frame(&state, 320.0);
        let labels: Vec<_> = rendered
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Enlace copiado"));
        assert!(labels
            .iter()
            .any(|label| label.contains("Canciones copiadas")));
        state.page = Page::Artist(artist);
        let rendered = frame(&state, 320.0);
        assert!(rendered.access.iter().any(|node| node.id == "page-album:0"));
        assert!(rendered
            .access
            .iter()
            .any(|node| node.id == "page-single:0"));
    }
}
