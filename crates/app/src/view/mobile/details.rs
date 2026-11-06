//! Mobile detail pages use the same catalog, library and track actions as desktop.

use super::*;

const GUTTER: f32 = 16.0;
/// BitChord's discs under a page's name: 50 across, white at 7% within a rim at 10%.
const DISC: f32 = 50.0;
const DISC_FILL: Color = Color::rgba(255, 255, 255, 18);
const DISC_RIM: Color = Color::rgba(255, 255, 255, 26);
const PLAY_PILL: f32 = 50.0;
/// The widest the Play pill grows; narrower, it gives way to the discs beside it.
const PLAY_PILL_MOST: f32 = 180.0;
/// The artist's play, larger than the discs beside it.
const ARTIST_PLAY: f32 = 70.0;
const ARTIST_DISC: f32 = 52.0;
/// An artist's name over their photo, in Display, as large as BitChord's logo stands, and
/// as small as it gets to stay on one line.
const ARTIST_NAME: f32 = 44.0;
const ARTIST_NAME_LEAST: f32 = 28.0;
const DISPLAY_ADVANCE: f32 = 0.72;
/// The newest release's cover, and the pane under it: white at 6% over the page's colour.
const RELEASE_ART: f32 = 96.0;
const RELEASE_PANE: Color = Color::rgba(255, 255, 255, 15);
/// The credit under a title, BitChord's secondary ink over the page's colour.
const CREDIT: Color = Color::rgba(255, 255, 255, 185);

/// BitChord's sleeve: 76% of the width, up to 300.
fn cover_side(width: f32) -> f32 {
    (width * 0.76).clamp(176.0, 300.0)
}

/// A sleeve as BitChord's: corners of 12 over a shadow of 18.
fn sleeve(picture: Element) -> Element {
    picture.radius(12.0).shadow(Color::rgba(0, 0, 0, 150), limn::Point::new(0.0, 8.0), 18.0, 0.0).centered_x()
}

fn title(value: &str, palette: ComponentPalette) -> Element {
    Text::new(value, palette)
        .font_size(22.0)
        .line_height(28.0)
        .render()
        .font_family(FontFamily::SansBold)
        // Each line centred across the page, so a long name wraps rather than overflowing.
        .center_content()
        .anchor("mobile-detail-title")
}

/// A page's facts under its name: small and muted, centred.
fn meta(value: String, palette: ComponentPalette) -> Element {
    Text::new(value, palette).font_size(12.0).muted().render().font_family(FontFamily::SansMedium).center_content()
}

/// A page's own action, as a disc of BitChord's.
fn disc(action: Element, side: f32) -> Element {
    Element::panel(action.centered()).background(DISC_FILL).border(DISC_RIM, 1.0).round().size(Some(side), Some(side)).centered_y()
}

/// The actions under a page's name, as BitChord's, centred: shuffle and the page's `own`
/// (keep, download, its menu) as discs either side of a white Play pill; an artist's play a
/// larger white disc between its two.
pub(in crate::view) fn actions(state: &State, own: Vec<Element>, palette: ComponentPalette) -> Element {
    let artist = matches!(state.page, Page::Artist(_));
    let side = if artist { ARTIST_DISC } else { DISC };
    let (glyph, hint) = shuffle_step(state);
    let shuffle = controls::action("page-shuffle", glyph, &hint, 22.0, palette);
    let shuffle = disc(if state.player.shuffle { shuffle.background(palette.selected) } else { shuffle }, side);
    let play = if artist {
        controls::playback("page-play", false, ARTIST_PLAY, 30.0, palette)
    } else {
        let ink = SCHEME.canvas;
        let copy = Element::row(vec![Element::icon(Icon::PlayFill).large_icon(20.0).foreground(ink).centered_y(),
            Text::new(t!("common-play"), palette).font_size(16.0).truncate().render().font_family(FontFamily::SansBold).foreground(ink).centered_y()]).gap(8.0).padding(12.0);
        let pill = Element::panel(copy.centered()).background(palette.text).pressed_background(ROUND_HELD)
            .hover_surface("page-play:pill", ["page-play"]).round().size(None, Some(PLAY_PILL));
        Element::clickable("page-play", pill).accessible(limn::Access::new(limn::Role::Button).name(t!("common-play"))).round()
            .max_width(PLAY_PILL_MOST).flex(3.0).centered_y()
    };
    // The pill takes the room first; once it is at its widest the rest centres the row.
    let mut row = vec![Element::spacer().flex(0.05), shuffle, play];
    row.extend(own.into_iter().map(|action| disc(action, side)));
    row.push(Element::spacer().flex(0.05));
    Element::row(row).gap(if artist { 24.0 } else { 12.0 })
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
    // The circled plus that fills with a check, as a song is liked with.
    icon("keep", crate::view::like_glyph(kept), if kept { after } else { before }, palette)
}

fn tracks(state: &State, palette: ComponentPalette) -> Element {
    let rows = state.page_rows();
    if rows.is_empty() {
        let why = if state.page_search.trim().is_empty() { t!("mobile-no-songs-catalog") } else { t!("mobile-no-songs-filter") };
        return blank(Icon::MusicNote, t!("mobile-no-songs"), why, None, palette);
    }
    let count = rows.len();
    let range = crate::view::track_range(count, crate::view::MOBILE_TRACK_PITCH, state.track_window);
    let built = rows[range.clone()]
        .iter()
        .map(|(place, track)| track_row(state, format!("page-track:{place}"), track, &track.artist, palette))
        .collect();
    crate::view::windowed(built, range, count, crate::view::MOBILE_TRACK_PITCH, 4.0)
}

/// A list's search and order in the bar, as BitChord's: one floating pill, its magnifier a
/// cross while the search is open.
pub(in crate::view) fn bar_tools(state: &State, palette: ComponentPalette) -> Element {
    let open = state.page_searching;
    let search = icon("page-search", if open { Icon::Close } else { Icon::Search }, &if open { t!("desktop-close-search") } else { t!("mobile-search-list") }, palette);
    let order = crate::view::page_order(state, icon("page-sort", Icon::Sort, &t!("desktop-sort-button"), palette));
    Element::panel(Element::row(vec![search, order])).glass(GLASS).round().centered_y()
}

/// The field a list is searched in, as BitChord's: under the bar, on the page's colour, the
/// list's songs that hold what is typed under it.
fn search_field(state: &State, palette: ComponentPalette) -> Element {
    let field = SearchField::new("page-query", &state.page_search, t!("mobile-search-list"), ComponentPalette { surface: Color::rgba(0, 0, 0, 90), ..SCHEME.on_raised() })
        .size(ControlSize::Spacious).render().size(None, Some(46.0)).round().border(HAIRLINE, 0.5);
    if state.page_search.is_empty() { field } else { inside_cross(field, "page-search-clear", &t!("mobile-clear-search"), palette) }
}

/// A list's page while it is searched: the field and the songs it finds, the sleeve and its
/// name scrolled away as BitChord carries its page up to the field.
fn searched(state: &State, id: &str, palette: ComponentPalette) -> Element {
    scroll(state, id, vec![search_field(state, palette), tracks(state, palette)])
}

fn scroll(state: &State, id: &str, sections: Vec<Element>) -> Element {
    let sections = page_search_slot(state).into_iter().chain(sections).collect();
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
    if state.page_searching { return searched(state, "mobile-album", palette); }
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
            .font_size(19.0)
            .ink(CREDIT)
            .truncate()
            .render()
            .center_content()
            .centered_y(),
    )
    .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-view-album-artist", artist = album.artist.as_str())))
    .size(None, Some(TAP));
    let about = Element::column(vec![title(&album.title, palette), artist, meta(metadata, palette)]).gap(2.0);
    let kept = keep(
        state,
        &t!("mobile-save-album"),
        &t!("mobile-remove-album"),
        palette,
    );
    let actions = if album.tracks.is_empty() {
        Element::row(vec![disc(kept, DISC)]).centered_x()
    } else {
        page_actions(state, vec![kept], palette)
    };
    scroll(
        state,
        "mobile-album",
        vec![
            sleeve(sole_cover(state, &album.artwork_url, side, palette)),
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
    // BitChord's: the photo across the whole screen from its top edge, under the header, a
    // little taller than wide, dissolving into the page's colour with the name centred over
    // its foot in Display.
    let tall = (width / 0.95).round();
    let tone = artist.artwork_url.as_deref().and_then(|url| state.tints.get(url)).copied().map_or(SCHEME.canvas, super::page_tone);
    // On one line where it can be, as BitChord's logo stands: Display's letters run about
    // 0.72 of their size wide. A name too long for even the smallest wraps.
    let size = ((width - 2.0 * GUTTER) / (artist.name.chars().count().max(1) as f32 * DISPLAY_ADVANCE)).clamp(ARTIST_NAME_LEAST, ARTIST_NAME);
    let name = Text::new(&artist.name, palette).font_size(size).line_height(size * 1.15).render()
        .font_family(FontFamily::Display).center_content().anchor("mobile-detail-title");
    let fade = Element::column(vec![Element::spacer().flex(1.0), name]).padding(GUTTER)
        .vertical_gradient(vec![Color { a: 0, ..tone }, Color { a: 200, ..tone }, tone]).size(None, Some((tall * 0.5).round()));
    let hero = Element::stack(vec![sole_cover(state, &artist.artwork_url, tall, palette).size(Some(width), Some(tall)),
        Element::column(vec![Element::spacer().flex(1.0), fade])]).size(Some(width), Some(tall)).clip_children();
    let mut sections = Vec::new();
    let followed = keep(state, &t!("mobile-follow-artist"), &t!("mobile-unfollow-artist"), palette);
    sections.push(if artist.songs.is_empty() {
        Element::row(vec![disc(followed, ARTIST_DISC)]).centered_x()
    } else {
        page_actions(state, vec![followed], palette)
    });
    if let Some(latest) = latest_release(state, artist, palette) { sections.push(latest); }
    if !artist.songs.is_empty() {
        let top = Text::new(t!("mobile-top-songs"), palette).font_size(24.0).render().font_family(FontFamily::SansBold);
        sections.push(Element::column(vec![top, tracks(state, palette)]).gap(12.0));
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
    // The photo is outside the gutters, and the page starts at the screen's top under it.
    ScrollArea::new("mobile-artist", Element::column(vec![hero, Element::column(sections).gap(24.0).padding(GUTTER)])).render().flex(1.0)
}

/// BitChord's newest release card under an artist's buttons: the cover, what it is and its
/// year over its name, on a pane a shade lighter than the page. The newest by year of the
/// albums and the singles; the first of each when the catalog gives none.
fn latest_release(state: &State, artist: &Artist, palette: ComponentPalette) -> Option<Element> {
    let year = |release: &AlbumRef| release.year.parse::<u32>().unwrap_or(0);
    let albums = artist.albums.iter().enumerate().map(|(index, release)| (format!("page-album:{index}"), t!("common-album"), release));
    let singles = artist.singles.iter().enumerate().map(|(index, release)| (format!("page-single:{index}"), t!("mobile-single"), release));
    let (id, kind, release) = singles.chain(albums).max_by_key(|(_, _, release)| year(release))?;
    let about = if release.year.is_empty() { kind.into_owned() } else { format!("{kind} · {}", release.year) };
    let copy = Element::column(vec![
        Text::new(about, palette).font_size(15.0).muted().truncate().render(),
        Text::new(&release.title, palette).font_size(20.0).truncate().render().font_family(FontFamily::SansBold),
    ]).gap(2.0).centered_y().flex(1.0);
    let cover = sole_cover(state, &release.artwork_url, RELEASE_ART, palette).size(Some(RELEASE_ART), Some(RELEASE_ART)).radius(12.0).border(THUMB_RIM, 0.5).clip_children();
    let card = Element::panel(Element::row(vec![cover, copy]).gap(16.0).padding(12.0)).background(RELEASE_PANE).radius(20.0);
    // Its own id: the same release is a tile on its shelf too.
    Some(Element::clickable(format!("latest-{id}"), card).dispatch_as(id).accessible(limn::Access::new(limn::Role::Button).name(release.title.as_str())).radius(20.0))
}

/// A mix the home made, as a playlist reads: its picture, what it is and who it plays, and
/// its songs.
pub(in crate::view) fn mix(state: &State, mix: &Mix, width: f32, palette: ComponentPalette) -> Element {
    if state.page_searching { return searched(state, "mobile-mix", palette); }
    let about = Element::column(vec![
        Text::eyebrow(t!("desktop-mix"), palette).render().center_content(),
        title(&crate::view::mix_name(mix), palette),
        Text::new(crate::view::mix_detail(mix), palette).font_size(15.0).ink(CREDIT).truncate().render().center_content(),
        meta(format!("{} · {}", songs(mix.tracks.len()), duration(&mix.tracks)), palette),
    ])
    .gap(2.0);
    let cover = sleeve(crate::view::mix_cover(state, mix, cover_side(width), true, palette));
    scroll(state, "mobile-mix", vec![cover, about, page_actions(state, Vec::new(), palette), tracks(state, palette)])
}

pub(in crate::view) fn playlist(
    state: &State,
    playlist: &Playlist,
    width: f32,
    palette: ComponentPalette,
) -> Element {
    if state.page_searching && !playlist.tracks.is_empty() { return searched(state, "mobile-playlist", palette); }
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
    } else {
        sole_cover(state, &artwork, side, palette)
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
        .render()
        .center_content(),
        title(&playlist.name, palette),
        meta(format!("{} · {}", songs(playlist.tracks.len()), duration(&playlist.tracks)), palette),
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
    let mut sections = vec![sleeve(picture), Element::column(about).gap(4.0)];
    // As the desktop's: what else a list can do waits behind its dots.
    let more = crate::view::playlist_menu(state, playlist, palette);
    if playlist.tracks.is_empty() {
        sections.push(Element::row(vec![disc(more, DISC)]).centered_x());
        sections.push(blank(Icon::MusicNote, t!("mobile-playlist-empty"), t!("mobile-playlist-empty-detail"), None, palette));
    } else {
        sections.push(page_actions(state, vec![more], palette));
        // What the menu just did, said under it.
        let copied = (state.list_copied == Some(playlist.id)).then(|| t!("common-link-copied"));
        let exported = (state.list_exported == Some(playlist.id)).then(|| t!("mobile-songs-copied"));
        sections.extend(copied.into_iter().chain(exported).map(|said| Text::new(said, palette).font_size(13.0).muted().render()));
        sections.push(tracks(state, palette));
    }
    sections.extend(crate::view::suggested(state, playlist, width, palette));
    scroll(state, "mobile-playlist", sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Command;
    use limn::{DrawCommand, Frame, HitKind, UiAction, UiTree};

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
                } if *font_size >= 22.0 => Some((text, position, font_size)),
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
    fn an_artists_newest_release_is_carded_under_their_buttons() {
        let (_, mut artist, _) = pages();
        artist.albums[0].year = "2024".into();
        artist.singles[0].year = "2026".into();
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Artist(artist.clone());
        let frame = frame(&state, 390.0);
        assert!(frame.regions.iter().any(|region| region.id == "latest-page-single:0"), "the 2026 single, not the 2024 album");
        assert!(!frame.regions.iter().any(|region| region.id == "latest-page-album:0"));
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
                    "more:page-track:0",
                    "more:page-track:1",
                ];
                if matches!(state.page, Page::Playlist(_)) {
                    ids.extend(["page-more", "page-download", "suggested:0", "suggested-add:0", "suggested-refresh"]);
                } else {
                    ids.push("keep");
                }
                touch_controls(&frame(&state, width), width, &ids);
                // A list's search and order are in the bar, as BitChord's; an artist has none.
                let mut tree = UiTree::new(super::super::view(&state, Size::new(width, 844.0)));
                tree.set_reduced_motion(true);
                let shown = tree.layout(Size::new(width, 844.0));
                let tools = !matches!(state.page, Page::Artist(_));
                for id in ["page-search", "page-sort"] {
                    let count = shown.regions.iter().filter(|region| region.id == id && region.kind == HitKind::Button).count();
                    assert_eq!(count, usize::from(tools), "{id} in the bar on {:?} at {width}", state.page);
                }
                if tools {
                    let search = shown.access.iter().find(|node| node.id == "page-search").unwrap().rect;
                    assert!(search.x > width / 2.0 && search.x + search.width <= width + 0.1 && search.y < 60.0, "at the bar's end: {search:?}");
                }
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
