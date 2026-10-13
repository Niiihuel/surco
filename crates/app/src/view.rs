//! The window, described from the state. Rebuilt on every frame.

use dnm_audio::equalizer::BAND_HZ;
use dnm_core::{Album, AlbumRef, Artist, Repeat, Track};
use limn::{
    Button, ButtonVariant, Color, Component, ComponentPalette, ContextMenu, ContextMenuEntry, ContextMenuItem,
    ControlSize, DropdownMenu, DropdownMenuEntry, DropdownMenuItem, Element, Empty, FontFamily, Icon, IconButton, Link,
    ListRow, MediaTile, Popover, PopoverPlacement, RadiusRole, RowSizing, ScrollArea, SearchField, Size, Slider,
    Spinner, Text, TextRole, Theme,
};

use crate::state::{clock, Page, State, ARTIST_SONGS_SHOWN, EQUALIZER_DB};

/// dnmusic's palette: near-black surfaces told apart by luminance, white ink.
pub const THEME: Theme = Theme {
    background: Color::rgb(18, 18, 18),
    foreground: Color::rgb(255, 255, 255),
    accent: Color::rgb(255, 255, 255),
};

const SIDEBAR_WIDTH: f32 = 260.0;
const PLAYER_HEIGHT: f32 = 76.0;
const VOLUME_WIDTH: f32 = 120.0;
const VOLUME_WIDTH_NARROW: f32 = 84.0;
const QUEUE_WIDTH: f32 = 300.0;
/// Below this the queue takes the place of the results instead of squeezing them, and the
/// volume gives up its share of the bar to the transport.
const WIDE_FROM: f32 = 960.0;
const EQUALIZER_SLIDER: f32 = 180.0;
const TILE_WIDTH: f32 = 148.0;
const COVER: f32 = 40.0;
const NUMBER_WIDTH: f32 = 24.0;
const DURATION_WIDTH: f32 = 56.0;
const MENU_WIDTH: f32 = 272.0;
/// dnmusic's track row: 60 high around a 40 cover, the title at 15 over a muted line at 12.
const TRACK_ROW: RowSizing = RowSizing {
    height: Some(60.0),
    two_line_height: Some(60.0),
    padding: 8.0,
    gap: 16.0,
    leading_width: COVER,
    leading_height: Some(44.0),
    title_size: 15.0,
    subtitle_size: 12.0,
    copy_gap: 2.0,
    title_height: Some(22.0),
    subtitle_height: Some(20.0),
    ..RowSizing::COMPACT
};
const ALBUM_COVER: f32 = 168.0;
const ARTIST_PICTURE: f32 = 128.0;
const PAGE_TITLE: f32 = 26.0;

pub fn view(state: &State, window: Size) -> Element {
    let palette = THEME.palette();
    let wide = window.width >= WIDE_FROM;
    let panes = match (state.queue_open, wide) {
        (false, _) => vec![sidebar(state, palette), pane(state, palette)],
        (true, true) => vec![sidebar(state, palette), pane(state, palette), queue(state, palette).size(Some(QUEUE_WIDTH), None)],
        (true, false) => vec![sidebar(state, palette), queue(state, palette).flex(1.0)],
    };
    let body = Element::row(panes).flex(1.0);
    Element::panel(Element::column(vec![body, player(state, wide, palette)]))
        .background(THEME.background)
        .size(Some(window.width), Some(window.height))
}

fn sidebar(state: &State, palette: ComponentPalette) -> Element {
    Element::panel(
        Element::column(vec![
            Element::text("dnmusic").font_family(FontFamily::Sans).font_size(18.0).foreground(palette.text),
            SearchField::new("search", &state.query, "Buscar canciones y artistas", palette)
                .width(SIDEBAR_WIDTH - 24.0)
                .render(),
        ])
        .gap(16.0),
    )
    .padding(12.0)
    .background(palette.surface)
    .size(Some(SIDEBAR_WIDTH), None)
}

fn centered(element: Element) -> Element {
    Element::column(vec![Element::spacer().flex(1.0), element.centered_x(), Element::spacer().flex(1.0)]).flex(1.0)
}

fn heading(title: &str, palette: ComponentPalette) -> Element {
    Text::new(title, palette).role(TextRole::Heading).render()
}

/// A row of tiles that scrolls sideways inside the page.
fn shelf(id: &str, tiles: Vec<Element>) -> Element {
    Element::horizontal_scroll_2d(id, Element::row(tiles).gap(4.0))
}

fn tile(state: &State, id: String, title: &str, subtitle: Option<String>, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    let tile = MediaTile::new(id, title, palette).width(TILE_WIDTH).round(round);
    let tile = match artwork.as_ref().and_then(|url| state.artwork.get(url)) {
        Some(image) => tile.artwork(Element::raster_image(image.clone().cover())),
        None => tile,
    };
    match subtitle {
        Some(subtitle) => tile.subtitle(subtitle),
        None => tile,
    }
    .render()
}

fn release_tiles(state: &State, prefix: &str, releases: &[AlbumRef], palette: ComponentPalette) -> Vec<Element> {
    releases
        .iter()
        .enumerate()
        .map(|(index, album)| {
            // One line fits one fact: whose it is, or when it came out on its artist's page.
            let subtitle = album.artist.clone().or_else(|| (!album.year.is_empty()).then(|| album.year.clone()));
            tile(state, format!("{prefix}:{index}"), &album.title, subtitle, &album.artwork_url, false, palette)
        })
        .collect()
}

/// `element` in the middle of a row's height.
fn middle(element: Element) -> Element {
    Element::column(vec![Element::spacer().flex(1.0), element, Element::spacer().flex(1.0)])
}

/// One entry of a row's menu.
struct Action {
    /// What a click on it reports.
    id: String,
    label: &'static str,
    icon: Icon,
    /// A second line: what the action leads to.
    detail: Option<String>,
}

/// What a row's menu offers for its track. `None` is a divider.
fn track_actions(row: &str, track: &Track) -> Vec<Option<Action>> {
    let item = |action: &str, label, icon, detail| Some(Action { id: format!("do:{action}:{row}"), label, icon: Icon::Custom(icon), detail });
    let mut actions = vec![
        item("queue", "Agregar a la cola", include_str!("../assets/icons/list-plus.svg"), None),
        item("next", "Poner a continuación", include_str!("../assets/icons/list-start.svg"), None),
        None,
    ];
    if track.album_id.is_some() {
        actions.push(item("album", "Ir al álbum", include_str!("../assets/icons/disc-3.svg"), track.album.clone()));
    }
    actions.push(item("artist", "Ir al artista", include_str!("../assets/icons/user-round.svg"), Some(track.artist.clone())));
    actions
}

/// A track the way dnmusic lists it: its number where the list has an order, its cover, the
/// title over a muted line, how long it lasts, and the dots that open its menu. The same menu
/// answers a secondary click.
fn track_row(state: &State, row: String, track: &Track, number: Option<usize>, subtitle: &str, palette: ComponentPalette) -> Element {
    let current = state.current().is_some_and(|current| current.id == track.id);
    let muted = |text: String| Element::text(text).font_family(FontFamily::Monospace).font_size(12.0).foreground(palette.muted);
    let mut leading = Vec::new();
    if let Some(number) = number {
        // The playing track shows that it plays where its number was.
        let mark = match (current, state.player.playing) {
            (true, true) => Element::icon(Icon::Audio).foreground(palette.text),
            (true, false) => Element::icon(Icon::PlayFill).foreground(palette.text),
            (false, _) => muted(number.to_string()),
        };
        leading.push(mark.centered().size(Some(NUMBER_WIDTH), Some(COVER)));
    }
    leading.push(cover(state, &track.artwork_url, COVER, palette));
    let sizing = RowSizing { leading_width: if number.is_some() { NUMBER_WIDTH + TRACK_ROW.gap + COVER } else { COVER }, ..TRACK_ROW };

    let open = state.menu.as_deref() == Some(row.as_str());
    let actions = track_actions(&row, track);
    let entries = actions.iter().map(|action| match action {
        Some(action) => {
            let item = DropdownMenuItem::new(&action.id, action.label).icon(action.icon);
            DropdownMenuEntry::Item(match &action.detail {
                Some(detail) => item.description(detail),
                None => item,
            })
        }
        None => DropdownMenuEntry::Separator,
    });
    let dots = IconButton::new(format!("more:{row}"), Icon::More, palette)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Compact)
        .tooltip("Opciones")
        .active(open)
        .render();
    let menu = DropdownMenu::new(dots, entries.collect(), open, "menu-close", palette)
        .width(MENU_WIDTH)
        .placement(PopoverPlacement::BottomEnd)
        .render();

    let mut list_row = ListRow::new(&row, &track.title, palette)
        .sizing(sizing)
        .leading(middle(Element::row(leading).gap(TRACK_ROW.gap)))
        .subtitle(subtitle)
        .title_fade(true)
        .radius(RadiusRole::Card);
    if current && number.is_none() {
        list_row = list_row.trailing(middle(Element::icon(Icon::Audio).foreground(palette.text)));
    }
    if track.duration_ms > 0 {
        let duration = muted(clock(track.duration_ms as f64 / 1000.0)).centered_x().size(Some(DURATION_WIDTH), None);
        list_row = list_row.trailing(middle(duration));
    }
    // The dots show while the pointer is on the row, or while their menu is open.
    let dots = Element::row(vec![menu]).reveal_on_hover(format!("dots:{row}"));
    let element = list_row.trailing_action(format!("more:{row}"), middle(dots)).render().hover_region();
    let entries = actions.into_iter().map(|action| match action {
        Some(action) => ContextMenuEntry::Item(ContextMenuItem::new(action.id, action.label).icon(action.icon)),
        None => ContextMenuEntry::Separator,
    });
    ContextMenu::new(format!("context:{row}"), element, entries.collect(), palette).render()
}

/// The main pane: the search, or the page the user walked to from it.
fn pane(state: &State, palette: ComponentPalette) -> Element {
    let page = match &state.page {
        Page::Search => return results(state, palette),
        Page::Loading(_) => centered(Spinner::new("opening", palette.muted).size(24.0).render()),
        Page::Failed(_, message) => centered(
            Empty::new("No se pudo abrir", message, palette)
                .icon(Icon::WarningTriangle)
                .action("retry", "Reintentar", ButtonVariant::Secondary)
                .render(),
        ),
        Page::Artist(artist) => artist_page(state, artist, palette),
        Page::Album(album) => album_page(state, album, palette),
    };
    let back = IconButton::new("back", Icon::ArrowLeft, palette).variant(ButtonVariant::Ghost).tooltip("Volver").enabled(state.can_go_back());
    Element::column(vec![Element::row(vec![back.render()]).padding(12.0), page]).flex(1.0)
}

fn results(state: &State, palette: ComponentPalette) -> Element {
    if state.searching {
        return centered(Spinner::new("searching", palette.muted).size(24.0).render());
    }
    if let Some(error) = &state.search_error {
        return centered(Empty::new("No se pudo buscar", error, palette).icon(Icon::WarningTriangle).render());
    }
    if state.submitted.is_none() {
        return centered(
            Empty::new("Buscá algo para escuchar", "Escribí una canción o un artista y apretá Enter.", palette)
                .icon(Icon::Search)
                .render(),
        );
    }
    let found = &state.found;
    if found.tracks.is_empty() && found.artists.is_empty() && found.albums.is_empty() {
        return centered(Empty::new("Sin resultados", "Probá con otras palabras.", palette).icon(Icon::MusicNote).render());
    }
    let mut sections = Vec::new();
    if !found.artists.is_empty() {
        let tiles = found.artists.iter().enumerate().map(|(index, artist)| {
            tile(state, format!("artist:{index}"), &artist.name, Some("Artista".into()), &artist.artwork_url, true, palette)
        });
        sections.extend([heading("Artistas", palette), shelf("found-artists", tiles.collect())]);
    }
    if !found.albums.is_empty() {
        sections.extend([heading("Álbumes", palette), shelf("found-albums", release_tiles(state, "album", &found.albums, palette))]);
    }
    if !found.tracks.is_empty() {
        let rows = found.tracks.iter().enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, None, &track.artist, palette));
        sections.extend([heading("Canciones", palette), Element::column(rows.collect()).gap(2.0)]);
    }
    ScrollArea::new("results", Element::column(sections).gap(12.0).padding(12.0)).render().flex(1.0)
}

fn play_buttons(palette: ComponentPalette) -> Element {
    Element::row(vec![
        Button::new("page-play", "Reproducir", palette).icon(Icon::PlayFill).render(),
        Button::new("page-shuffle", "Aleatorio", palette).icon(Icon::Shuffle).variant(ButtonVariant::Secondary).render(),
    ])
    .gap(8.0)
}

fn artist_page(state: &State, artist: &Artist, palette: ComponentPalette) -> Element {
    let mut about = vec![Text::new(&artist.name, palette).size(PAGE_TITLE).truncate().render()];
    if let Some(subscribers) = &artist.subscribers {
        about.push(Text::new(format!("{subscribers} suscriptores"), palette).muted().render());
    }
    about.push(play_buttons(palette));
    let header = Element::row(vec![
        cover(state, &artist.artwork_url, ARTIST_PICTURE, palette).radius(ARTIST_PICTURE / 2.0),
        // The copy gives way in a narrow pane; the picture keeps its size.
        Element::column(about).gap(10.0).flex(1.0).centered_y(),
    ])
    .gap(20.0);
    let mut sections = vec![header];
    if !artist.songs.is_empty() {
        let rows = artist.songs.iter().take(ARTIST_SONGS_SHOWN).enumerate().map(|(index, track)| {
            track_row(state, format!("page-track:{index}"), track, Some(index + 1), track.album.as_deref().unwrap_or(&track.artist), palette)
        });
        sections.extend([heading("Canciones", palette), Element::column(rows.collect()).gap(2.0)]);
    }
    if !artist.albums.is_empty() {
        sections.extend([heading("Álbumes", palette), shelf("artist-albums", release_tiles(state, "page-album", &artist.albums, palette))]);
    }
    if !artist.singles.is_empty() {
        let tiles = release_tiles(state, "page-single", &artist.singles, palette);
        sections.extend([heading("Sencillos y EP", palette), shelf("artist-singles", tiles)]);
    }
    ScrollArea::new("artist", Element::column(sections).gap(12.0).padding(12.0)).render().flex(1.0)
}

fn album_page(state: &State, album: &Album, palette: ComponentPalette) -> Element {
    let minutes = album.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000;
    let length = if minutes >= 60 { format!("{} h {} min", minutes / 60, minutes % 60) } else { format!("{minutes} min") };
    let count = match album.tracks.len() {
        1 => "1 canción".to_owned(),
        count => format!("{count} canciones"),
    };
    let facts: Vec<&str> = [album.year.as_str(), count.as_str(), length.as_str()].into_iter().filter(|fact| !fact.is_empty()).collect();
    let header = Element::row(vec![
        cover(state, &album.artwork_url, ALBUM_COVER, palette),
        Element::column(vec![
            Text::new(&album.title, palette).size(PAGE_TITLE).truncate().render(),
            Link::new("page-artist", &album.artist, palette).color(palette.text).font_size(14.0).render(),
            Text::new(facts.join(" · "), palette).muted().render(),
            play_buttons(palette),
        ])
        .gap(10.0)
        .flex(1.0)
        .centered_y(),
    ])
    .gap(20.0);
    let rows = album
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| track_row(state, format!("page-track:{index}"), track, Some(index + 1), &track.artist, palette));
    let sections = vec![header, Element::column(rows.collect()).gap(2.0)];
    ScrollArea::new("album", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
}

fn queue(state: &State, palette: ComponentPalette) -> Element {
    let title = Element::text("A continuación").font_family(FontFamily::Sans).font_size(14.0).foreground(palette.text);
    let rows: Vec<Element> = state
        .player
        .upcoming
        .iter()
        .enumerate()
        .map(|(index, track)| {
            ListRow::new(format!("upcoming:{index}"), &track.title, palette)
                .sizing(TRACK_ROW)
                .leading(middle(cover(state, &track.artwork_url, COVER, palette)))
                .subtitle(&track.artist)
                .title_fade(true)
                .radius(RadiusRole::Card)
                .render()
        })
        .collect();
    let list = if rows.is_empty() {
        Element::column(vec![
            Element::spacer().flex(1.0),
            Empty::new("Nada en la cola", "Elegí una canción y acá aparece lo que sigue.", palette).icon(Icon::Playlist).render().centered_x(),
            Element::spacer().flex(1.0),
        ])
        .flex(1.0)
    } else {
        ScrollArea::new("queue-list", Element::column(rows).gap(2.0)).render().flex(1.0)
    };
    Element::panel(Element::column(vec![title, list]).gap(12.0)).padding(12.0).background(palette.surface)
}

/// Ten bands and the preamp, a row each: the label, the slider and the gain it is set to.
fn equalizer(state: &State, palette: ComponentPalette) -> Element {
    let text = |value: String, width: f32| {
        Element::text(value).font_family(FontFamily::Monospace).font_size(11.0).foreground(palette.muted).size(Some(width), None).centered_y()
    };
    let row = |id: String, label: String, gain: f32| {
        Element::row(vec![
            text(label, 36.0),
            Slider::new(id, (gain / EQUALIZER_DB + 1.0) / 2.0, palette).steps(EQUALIZER_DB as usize * 2 + 1).width(EQUALIZER_SLIDER).render().centered_y(),
            text(if gain == 0.0 { "0 dB".into() } else { format!("{gain:+.0} dB") }, 44.0),
        ])
        .gap(8.0)
    };
    let hertz = |hz: f32| if hz >= 1000.0 { format!("{:.0}k", hz / 1000.0) } else { format!("{hz:.0}") };
    let header = Element::row(vec![
        Element::text("Ecualizador").font_family(FontFamily::Sans).font_size(13.0).foreground(palette.text).centered_y(),
        Element::spacer().flex(1.0),
        Button::new("equalizer-reset", "Restablecer", palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render(),
    ]);
    let mut rows = vec![header, row("equalizer:preamp".into(), "Pre".into(), state.equalizer.preamp_db)];
    rows.extend(BAND_HZ.iter().zip(state.equalizer.bands_db).enumerate().map(|(band, (hz, gain))| row(format!("equalizer:{band}"), hertz(*hz), gain)));
    Element::column(rows).gap(6.0).padding(12.0)
}

fn cover(state: &State, artwork: &Option<String>, side: f32, palette: ComponentPalette) -> Element {
    let radius = limn::tokens().radius.control;
    match artwork.as_ref().and_then(|url| state.artwork.get(url)) {
        // Covers are square; a video's thumbnail and an artist's banner are not.
        Some(image) => Element::raster_image(image.clone().cover()).size(Some(side), Some(side)).radius(radius),
        None => Element::panel(Element::icon(Icon::MusicNote).foreground(palette.muted).centered())
            .size(Some(side), Some(side))
            .background(palette.selected)
            .radius(radius),
    }
}

fn player(state: &State, wide: bool, palette: ComponentPalette) -> Element {
    let current = state.current();
    let has_track = current.is_some();
    let transport = |id: &str, icon: Icon, hint: &str| {
        IconButton::new(id, icon, palette)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Regular)
            .tooltip(hint)
            .enabled(has_track)
    };
    let now_playing = match current {
        Some(track) => Element::row(vec![
            cover(state, &track.artwork_url, 48.0, palette),
            // The title leads to the album and the artist to their page.
            Element::column(vec![
                Element::clickable(
                    "now-album",
                    Element::faded_text(&track.title, 24.0).font_family(FontFamily::Sans).font_size(14.0).foreground(palette.text),
                ),
                Element::clickable(
                    "now-artist",
                    Element::faded_text(state.player.error.as_deref().unwrap_or(&track.artist), 24.0)
                        .font_family(FontFamily::Sans)
                        .font_size(12.0)
                        .foreground(palette.muted),
                ),
            ])
            .gap(4.0)
            .flex(1.0)
            .centered_y(),
        ])
        .gap(12.0),
        None => Element::spacer(),
    };
    let repeat_icon = if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat };
    let buttons = Element::row(vec![
        Element::spacer().flex(1.0),
        transport("shuffle", Icon::Shuffle, "Aleatorio").active(state.player.shuffle).render(),
        transport("previous", Icon::SkipBack, "Anterior").render(),
        transport("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" })
            .render(),
        transport("next", Icon::SkipForward, "Siguiente").render(),
        transport("repeat", repeat_icon, "Repetir").active(state.player.repeat != Repeat::Off).render(),
        Element::spacer().flex(1.0),
    ])
    .gap(4.0);
    let time = |seconds: f64| {
        Element::text(clock(seconds)).font_family(FontFamily::Monospace).font_size(11.0).foreground(palette.muted).centered_y()
    };
    let seek = Element::row(vec![
        time(f64::from(state.progress()) * state.player.duration),
        Slider::new("seek", state.progress(), palette).enabled(has_track && state.player.duration > 0.0).render().flex(1.0).centered_y(),
        time(state.player.duration),
    ])
    .gap(10.0);
    let side = |id: &str, icon: Icon, hint: &str, active: bool| {
        IconButton::new(id, icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).tooltip(hint).active(active).render().centered_y()
    };
    let equalizer = Popover::new(
        side("equalizer", Icon::Sliders, "Ecualizador", state.equalizer_open),
        equalizer(state, palette),
        state.equalizer_open,
        "equalizer-close",
        palette,
    )
    .placement(PopoverPlacement::TopEnd)
    .width(36.0 + EQUALIZER_SLIDER + 44.0 + 16.0 + 24.0)
    .render()
    .centered_y();
    let mut volume = vec![
        equalizer,
        side("queue", Icon::Playlist, "Cola", state.queue_open),
        IconButton::new("mute", if state.volume > 0.0 { Icon::Volume } else { Icon::VolumeMuted }, palette)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Compact)
            .tooltip(if state.volume > 0.0 { "Silenciar" } else { "Activar sonido" })
            .render()
            .centered_y(),
        Slider::new("volume", state.volume, palette).width(if wide { VOLUME_WIDTH } else { VOLUME_WIDTH_NARROW }).render().centered_y(),
    ];
    // Wide, the three parts share the bar 1:2:1 and the transport sits in the middle of the
    // window. Narrow, the volume takes only what it needs.
    if wide {
        volume.insert(0, Element::spacer().flex(1.0));
    }
    let volume = Element::row(volume).gap(6.0).centered_y();
    Element::panel(
        Element::row(vec![
            now_playing.flex(1.0).centered_y(),
            Element::column(vec![buttons, seek]).gap(2.0).flex(2.0).centered_y(),
            if wide { volume.flex(1.0) } else { volume },
        ])
        .gap(24.0),
    )
    .padding(12.0)
    .background(palette.surface)
    .size(None, Some(PLAYER_HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Content, Event, Target};
    use crate::player::PlayerView;
    use dnm_core::{ArtistRef, Found};
    use limn::{HitKind, UiTree};

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("Title {id}"),
            artist: "Artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: None,
            duration_ms: 200_000,
        }
    }

    fn asked(query: &str) -> State {
        let mut state = State::default();
        state.query = query.into();
        state
    }

    fn ids(state: &State, size: Size) -> Vec<String> {
        let mut tree = UiTree::new(view(state, size));
        // A disabled control keeps a hover region for its tooltip; these are the ones that answer a press.
        tree.layout(size).regions.iter().filter(|region| matches!(region.kind, HitKind::Button | HitKind::TextInput)).map(|region| region.id.clone()).collect()
    }

    #[test]
    fn every_state_lays_out_at_narrow_and_wide_windows() {
        let mut searching = asked("q");
        searching.search();
        let mut failed = asked("q");
        failed.search();
        failed.apply(Event::SearchFailed { query: "q".into(), message: "sin red".into() });
        let mut empty = asked("q");
        empty.search();
        empty.apply(Event::SearchResults { query: "q".into(), found: Found::default() });
        for state in [State::default(), searching, failed, empty] {
            for width in [720.0, 1180.0, 2200.0] {
                let found = ids(&state, Size::new(width, 700.0));
                assert!(found.iter().any(|id| id == "search"), "the search field is always there at {width}");
                assert!(found.iter().any(|id| id == "volume"));
                assert!(found.iter().any(|id| id == "queue") && found.iter().any(|id| id == "equalizer"));
                assert!(!found.iter().any(|id| id == "seek"), "nothing to seek without a track");
            }
        }
    }

    #[test]
    fn results_are_clickable_rows_and_playing_one_enables_the_transport() {
        let mut state = asked("q");
        state.search();
        let found = Found { tracks: vec![track("a"), track("b"), track("c")], ..Found::default() };
        state.apply(Event::SearchResults { query: "q".into(), found });
        let size = Size::new(1180.0, 700.0);
        let before = ids(&state, size);
        for id in ["track:0", "track:1", "track:2"] {
            assert!(before.iter().any(|found| found == id), "{id} is a row");
        }
        assert!(!before.iter().any(|id| id == "play-pause"), "transport is passive with nothing loaded");
        state.show(PlayerView { current: Some(track("b")), playing: true, duration: 200.0, ..PlayerView::default() });
        let after = ids(&state, size);
        for id in ["play-pause", "next", "previous", "shuffle", "repeat", "seek", "mute", "volume"] {
            assert!(after.iter().any(|found| found == id), "{id} is live while a track plays");
        }
    }

    #[test]
    fn the_player_bar_keeps_every_control_inside_the_window() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), duration: 200.0, ..PlayerView::default() });
        for width in [698.0, 720.0, 960.0, 1400.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            for region in tree.layout(size).regions {
                let rect = region.rect;
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width, "{} spans {}..{} at {width}", region.id, rect.x, rect.x + rect.width);
            }
        }
    }

    #[test]
    fn search_artist_and_album_pages_offer_their_tiles_tracks_and_way_back() {
        let release = |id: &str| AlbumRef { id: id.into(), title: format!("Album {id}"), artist: Some("Artist".into()), year: "2017".into(), artwork_url: None };
        let mut state = asked("q");
        state.search();
        let found = Found {
            artists: vec![ArtistRef { id: "who".into(), name: "Artist".into(), subscribers: None, artwork_url: None }],
            albums: vec![release("lp")],
            tracks: vec![track("a")],
        };
        state.apply(Event::SearchResults { query: "q".into(), found });
        // Tall enough that nothing asked for below scrolls out of the pane.
        let size = Size::new(1180.0, 1400.0);
        let has = |state: &State, wanted: &[&str]| {
            let found = ids(state, size);
            for id in wanted {
                assert!(found.iter().any(|found| found == id), "{id} is on the page: {found:?}");
            }
        };
        has(&state, &["artist:0", "album:0", "track:0", "more:track:0"]);
        state.update(&limn::UiAction::Clicked { id: "more:track:0".into() });
        has(&state, &["do:queue:track:0", "do:next:track:0", "do:artist:track:0"]);
        state.update(&limn::UiAction::Clicked { id: "menu-close".into() });

        state.update(&limn::UiAction::Clicked { id: "artist:0".into() });
        let artist = Artist {
            id: "who".into(),
            name: "Artist".into(),
            subscribers: Some("4.36M".into()),
            artwork_url: None,
            songs: vec![track("s1"), track("s2")],
            albums: vec![release("lp")],
            singles: vec![release("single")],
        };
        state.apply(Event::Opened { target: Target::Artist("who".into()), result: Ok(Content::Artist(artist)) });
        has(&state, &["back", "page-play", "page-shuffle", "page-track:0", "page-track:1", "page-album:0", "page-single:0"]);

        state.update(&limn::UiAction::Clicked { id: "page-album:0".into() });
        has(&state, &["back"]);
        let album = Album {
            id: "lp".into(),
            title: "Album lp".into(),
            artist: "Artist".into(),
            artist_id: Some("who".into()),
            year: "2017".into(),
            artwork_url: None,
            tracks: vec![track("t1"), track("t2")],
        };
        state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album)) });
        has(&state, &["back", "page-artist", "page-play", "page-track:0", "page-track:1"]);

        state.update(&limn::UiAction::Clicked { id: "page-artist".into() });
        state.apply(Event::Opened { target: Target::Artist("who".into()), result: Err("sin red".into()) });
        has(&state, &["back", "retry"]);
    }

    #[test]
    fn the_queue_and_the_equalizer_open_at_any_width() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b"), track("c")], duration: 200.0, ..PlayerView::default() });
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        state.update(&limn::UiAction::Clicked { id: "equalizer".into() });
        for width in [720.0, 1180.0] {
            let found = ids(&state, Size::new(width, 700.0));
            for id in ["upcoming:0", "upcoming:1", "equalizer:preamp", "equalizer:0", "equalizer:9", "equalizer-reset"] {
                assert!(found.iter().any(|found| found == id), "{id} is there at {width}");
            }
        }
    }
}
