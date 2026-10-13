//! The window, described from the state. Rebuilt on every frame.

use dnm_audio::equalizer::BAND_HZ;
use dnm_core::{Album, AlbumRef, Artist, Repeat, Track};
use limn::{
    ActionRow, ActionRowSizing, Alert, AlertDialog, Button, ButtonGroup, ButtonGroupSizing, ButtonVariant, Color, Component,
    ComponentPalette, ContextMenu, ContextMenuEntry, ContextMenuItem, ControlColors, ControlSize, DropdownMenu, DropdownMenuEntry,
    DropdownMenuItem, DropdownOption, Element, Empty, FontFamily, FormField, FormFieldSizing, Icon, IconButton, Link, ListRow, MediaCard, MediaTile,
    Modal, PageColumn, PageHeader, Popover, PopoverPlacement, PreferencesGroup, RadiusRole, ResizeHandle, RowSizing, Scheme,
    ScrollArea, SearchField, Select, Size, SkeletonGroup, Slider, Spinner, Switch, Tab, Tabs, Text, TextField, TextRole, Ticker,
    TooltipPlacement, TooltipSpec,
};

use crate::state::{
    clock, greeting, large_cover, share_link, Confirm, Entry, Filter, Lyrics, Page, Playlist, Quality, Side, State, ARTIST_SONGS_SHOWN, CROSSFADE_MAX, EQUALIZER_DB, SHELF,
};

/// dnmusic's colors, one per role: near-black surfaces told apart by luminance, white ink.
/// Every palette a component takes is derived from here, so that a control on a card, a
/// menu and a page cannot disagree about a shared color.
pub const SCHEME: Scheme = Scheme {
    canvas: Color::rgb(18, 18, 18),
    panel: Color::rgb(18, 18, 18),
    raised: Color::rgb(28, 28, 28),
    hover: Color::rgb(34, 34, 34),
    selected: Color::rgb(42, 42, 42),
    border: Color::rgb(46, 46, 46),
    text: Color::rgb(255, 255, 255),
    muted: Color::rgb(179, 179, 179),
    muted_on_raised: Color::rgb(179, 179, 179),
    menu: Color::rgb(36, 36, 38),
    menu_hover: Color::rgb(52, 52, 54),
    menu_muted: Color::rgb(179, 179, 179),
    shadow: Color::rgba(0, 0, 0, 140),
    control: ControlColors {
        surface: Color::rgb(42, 42, 42),
        hover: Color::rgb(52, 52, 52),
        pressed: Color::rgb(60, 60, 60),
        border: Color::rgb(60, 60, 60),
        text: Color::rgb(255, 255, 255),
    },
};

/// A segmented control: a track a step above its surface, the chosen segment sunk to the
/// page's color under it.
fn segmented_palette() -> ComponentPalette {
    ComponentPalette { surface: SCHEME.selected, raised: SCHEME.selected, hover: SCHEME.control.hover, selected: SCHEME.canvas, ..SCHEME.palette() }
}

/// A choice among a few, as wide as the room it is given. `group` names it, so the
/// selection travels from one segment to the next.
/// A line of lyrics reads from across the room.
const LYRIC_SIZE: f32 = 18.0;
const LYRIC_GAP: f32 = 14.0;
/// The three qualities side by side in a settings row, and the choice of output under them.
const QUALITY_WIDTH: f32 = 216.0;
const SYSTEM_OUTPUT: &str = "Predeterminado del sistema";

fn segments(group: &str, width: f32, tabs: Vec<Tab>) -> Element {
    Tabs::new(tabs, segmented_palette()).id(group).size(ControlSize::Compact).fill_width(width).render()
}

/// The window is laid out as Spotify's: a bar with the search across the top, three rounded
/// panels a gap apart on a black floor, and the playback bar across the bottom. The gaps
/// beside the middle panel are the handles that resize the other two.
const CANVAS: Color = Color::rgb(0, 0, 0);
const CHOSEN: Color = Color::rgb(31, 31, 31);
const GAP: f32 = 8.0;
const PANEL_RADIUS: f32 = 8.0;
const TOP_BAR_HEIGHT: f32 = 56.0;
const SEARCH_MAX_WIDTH: f32 = 474.0;
/// The room each end of the top bar keeps: two header buttons.
const TOP_BAR_END: f32 = 56.0;
/// The round button's fill under the pointer and while held, and what it shows disabled.
const ROUND_HOVER: Color = Color::rgb(224, 224, 224);
const ROUND_HELD: Color = Color::rgb(204, 204, 204);
const ROUND_OFF: Color = Color::rgb(54, 54, 54);
const ROUND_OFF_INK: Color = Color::rgb(117, 117, 117);
const PLAYER_HEIGHT: f32 = 76.0;
const VOLUME_WIDTH: f32 = 93.0;
const VOLUME_WIDTH_NARROW: f32 = 64.0;
/// The library folded to its icons, for a window too narrow to spare it a column.
const RAIL_WIDTH: f32 = 64.0;
const RAIL_BELOW: f32 = 860.0;
/// From this width the right panel has a column of its own; below it, the queue takes the
/// content's place.
const PANEL_FROM: f32 = 1024.0;
/// What the middle panel keeps however wide the user drags the other two.
const CONTENT_MIN_WIDTH: f32 = 360.0;
const EQUALIZER_SLIDER: f32 = 180.0;
const TILE_WIDTH: f32 = 148.0;
const COVER: f32 = 40.0;
const NUMBER_WIDTH: f32 = 24.0;
const DURATION_WIDTH: f32 = 56.0;
const MENU_WIDTH: f32 = 272.0;
const DOWNLOAD: &str = include_str!("../assets/icons/download.svg");
/// An entry of the library: 64 high around a 48 tile.
const LIBRARY_ROW: RowSizing = RowSizing {
    height: Some(64.0),
    two_line_height: Some(64.0),
    padding: 8.0,
    gap: 12.0,
    leading_width: LIBRARY_TILE,
    title_size: 15.0,
    subtitle_size: 12.0,
    copy_gap: 2.0,
    title_height: Some(26.0),
    subtitle_height: Some(20.0),
    ..RowSizing::COMPACT
};
const LIBRARY_TILE: f32 = 48.0;
/// A quick-access entry of the home page: a short row around the same tile.
const QUICK_ROW: RowSizing = RowSizing { height: Some(64.0), padding: 8.0, gap: 12.0, leading_width: LIBRARY_TILE, title_size: 14.0, ..RowSizing::COMPACT };
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
    let palette = SCHEME.palette();
    let surface = |element: Element| Element::panel(element).background(SCHEME.canvas).radius(PANEL_RADIUS);
    let gap = || Element::spacer().size(Some(GAP), None);
    // A panel on its way open or shut is drawn part of its width, uncovering content that
    // keeps the width it has at rest.
    let sliding = |content: Element, whole: f32, shown: f32| surface(Element::row(vec![content.size(Some(whole), None)])).size(Some(shown), None).clip_children();
    let now = std::time::Instant::now();
    let left_slide = state.library_slide.value(now).clamp(0.0, 1.0);
    let right_slide = if window.width >= PANEL_FROM { state.panel_slide.value(now).clamp(0.0, 1.0) } else { 0.0 };
    let rail = window.width < RAIL_BELOW;
    // The side panels give way before the content does.
    let right_whole = state.right_width.min(window.width - 4.0 * GAP - RAIL_WIDTH - CONTENT_MIN_WIDTH);
    let right = right_whole * right_slide;
    let left_whole = if rail { RAIL_WIDTH } else { state.left_width.min(window.width - 4.0 * GAP - right - CONTENT_MIN_WIDTH).max(RAIL_WIDTH) };
    let left = left_whole * left_slide;
    let handle = |side: Side, slide: f32| {
        // Only a panel at rest can be resized.
        if slide < 1.0 || (side == Side::Left && rail) { gap() } else { ResizeHandle::vertical(side.handle(), palette).thickness(GAP).render() }
    };

    let mut columns = vec![gap()];
    let mut content = window.width - 2.0 * GAP;
    if left_slide > 0.0 {
        columns.extend([sliding(library(state, rail, left_whole, palette), left_whole, left), handle(Side::Left, left_slide)]);
        content -= left + GAP;
    }
    if right_slide > 0.0 {
        content -= right + GAP;
        columns.push(surface(pane(state, content, palette)).flex(1.0));
        columns.extend([handle(Side::Right, right_slide), sliding(panel(state, palette), right_whole, right)]);
    } else if (state.queue_open || state.lyrics_open) && window.width < PANEL_FROM {
        columns.push(surface(panel(state, palette)).flex(1.0));
    } else {
        columns.push(surface(pane(state, content, palette)).flex(1.0));
    }
    columns.push(gap());
    let window_tree = Element::panel(Element::column(vec![top_bar(state, window.width, palette), Element::row(columns).flex(1.0), player(state, window.width, palette)]))
        .background(CANVAS)
        .size(Some(window.width), Some(window.height));
    let shared = share_dialog(state, window_tree, window.width, palette);
    let created = playlist_dialog(state, shared, window.width, palette);
    confirm_dialog(state, created, palette)
}

/// How a form's field reads in a dialog: its label a step up, its hint under the control.
const DIALOG_FIELD: FormFieldSizing = FormFieldSizing { label_role: TextRole::Label, hint_role: TextRole::Caption, gap: 8.0, ..FormFieldSizing::DEFAULT };

/// A dialog's footer: the way out first, what goes ahead last, at the end.
fn dialog_footer(cancel: Element, confirm: Element) -> Element {
    ButtonGroup::new(vec![cancel, confirm]).align_end().sizing(ButtonGroupSizing { gap: 8.0, stacked_gap: 8.0 }).render()
}

/// Making a playlist: its name, and whether others will add to it.
fn playlist_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some(draft) = &state.creating else { return window_tree };
    let dialog = Modal::fit_width(width, 440.0).max(240.0);
    // This dialog is short enough never to scroll: its form reaches both paddings.
    let form = Modal::fitted_body_width(dialog);
    let name = TextField::new("playlist-name", &draft.name, state.collection.default_name(), palette).size(ControlSize::Regular).render();
    let kind = |id: &str, label: &str, chosen: bool| Tab::new(id, label, 0.0).active(chosen);
    let kinds = segments(
        "playlist-kind",
        form,
        vec![kind("playlist-kind:normal", "Normal", !draft.collaborative), kind("playlist-kind:collaborative", "Colaborativa", draft.collaborative)],
    );
    let about = if draft.collaborative {
        "Otras personas van a poder agregar canciones con un enlace. Por ahora queda marcada en este equipo: invitar llega con las cuentas."
    } else {
        "Solo vos agregás y quitás canciones."
    };
    let mut fields: Vec<Element> = FormField::new("Título", name, palette).sizing(DIALOG_FIELD).rows();
    fields.extend(FormField::new("Tipo", kinds, palette).hint(about).sizing(DIALOG_FIELD).rows());
    if let Some(track) = &draft.first {
        fields.push(Text::new(format!("Empieza con «{}».", track.title), palette).role(TextRole::Caption).muted().truncate().render());
    }
    let cancel = Button::new("playlist-cancel", "Cancelar", palette).variant(ButtonVariant::Outline).size(ControlSize::Regular).render();
    let create = Button::new("playlist-create", "Crear", palette).size(ControlSize::Regular).render();
    Modal::new(window_tree, true, "Nueva playlist", "Juntá canciones bajo un nombre.", "playlist-cancel", palette)
        .width(dialog)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, create))
        .initial_focus("playlist-name")
        .return_focus("playlist-new")
        .render()
}

/// What cannot be undone asks before it is done.
fn confirm_dialog(state: &State, window_tree: Element, palette: ComponentPalette) -> Element {
    let (title, about, yes) = match &state.confirm {
        Some(Confirm::DeletePlaylist(id)) => {
            let name = state.collection.playlist(*id).map_or("esta playlist", |playlist| playlist.name.as_str());
            (format!("¿Eliminar «{name}»?"), "La playlist se borra de tu biblioteca. Las canciones no se tocan.", "Eliminar")
        }
        Some(Confirm::ClearDownloads) => ("¿Eliminar las descargas?".to_owned(), "Ya no vas a tener acceso a lo que descargaste en este equipo.", "Eliminar"),
        Some(Confirm::ClearRecent) => ("¿Borrar el historial de reproducciones?".to_owned(), "Los accesos rápidos de Inicio quedan vacíos.", "Borrar"),
        None => (String::new(), "", ""),
    };
    AlertDialog::new(window_tree, state.confirm.is_some(), title, about, "confirm-yes", "confirm-cancel", palette)
        .confirm_label(yes)
        .cancel_label("Cancelar")
        .render()
}

/// A button of a header: quiet, small, with its hint under it.
fn header_button(id: &str, icon: Icon, hint: TooltipSpec, palette: ComponentPalette) -> IconButton {
    IconButton::new(id, icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Dense).tooltip_with(hint.placement(TooltipPlacement::Below))
}

/// A track as the card that is handed around: its sleeve plays it, and while it plays the
/// record is out and turning.
fn media_card(state: &State, id: &str, track: &Track, palette: ComponentPalette) -> Element {
    let playing = state.player.playing && state.current().is_some_and(|current| current.id == track.id);
    let mut card = MediaCard::new(id, &track.title, palette)
        .artist(&track.artist)
        .playing(playing)
        .mark(Element::icon(Icon::Audio).foreground(palette.muted));
    if let Some(url) = &track.artwork_url {
        if let Some(image) = state.artwork.get(url) {
            card = card.artwork(image.clone());
        }
        if let Some(tint) = state.tints.get(url) {
            card = card.tint(*tint);
        }
    }
    card.render()
}

/// Sharing a track: its card, as whoever opens the link will see it, over the link itself.
fn share_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let (card, link) = match &state.sharing {
        Some(track) => (media_card(state, "share-card", track, palette), share_link(track)),
        None => (Element::spacer(), String::new()),
    };
    let copy = if state.copied { Button::new("share-copy", "Enlace copiado", palette).icon(Icon::Check) } else { Button::new("share-copy", "Copiar enlace", palette).icon(Icon::Copy) };
    Modal::new(window_tree, state.sharing.is_some(), "Compartir canción", "Quien abra el enlace la escucha en dnmusic.", "share-close", palette)
        .width(Modal::fit_width(width, 420.0))
        .body(Element::column(vec![card.centered_x(), Text::new(link, palette).mono().muted().truncate().render()]).gap(12.0))
        .footer(ButtonGroup::new(vec![copy.size(ControlSize::Regular).render()]).align_end().render())
        .render()
}

/// Across the top: the way back, home, and the search in the middle of the window. A side
/// panel that is shut is opened from here, on its own side.
fn top_bar(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let search = SearchField::new("search", &state.query, "¿Qué querés escuchar?", SCHEME.on_raised())
        .width((width - 280.0).clamp(160.0, SEARCH_MAX_WIDTH))
        .render()
        .centered_y();
    let mut leading = Vec::new();
    if !state.library_open {
        leading.push(header_button("toggle-library-open", Icon::PanelLeftOpen, TooltipSpec::new("Mostrar tu biblioteca").shortcut("Ctrl+B"), palette).render().centered_y());
    }
    leading.push(header_button("back", Icon::ArrowLeft, TooltipSpec::new("Volver"), palette).enabled(state.can_go_back()).render().centered_y());
    let mut trailing = vec![Element::spacer().flex(1.0)];
    trailing.push(header_button("settings", Icon::Settings, TooltipSpec::new("Configuración"), palette).active(state.page == Page::Settings).render().centered_y());
    if !state.panel_open && width >= PANEL_FROM {
        trailing.push(header_button("toggle-panel-open", Icon::PanelRightOpen, TooltipSpec::new("Mostrar lo que suena"), palette).render().centered_y());
    }
    // Both ends take the same room, so the search stays in the middle of the window.
    let end = |children: Vec<Element>| Element::row(children).gap(4.0).size(Some(TOP_BAR_END), None);
    Element::panel(
        Element::row(vec![
            end(leading),
            Element::spacer().flex(1.0),
            header_button("home", Icon::Home, TooltipSpec::new("Inicio"), palette).render().centered_y(),
            search,
            Element::spacer().flex(1.0),
            end(trailing),
        ])
        .gap(GAP),
    )
    .padding(GAP)
    .size(None, Some(TOP_BAR_HEIGHT))
}

/// A square of the library's size for something with no picture of its own.
fn library_tile(icon: Icon, palette: ComponentPalette) -> Element {
    Element::panel(Element::icon(icon).foreground(palette.text).centered())
        .background(CHOSEN)
        .radius(limn::tokens().radius.control)
        .size(Some(LIBRARY_TILE), Some(LIBRARY_TILE))
}

/// The left panel: what the user keeps, under the filters that narrow it. Folded, it is a
/// rail of pictures.
fn library(state: &State, rail: bool, width: f32, palette: ComponentPalette) -> Element {
    let shows = |filter: Filter| state.filter == Filter::All || state.filter == filter;
    // One entry: the id it reports, its picture, its name and what it is.
    let mut entries: Vec<(String, Element, &str, String, bool)> = Vec::new();
    if shows(Filter::Lists) {
        let count = match state.saved.len() {
            1 => "Playlist · 1 canción".to_owned(),
            count => format!("Playlist · {count} canciones"),
        };
        entries.push(("downloads".into(), library_tile(Icon::Custom(DOWNLOAD), palette), "Descargas", count, state.page == Page::Downloads));
        for (index, playlist) in state.collection.playlists.iter().enumerate() {
            let picture = match playlist.tracks.first() {
                Some(track) => cover(state, &track.artwork_url, LIBRARY_TILE, palette),
                None => library_tile(Icon::Playlist, palette),
            };
            let about = format!("{} · {}", if playlist.collaborative { "Playlist colaborativa" } else { "Playlist" }, songs(playlist.tracks.len()));
            entries.push((format!("playlist:{index}"), picture, &playlist.name, about, state.page == Page::Playlist(playlist.id)));
        }
    }
    if shows(Filter::Artists) {
        for (index, artist) in state.collection.artists() {
            let picture = cover(state, &artist.artwork_url, LIBRARY_TILE, palette).radius(LIBRARY_TILE / 2.0);
            let here = matches!(&state.page, Page::Artist(page) if page.id == artist.id);
            entries.push((format!("kept:{index}"), picture, &artist.name, "Artista".into(), here));
        }
    }
    if shows(Filter::Albums) {
        for (index, album) in state.collection.albums() {
            let about = album.artist.as_ref().map_or_else(|| "Álbum".to_owned(), |artist| format!("Álbum · {artist}"));
            let here = matches!(&state.page, Page::Album(page) if page.id == album.id);
            entries.push((format!("kept:{index}"), cover(state, &album.artwork_url, LIBRARY_TILE, palette), &album.title, about, here));
        }
    }
    if rail {
        let pictures = entries.into_iter().map(|(id, picture, ..)| Element::clickable(id, picture));
        let shut = header_button("toggle-library", Icon::PanelLeftClose, TooltipSpec::new("Ocultar tu biblioteca").shortcut("Ctrl+B"), palette);
        let head = Element::row(vec![Element::spacer().flex(1.0), shut.render().centered_y(), Element::spacer().flex(1.0)]).size(Some(LIBRARY_TILE), Some(40.0));
        return ScrollArea::new("library", Element::column(std::iter::once(head).chain(pictures).collect()).gap(8.0).padding(8.0)).render();
    }
    // A panel is shut from its own header.
    let header = Element::row(vec![
        header_button("toggle-library", Icon::PanelLeftClose, TooltipSpec::new("Ocultar tu biblioteca").shortcut("Ctrl+B"), palette).render().centered_y(),
        Text::new("Tu biblioteca", palette).size(15.0).muted().render().centered_y(),
        Element::spacer().flex(1.0),
        header_button("playlist-new", Icon::Add, TooltipSpec::new("Crear playlist"), palette).render().centered_y(),
    ])
    .gap(8.0)
    .size(None, Some(40.0));
    let filter = |id: &str, label: &str, filter: Filter| Tab::new(id, label, 0.0).active(state.filter == filter);
    let filters = segments(
        "library-filter",
        width - 16.0,
        vec![
            filter("filter:all", "Todo", Filter::All),
            filter("filter:lists", "Playlists", Filter::Lists),
            filter("filter:artists", "Artistas", Filter::Artists),
            filter("filter:albums", "Álbumes", Filter::Albums),
        ],
    );
    let rows: Vec<Element> = entries
        .into_iter()
        .map(|(id, picture, name, about, here)| {
            // The page being shown stands out by its fill.
            let palette = if here { ComponentPalette { surface: CHOSEN, ..palette } } else { palette };
            let row = ListRow::new(id, name, palette).sizing(LIBRARY_ROW).leading(picture).subtitle(about).title_fade(true).radius(RadiusRole::Card);
            if here { row.filled() } else { row }.render()
        })
        .collect();
    let list = if rows.is_empty() {
        let what = if state.filter == Filter::Artists { "Seguí a un artista desde su página y aparece acá." } else { "Guardá un álbum desde su página y aparece acá." };
        Element::panel(Text::new(what, palette).muted().render()).padding(8.0)
    } else {
        ScrollArea::new("library", Element::column(rows).gap(2.0)).render().flex(1.0)
    };
    Element::column(vec![Element::panel(header).padding(8.0), filters, list]).gap(6.0).padding(8.0)
}

/// The page the app opens on: what was opened lately, a click away, and what the user keeps.
fn home(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let collection = &state.collection;
    if collection.recent.is_empty() && collection.kept.is_empty() && state.saved.is_empty() && collection.heard.is_empty() {
        return centered(
            Empty::new("Buscá algo para escuchar", "Escribí una canción o un artista arriba y apretá Enter.", palette).icon(Icon::Search).render(),
        );
    }
    let hour = chrono::Timelike::hour(&chrono::Local::now());
    let mut sections = vec![Text::new(greeting(hour), palette).size(PAGE_TITLE).render()];

    // Quick access: a grid of short rows, each a step above the page.
    let columns = if width >= 720.0 { 3 } else { 2 };
    let side = ((width - 24.0 - (columns - 1) as f32 * GAP) / columns as f32).max(120.0);
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let quick = |id: String, name: &str, picture: Element| {
        ListRow::new(id, name, raised).sizing(QUICK_ROW).leading(picture).title_fade(true).radius(RadiusRole::Card).width(side).filled().render()
    };
    let mut tiles = Vec::new();
    if !state.saved.is_empty() {
        // The library lists the same entries: here they keep ids of their own and report the library's.
        tiles.push(quick("home-downloads".into(), "Descargas", library_tile(Icon::Custom(DOWNLOAD), palette)).dispatch_as("downloads"));
    }
    for (index, entry) in collection.recent.iter().enumerate() {
        let picture = cover(state, entry.artwork(), LIBRARY_TILE, palette);
        let picture = if matches!(entry, Entry::Artist(_)) { picture.radius(LIBRARY_TILE / 2.0) } else { picture };
        tiles.push(quick(format!("recent:{index}"), entry.title(), picture));
    }
    let mut tiles = tiles.into_iter().peekable();
    let mut grid = Vec::new();
    while tiles.peek().is_some() {
        grid.push(Element::row(tiles.by_ref().take(columns).collect()).gap(GAP));
    }
    sections.push(Element::column(grid).gap(GAP));

    // What was played, and what the catalog would play after the last of it.
    let pick = |id: String, track: &Track| tile(state, id, &track.title, Some(track.artist.clone()), &track.artwork_url, false, palette);
    let heard: Vec<Element> = collection.heard.iter().take(SHELF).enumerate().map(|(index, track)| pick(format!("heard:{index}"), track)).collect();
    if !heard.is_empty() {
        sections.extend([heading("Volver a escuchar", palette), shelf("home-heard", heard)]);
    }
    for (shelf_index, seed) in state.seeds().into_iter().enumerate() {
        let picks: Vec<Element> = state.recommended.get(&seed.id).into_iter().flatten().enumerate().map(|(index, track)| pick(format!("rec:{shelf_index}:{index}"), track)).collect();
        if !picks.is_empty() {
            sections.extend([heading(&format!("Porque escuchaste «{}»", seed.title), palette), shelf(&format!("home-rec:{shelf_index}"), picks)]);
        }
    }

    let albums: Vec<Element> = collection
        .albums()
        .map(|(index, album)| {
            tile(state, format!("home-kept:{index}"), &album.title, album.artist.clone(), &album.artwork_url, false, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !albums.is_empty() {
        sections.extend([heading("Tus álbumes", palette), shelf("home-albums", albums)]);
    }
    let artists: Vec<Element> = collection
        .artists()
        .map(|(index, artist)| {
            tile(state, format!("home-kept:{index}"), &artist.name, Some("Artista".into()), &artist.artwork_url, true, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !artists.is_empty() {
        sections.extend([heading("Tus artistas", palette), shelf("home-artists", artists)]);
    }
    ScrollArea::new("home", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
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
fn track_actions(state: &State, row: &str, track: &Track) -> Vec<Option<Action>> {
    let item = |action: &str, label, icon, detail| Some(Action { id: format!("do:{action}:{row}"), label, icon: Icon::Custom(icon), detail });
    let mut actions = vec![
        item("queue", "Agregar a la cola", include_str!("../assets/icons/list-plus.svg"), None),
        item("next", "Poner a continuación", include_str!("../assets/icons/list-start.svg"), None),
        item("share", "Compartir", include_str!("../assets/icons/share-2.svg"), None),
        None,
        item("add-new", "Agregar a una playlist nueva", include_str!("../assets/icons/list-plus.svg"), None),
    ];
    // Each of the user's playlists that does not hold the track yet.
    for playlist in state.collection.playlists.iter().filter(|playlist| !playlist.tracks.iter().any(|held| held.id == track.id)) {
        actions.push(Some(Action {
            id: format!("do:add-{}:{row}", playlist.id),
            label: "Agregar a",
            icon: Icon::Playlist,
            detail: Some(playlist.name.clone()),
        }));
    }
    if matches!(state.page, Page::Playlist(_)) && row.starts_with("page-track:") {
        actions.push(item("remove", "Quitar de esta playlist", include_str!("../assets/icons/circle-minus.svg"), None));
    }
    actions.push(None);
    if track.album_id.is_some() {
        actions.push(item("album", "Ir al álbum", include_str!("../assets/icons/disc-3.svg"), track.album.clone()));
    }
    actions.push(item("artist", "Ir al artista", include_str!("../assets/icons/user-round.svg"), Some(track.artist.clone())));
    actions.push(None);
    actions.push(if state.is_saved(&track.id) {
        item("unsave", "Quitar descarga", include_str!("../assets/icons/trash-2.svg"), None)
    } else {
        item("save", "Descargar para escuchar sin conexión", DOWNLOAD, None)
    });
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
    let actions = track_actions(state, &row, track);
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
    let menu = DropdownMenu::new(dots, entries.collect(), open, "menu-close", SCHEME.menu())
        .width(MENU_WIDTH)
        .placement(PopoverPlacement::BottomEnd)
        .render();

    let mut list_row = ListRow::new(&row, &track.title, palette)
        .sizing(sizing)
        .leading(middle(Element::row(leading).gap(TRACK_ROW.gap)))
        .subtitle(subtitle)
        .title_fade(true)
        .radius(RadiusRole::Card);
    // After the copy: that it plays, that it is kept on disk or on its way there, how long it lasts.
    let mut facts = Vec::new();
    if current && number.is_none() {
        facts.push(Element::icon(Icon::Audio).foreground(palette.text).size(Some(16.0), Some(16.0)).centered_y());
    }
    if state.saving.contains(&track.id) {
        facts.push(Spinner::new(format!("saving:{row}"), palette.muted).size(12.0).render().centered_y());
    } else if state.is_saved(&track.id) {
        facts.push(Element::icon(Icon::Custom(DOWNLOAD)).foreground(palette.muted).size(Some(14.0), Some(14.0)).centered_y());
    }
    if track.duration_ms > 0 {
        facts.push(muted(clock(track.duration_ms as f64 / 1000.0)).centered_x().size(Some(DURATION_WIDTH), None).centered_y());
    }
    if !facts.is_empty() {
        list_row = list_row.trailing(middle(Element::row(facts).gap(8.0)));
    }
    // The dots show while the pointer is on the row, or while their menu is open. A trailing
    // action sits flush with the row's edge: the row's own padding keeps them off it.
    let dots = Element::row(vec![menu, Element::spacer().size(Some(TRACK_ROW.padding), None)]).reveal_on_hover(format!("dots:{row}"));
    let element = list_row.trailing_action(format!("more:{row}"), middle(dots)).render().hover_region();
    let entries = actions.into_iter().map(|action| match action {
        Some(action) => ContextMenuEntry::Item(ContextMenuItem::new(action.id, action.label).icon(action.icon)),
        None => ContextMenuEntry::Separator,
    });
    ContextMenu::new(format!("context:{row}"), element, entries.collect(), SCHEME.menu()).render()
}

/// Rows that pulse where a list is on its way.
fn loading(id: &str, palette: ComponentPalette) -> Element {
    let rows = SkeletonGroup::rows(id, palette, 8).row_height(TRACK_ROW.height.unwrap_or_default()).icon(true).into_rows();
    Element::column(rows).gap(2.0).padding(12.0).flex(1.0)
}

/// The main pane: home, the search, or the page the user walked to. A page fades in and
/// settles when it takes the pane.
fn pane(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let entering = match &state.page {
        Page::Search if state.searching => "page:searching".to_owned(),
        Page::Search => format!("page:search:{}", state.submitted.as_deref().unwrap_or_default()),
        Page::Loading(_) => "page:loading".to_owned(),
        Page::Failed(..) => "page:failed".to_owned(),
        Page::Artist(artist) => format!("page:artist:{}", artist.id),
        Page::Album(album) => format!("page:album:{}", album.id),
        Page::Song(track) => format!("page:song:{}", track.id),
        Page::Downloads => "page:downloads".to_owned(),
        Page::Playlist(id) => format!("page:playlist:{id}"),
        Page::Settings => "page:settings".to_owned(),
    };
    page(state, width, palette).animate_enter(entering)
}

fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    match &state.page {
        Page::Search if state.submitted.is_none() && !state.searching => home(state, width, palette),
        Page::Search => results(state, palette),
        Page::Loading(_) => loading("opening", palette),
        Page::Failed(_, message) => centered(
            Empty::new("No se pudo abrir", message, palette)
                .icon(Icon::WarningTriangle)
                .action("retry", "Reintentar", ButtonVariant::Secondary)
                .render(),
        ),
        Page::Artist(artist) => artist_page(state, artist, palette),
        Page::Album(album) => album_page(state, album, palette),
        Page::Song(track) => centered(
            Element::column(vec![
                media_card(state, "song-card", track, palette).centered_x(),
                Text::new("Tocá la tapa para escucharla.", palette).muted().render().centered_x(),
            ])
            .gap(12.0),
        ),
        Page::Downloads => downloads_page(state, palette),
        Page::Playlist(id) => match state.collection.playlist(*id) {
            Some(playlist) => playlist_page(state, playlist, palette),
            None => centered(Empty::new("Esta playlist ya no existe", "Fue eliminada de tu biblioteca.", palette).icon(Icon::Playlist).render()),
        },
        Page::Settings => settings_page(state, width, palette),
    }
}

/// "1 canción", "12 canciones".
fn songs(count: usize) -> String {
    if count == 1 { "1 canción".to_owned() } else { format!("{count} canciones") }
}

/// One of the user's playlists: what it is, what to do with it, and its tracks in order.
fn playlist_page(state: &State, playlist: &Playlist, palette: ComponentPalette) -> Element {
    let minutes = playlist.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000;
    let length = if minutes >= 60 { format!("{} h {} min", minutes / 60, minutes % 60) } else { format!("{minutes} min") };
    let picture = match playlist.tracks.first() {
        Some(track) => cover(state, &track.artwork_url, ALBUM_COVER, palette),
        None => Element::panel(Element::icon(Icon::Playlist).large_icon(48.0).foreground(palette.muted).centered())
            .background(CHOSEN)
            .radius(limn::tokens().radius.control)
            .size(Some(ALBUM_COVER), Some(ALBUM_COVER)),
    };
    let mut actions = vec![
        Button::new("page-play", "Reproducir", palette).icon(Icon::PlayFill).render(),
        Button::new("page-shuffle", "Aleatorio", palette).icon(Icon::Shuffle).variant(ButtonVariant::Secondary).render(),
        Button::new("playlist-delete", "Eliminar", palette).icon(Icon::Trash).variant(ButtonVariant::Ghost).render(),
    ];
    if playlist.tracks.is_empty() {
        // Nothing to play yet.
        actions.drain(..2);
    }
    let header = Element::row(vec![
        picture,
        Element::column(vec![
            Text::eyebrow(if playlist.collaborative { "PLAYLIST COLABORATIVA" } else { "PLAYLIST" }, palette).render(),
            Text::new(&playlist.name, palette).size(PAGE_TITLE).truncate().render(),
            Text::new(format!("{} · {length}", songs(playlist.tracks.len())), palette).muted().render(),
            Element::row(actions).gap(8.0),
        ])
        .gap(10.0)
        .flex(1.0)
        .centered_y(),
    ])
    .gap(20.0);
    let list = if playlist.tracks.is_empty() {
        Element::panel(
            Empty::new("Todavía no tiene canciones", "Buscá una canción y elegí «Agregar a» en su menú.", palette).icon(Icon::MusicNote).render().centered_x(),
        )
        .padding(24.0)
    } else {
        let rows = playlist.tracks.iter().enumerate().map(|(index, track)| {
            let row = track_row(state, format!("page-track:{index}"), track, Some(index + 1), &track.artist, palette);
            Element::column(vec![row]).animate_height(format!("playlist-row:{}:{}", playlist.id, track.id), false)
        });
        Element::column(rows.collect()).keep_leaving("playlist-rows")
    };
    ScrollArea::new("playlist", Element::column(vec![header, list]).gap(16.0).padding(12.0)).render().flex(1.0)
}

/// A size on disk as people read it.
fn megabytes(bytes: u64) -> String {
    let megabytes = bytes as f64 / 1_048_576.0;
    if megabytes >= 1024.0 { format!("{:.1} GB", megabytes / 1024.0) } else { format!("{megabytes:.0} MB") }
}

/// A settings row's measures: its name over what it does, a control at the end.
const SETTINGS_ROW: ActionRowSizing = ActionRowSizing { gap: 16.0, copy_gap: 4.0, icon_column: 20.0, min_height: 34.0, min_height_with_subtitle: 34.0 };

/// What the user can set: one page of groups, each row its name and what it does beside
/// its control. A narrow page stacks the control under the copy.
fn settings_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let column = PageColumn::new("settings", width, palette).gutter(12.0, 20.0, 600.0).gap(12.0).reserve_rail(true);
    // A switch or a short button fits beside its row's copy in less room than a field would.
    let compact = column.content_width() < 480.0;
    let (card, control) = (SCHEME.card(), SCHEME.on_raised());
    let row = |title: &str, about: &str| ActionRow::new(title, card).subtitle(about).wrap_title(true).align_top(true).sizing(SETTINGS_ROW);
    let switch = |title: &str, about: &str, id: &str, on: bool| row(title, about).trailing(Switch::new(id, on, control).render()).stacked(compact).render();
    let action = |id: &str, label: &str, enabled: bool| Button::new(id, label, control).variant(ButtonVariant::Outline).size(ControlSize::Compact).enabled(enabled).render();
    let group = |title: &str, rows: Vec<Element>| PreferencesGroup::new(title, rows, card).render();

    let equalizer = if state.equalizer == dnm_audio::equalizer::EqualizerValues::FLAT { "Plano" } else { "Personalizado" };
    let downloads = format!("{} · {}", songs(state.saved.len()), megabytes(state.saved_bytes));
    let device = |id: &str, name: &str| DropdownOption::new(format!("setting:output:{id}"), name).selected(state.output.as_deref().unwrap_or_default() == id);
    let devices = std::iter::once(device("", SYSTEM_OUTPUT)).chain(state.outputs.iter().map(|output| device(&output.id, &output.name)));
    let playing_through = state.outputs.iter().find(|output| state.output.as_deref() == Some(output.id.as_str())).map_or(SYSTEM_OUTPUT, |output| output.name.as_str());
    let output = Select::new("setting:output", playing_through, devices.collect(), state.output_open, "setting:output-dismiss", control)
        .menu_palette(SCHEME.menu())
        .width(QUALITY_WIDTH)
        .render();
    let chosen = state.settings.quality;
    let quality = segments(
        "setting-quality",
        QUALITY_WIDTH,
        Quality::ALL.into_iter().map(|(quality, name, label)| Tab::new(format!("setting:quality:{name}"), label, 0.0).active(quality == chosen)).collect(),
    );
    let ceiling = match chosen {
        Quality::Low => "Hasta 50 kbps.",
        Quality::Normal => "Hasta 70 kbps.",
        Quality::High => "Hasta 140 kbps.",
    };
    let seconds = state.settings.crossfade;
    let crossfade = Element::row(vec![
        Slider::new("setting:crossfade", seconds / CROSSFADE_MAX, control).steps(CROSSFADE_MAX as usize + 1).width(140.0).render().centered_y(),
        Text::new(if seconds == 0.0 { "No".to_owned() } else { format!("{seconds:.0} s") }, card).role(TextRole::Caption).muted().render().size(Some(28.0), None).centered_y(),
    ])
    .gap(8.0);
    let body = vec![
        PageHeader::new("Configuración", card).description("Lo que elegís acá vale para este equipo.").stacked(compact).render(),
        group(
            "Reproducción",
            vec![
                switch("Reproducción automática", "Cuando termina lo que estás escuchando, sigue con algo similar.", "setting:autoplay", state.settings.autoplay),
                row("Crossfade entre canciones", "Mezcla el final de una canción con el comienzo de la siguiente.").trailing(crossfade).stacked(compact).render(),
                row("Calidad de reproducción", &format!("{} Cambia a partir de la próxima canción.", ceiling)).trailing(quality).stacked(compact).render(),
                row("Ecualizador", &format!("Diez bandas y un previo. Ahora: {equalizer}.")).trailing(action("equalizer", "Ajustar", true)).stacked(compact).render(),
                row("Dispositivo de salida", "Por dónde suena la música.").trailing(output).stacked(compact).render(),
            ],
        ),
        group("Visualización", vec![switch("Reducir movimiento", "Muestra los cambios de una vez, sin animaciones.", "setting:motion", state.settings.reduce_motion)]),
        group(
            "Almacenamiento",
            vec![
                row(&format!("Descargas: {downloads}"), "Lo que descargaste para escuchar sin conexión.")
                    .trailing(action("downloads-clear", "Eliminar todas las descargas", !state.saved.is_empty()))
                    .stacked(compact)
                    .render(),
            ],
        ),
        group(
            "Privacidad y datos",
            vec![
                row("Borrar historial de reproducciones", "Borra de este equipo lo que escuchaste recientemente.")
                    .trailing(action("recent-clear", "Borrar", !state.collection.recent.is_empty()))
                    .stacked(compact)
                    .render(),
            ],
        ),
        group(
            "Acerca de",
            vec![
                row("Versión", "dnmusic para escritorio.").value(env!("CARGO_PKG_VERSION")).stacked(compact).render(),
                row("Licencias de terceros", "Motor de audio adaptado de zuno (Apache-2.0). Tarjeta de pista según Spell UI (MIT). Íconos de Lucide (ISC) y Bootstrap Icons (MIT). Catálogo con ytmapi-rs (MIT).").render(),
            ],
        ),
    ];
    column.children(body).render().flex(1.0)
}

fn results(state: &State, palette: ComponentPalette) -> Element {
    if state.searching {
        return loading("searching", palette);
    }
    if let Some(error) = &state.search_error {
        // Without a network there is still what was kept for it.
        let empty = Empty::new("No se pudo buscar", error, palette).icon(Icon::WarningTriangle);
        let empty = if state.saved.is_empty() { empty } else { empty.action("downloads", "Ver descargas", ButtonVariant::Secondary) };
        return centered(empty.render());
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

/// A page's actions: play it, shuffle it and, for a page the library can hold, keep it.
/// `keep` names the action when the page is not kept and when it is.
fn play_buttons(state: &State, keep: Option<(&str, &str)>, palette: ComponentPalette) -> Element {
    let mut buttons = vec![
        Button::new("page-play", "Reproducir", palette).icon(Icon::PlayFill).render(),
        Button::new("page-shuffle", "Aleatorio", palette).icon(Icon::Shuffle).variant(ButtonVariant::Secondary).render(),
    ];
    if let Some((keep, kept)) = keep {
        let (label, icon) = if state.keeps_page() { (kept, Icon::Check) } else { (keep, Icon::Add) };
        buttons.push(Button::new("keep", label, palette).icon(icon).variant(ButtonVariant::Ghost).render());
    }
    Element::row(buttons).gap(8.0)
}

fn artist_page(state: &State, artist: &Artist, palette: ComponentPalette) -> Element {
    let mut about = vec![Text::new(&artist.name, palette).size(PAGE_TITLE).truncate().render()];
    if let Some(subscribers) = &artist.subscribers {
        about.push(Text::new(format!("{subscribers} suscriptores"), palette).muted().render());
    }
    about.push(play_buttons(state, Some(("Seguir", "Siguiendo")), palette));
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

fn downloads_page(state: &State, palette: ComponentPalette) -> Element {
    let failure = state.save_error.as_ref().map(|error| Alert::new("No se pudo descargar", error, palette).render());
    if state.saved.is_empty() && state.saving.is_empty() {
        let empty = Empty::new("Nada descargado", "Lo que descargues desde el menú de una canción queda acá, para escucharlo sin conexión.", palette)
            .icon(Icon::Custom(DOWNLOAD))
            .render();
        return Element::column(failure.into_iter().chain([centered(empty)]).collect()).padding(12.0).flex(1.0);
    }
    let count = match (state.saved.len(), state.saving.len()) {
        (1, 0) => "1 canción en este equipo".to_owned(),
        (saved, 0) => format!("{saved} canciones en este equipo"),
        (saved, saving) => format!("{saved} en este equipo, {saving} descargando"),
    };
    let mut sections = vec![Element::column(vec![
        Text::new("Descargas", palette).size(PAGE_TITLE).render(),
        Text::new(count, palette).muted().render(),
        play_buttons(state, None, palette),
    ])
    .gap(10.0)];
    sections.extend(failure);
    // A download that is removed closes where it was.
    let rows = state.saved.iter().enumerate().map(|(index, track)| {
        let row = track_row(state, format!("page-track:{index}"), track, Some(index + 1), &track.artist, palette);
        Element::column(vec![row]).animate_height(format!("saved-row:{}", track.id), false)
    });
    sections.push(Element::column(rows.collect()).keep_leaving("saved-rows"));
    ScrollArea::new("downloads", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
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
            play_buttons(state, Some(("Guardar", "En tu biblioteca")), palette),
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

/// A panel's head: what it shows, and a muted line about it.
fn panel_header(title: &str, subtitle: &str, palette: ComponentPalette) -> Element {
    let copy = Element::column(vec![Text::new(title, palette).size(20.0).truncate().render(), Text::new(subtitle, palette).muted().truncate().render()]).gap(2.0);
    // A panel is shut from its own header.
    let shut = header_button("toggle-panel", Icon::PanelRightClose, TooltipSpec::new("Ocultar este panel"), palette).render();
    Element::row(vec![copy.flex(1.0), shut])
}

/// The right-hand column: what plays, or what comes after it while the queue is open.
fn panel(state: &State, palette: ComponentPalette) -> Element {
    let content = if state.queue_open {
        vec![panel_header("Lo que viene", "La cola, en su orden real", palette), upcoming(state, palette)]
    } else if state.lyrics_open {
        vec![panel_header("Letra", state.current().map_or("Nada por ahora", |track| track.title.as_str()), palette), lyrics(state, palette)]
    } else {
        match state.current() {
            Some(track) => {
                let side = state.right_width - 32.0;
                let link = |id: &str, action: &str, text: Element| Element::clickable(id, text).dispatch_as(action);
                vec![
                    panel_header("Sonando", track.album.as_deref().unwrap_or(&track.artist), palette),
                    {
                        // The large rendition once it has arrived; until then, the row's.
                        let large = track.artwork_url.as_deref().map(large_cover).filter(|url| state.artwork.contains_key(url));
                        cover(state, if large.is_some() { &large } else { &track.artwork_url }, side, palette).radius(RadiusRole::Panel.px())
                    },
                    link("panel-album", "now-album", Text::new(&track.title, palette).size(18.0).truncate().render()),
                    link("panel-artist", "now-artist", Text::new(&track.artist, palette).muted().truncate().render()),
                ]
            }
            None => vec![
                panel_header("Sonando", "Nada por ahora", palette),
                centered(Empty::new("Nada sonando", "Elegí una canción y acá aparece.", palette).icon(Icon::MusicNote).render()),
            ],
        }
    };
    let face = match (state.queue_open, state.lyrics_open) {
        (true, _) => "panel:queue",
        (_, true) => "panel:lyrics",
        _ => "panel:playing",
    };
    Element::column(content).gap(12.0).padding(16.0).animate_enter(face)
}

/// The lines of what plays, the one being sung in full ink. A line that is clicked is sung
/// from its start.
fn lyrics(state: &State, palette: ComponentPalette) -> Element {
    let note = |title: &str, about: &str| centered(Empty::new(title, about, palette).icon(Icon::Microphone).render());
    let lines = match &state.lyrics {
        _ if state.current().is_none() => return note("Nada sonando", "Elegí una canción y acá aparece su letra."),
        Some((_, Lyrics::Loading)) | None => return loading("lyrics-loading", palette),
        Some((_, Lyrics::Missing)) => return note("Sin letra sincronizada", "Todavía nadie la cargó para esta canción."),
        Some((_, Lyrics::Lines(_))) => state.lines(),
    };
    let sung = state.active_line();
    let rows = lines.iter().enumerate().map(|(index, line)| {
        let ink = if sung == Some(index) { palette.text } else { palette.muted };
        let text = Text::new(&line.text, palette).size(LYRIC_SIZE).ink(ink).render().animate_foreground(format!("lyric-ink:{index}"));
        Element::clickable(format!("lyric:{index}"), text)
    });
    ScrollArea::new("lyrics-list", Element::column(rows.collect()).gap(LYRIC_GAP)).render().flex(1.0)
}

fn upcoming(state: &State, palette: ComponentPalette) -> Element {
    let rows: Vec<Element> = state
        .player
        .upcoming
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let row = ListRow::new(format!("upcoming:{index}"), &track.title, palette)
                .sizing(TRACK_ROW)
                .leading(middle(cover(state, &track.artwork_url, COVER, palette)))
                .subtitle(&track.artist)
                .title_fade(true)
                .radius(RadiusRole::Card)
                .render();
            // Named by its track, so the row that starts playing closes and the rest glide up.
            Element::column(vec![row]).animate_height(format!("upcoming-row:{}", track.id), false)
        })
        .collect();
    if rows.is_empty() {
        return centered(Empty::new("Nada en la cola", "Elegí una canción y acá aparece lo que sigue.", palette).icon(Icon::Playlist).render());
    }
    ScrollArea::new("queue-list", Element::column(rows).keep_leaving("upcoming-rows")).render().flex(1.0)
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

/// Across the bottom: what plays, the transport over the progress bar, and the rest.
fn player(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let current = state.current();
    let has_track = current.is_some();
    let wide = width >= RAIL_BELOW;
    let transport = |id: &str, icon: Icon, hint: &str| {
        IconButton::new(id, icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).tooltip(hint).enabled(has_track)
    };
    let now_playing = match current {
        Some(track) => Element::row(vec![
            cover(state, &track.artwork_url, 56.0, palette).radius(4.0).centered_y(),
            // The title leads to the album and the artist to their page.
            // A new track's name rises into place as the last one's leaves.
            Element::column(vec![
                Ticker::new(
                    "now-title",
                    &track.id,
                    Element::clickable(
                        "now-album",
                        Element::faded_text(&track.title, 24.0).font_family(FontFamily::Sans).font_size(14.0).foreground(palette.text),
                    ),
                )
                .height(20.0)
                .render(),
                Ticker::new(
                    "now-artist-line",
                    &track.id,
                    Element::clickable(
                        "now-artist",
                        Element::faded_text(state.player.error.as_deref().unwrap_or(&track.artist), 24.0)
                            .font_family(FontFamily::Sans)
                            .font_size(12.0)
                            .foreground(palette.muted),
                    ),
                )
                .height(18.0)
                .render(),
            ])
            .gap(2.0)
            .flex(1.0)
            .centered_y(),
        ])
        .gap(12.0),
        None => Element::spacer(),
    };
    let repeat_icon = if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat };
    // Play is the one filled control of the bar: a white circle.
    let round = if has_track {
        ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette }
    } else {
        ComponentPalette { surface: ROUND_OFF, muted: ROUND_OFF_INK, ..palette }
    };
    let play = IconButton::new("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, round)
        .variant(ButtonVariant::Subtle)
        .size(ControlSize::Regular)
        .round()
        .tooltip(if state.player.playing { "Pausar" } else { "Reproducir" })
        .enabled(has_track)
        .render()
        .center_content();
    let buttons = Element::row(vec![
        transport("shuffle", Icon::Shuffle, "Aleatorio").active(state.player.shuffle).render(),
        transport("previous", Icon::SkipBack, "Anterior").render(),
        play,
        transport("next", Icon::SkipForward, "Siguiente").render(),
        transport("repeat", repeat_icon, "Repetir").active(state.player.repeat != Repeat::Off).render(),
    ])
    .gap(8.0)
    .centered_x();
    let time = |seconds: f64| {
        Element::text(clock(seconds)).font_family(FontFamily::Monospace).font_size(11.0).foreground(palette.muted).centered_y()
    };
    let seek = Element::row(vec![
        time(f64::from(state.progress()) * state.player.duration),
        Slider::new("seek", state.progress(), palette).enabled(has_track && state.player.duration > 0.0).render().flex(1.0).centered_y(),
        time(state.player.duration),
    ])
    .gap(8.0);
    let side = |id: &str, icon: Icon, hint: &str, active: bool| {
        IconButton::new(id, icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).tooltip(hint).active(active).render().centered_y()
    };
    let equalizer = Popover::new(
        side("equalizer", Icon::Sliders, "Ecualizador", state.equalizer_open),
        equalizer(state, palette),
        state.equalizer_open,
        "equalizer-close",
        SCHEME.menu(),
    )
    .placement(PopoverPlacement::TopEnd)
    .width(36.0 + EQUALIZER_SLIDER + 44.0 + 16.0 + 24.0)
    .render()
    .centered_y();
    let mut controls = Vec::new();
    if wide {
        controls.push(Element::spacer().flex(1.0));
    }
    controls.extend([
        side("lyrics", Icon::Microphone, "Letra", state.lyrics_open),
        side("queue", Icon::Playlist, "Cola", state.queue_open),
        equalizer,
        IconButton::new("mute", if state.volume > 0.0 { Icon::Volume } else { Icon::VolumeMuted }, palette)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Compact)
            .tooltip(if state.volume > 0.0 { "Silenciar" } else { "Activar sonido" })
            .render()
            .centered_y(),
        Slider::new("volume", state.volume, palette).width(if wide { VOLUME_WIDTH } else { VOLUME_WIDTH_NARROW }).render().centered_y(),
    ]);
    let controls = Element::row(controls).gap(6.0).centered_y();
    let middle = Element::column(vec![buttons, seek]).gap(2.0).centered_y();
    // Wide, the three parts share the bar 3:4:3 and the transport sits in the middle of the
    // window. Narrow, the controls take only what they need.
    let parts = if wide {
        vec![now_playing.flex(3.0).centered_y(), middle.flex(4.0), controls.flex(3.0)]
    } else {
        vec![now_playing.flex(1.0).centered_y(), middle.flex(1.6), controls]
    };
    Element::panel(Element::row(parts).gap(16.0)).padding(GAP).size(None, Some(PLAYER_HEIGHT))
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
                assert!(found.iter().any(|id| id == "downloads"), "the way to what is kept on disk");
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
    fn the_right_panel_has_its_column_only_where_there_is_room() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, ..PlayerView::default() });
        let at = |state: &State, width: f32| ids(state, Size::new(width, 700.0));
        let has = |found: &[String], id: &str| found.iter().any(|found| found == id);
        let wide = at(&state, 1400.0);
        assert!(has(&wide, "panel-artist") && has(&wide, "resize:left") && has(&wide, "resize:right"), "three panels and their handles");
        let narrow = at(&state, 700.0);
        assert!(!has(&narrow, "panel-artist") && !has(&narrow, "resize:left"), "one panel beside a rail when narrow");
        assert!(has(&narrow, "downloads") && has(&narrow, "search"), "the rail and the search are still there");
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        assert!(has(&at(&state, 1400.0), "upcoming:0") && has(&at(&state, 700.0), "upcoming:0"), "the queue shows at any width");
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        assert!(!has(&at(&state, 1400.0), "resize:right"), "a closed panel leaves no handle");
    }

    #[test]
    fn playlists_settings_and_their_dialogs_offer_their_controls() {
        let mut state = State::default();
        let size = Size::new(1400.0, 1000.0);
        let has = |state: &State, wanted: &[&str]| {
            let found = ids(state, size);
            for id in wanted {
                assert!(found.iter().any(|found| found == id), "{id} is there: {found:?}");
            }
        };
        has(&state, &["playlist-new", "settings"]);
        let click = |state: &mut State, id: &str| drop(state.update(&limn::UiAction::Clicked { id: id.into() }));

        click(&mut state, "playlist-new");
        has(&state, &["playlist-name", "playlist-kind:normal", "playlist-kind:collaborative", "playlist-create", "playlist-cancel"]);
        click(&mut state, "playlist-kind:collaborative");
        click(&mut state, "playlist-create");
        has(&state, &["playlist:0", "playlist-delete"]);
        click(&mut state, "playlist-delete");
        has(&state, &["confirm-yes", "confirm-cancel"]);
        click(&mut state, "confirm-cancel");

        click(&mut state, "settings");
        has(&state, &["setting:autoplay", "setting:motion"]);
        assert!(ids(&state, Size::new(720.0, 1000.0)).iter().any(|id| id == "setting:autoplay"), "and in a narrow window");
    }

    #[test]
    fn the_chosen_filter_travels_to_its_place_instead_of_jumping() {
        use std::time::{Duration, Instant};
        let mut state = State::default();
        let size = Size::new(1400.0, 900.0);
        let start = Instant::now();
        let mut tree = UiTree::new(view(&state, size));
        // Where the chosen segment's fill is drawn, and where each filter sits.
        let mut chosen = |state: &State, now: Instant| {
            tree.set_root(view(state, size));
            let frame = tree.layout_at(size, now);
            let at = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
            let (all, albums) = (at("filter:all"), at("filter:albums"));
            let fill = frame.commands.iter().find_map(|command| match command {
                limn::DrawCommand::FillRect { rect, color, .. } if *color == SCHEME.canvas && rect.height == all.height && rect.y == all.y => Some(rect.x),
                _ => None,
            });
            (fill.expect("the chosen segment is filled"), all.x, albums.x)
        };
        let (rest, all, albums) = chosen(&state, start);
        assert_eq!(rest, all);
        state.update(&limn::UiAction::Clicked { id: "filter:albums".into() });
        chosen(&state, start + Duration::from_millis(1));
        let (midway, ..) = chosen(&state, start + Duration::from_millis(60));
        assert!(midway > all && midway < albums, "on its way: {midway} between {all} and {albums}");
        let (arrived, ..) = chosen(&state, start + Duration::from_millis(600));
        assert!((arrived - albums).abs() < 0.5, "and there: {arrived} at {albums}");
    }

    #[test]
    fn each_panel_is_shut_from_its_own_header_and_opened_from_the_bar_above() {
        let mut state = State::default();
        let at = |state: &State| ids(state, Size::new(1400.0, 700.0));
        let has = |found: &[String], id: &str| found.iter().any(|found| found == id);
        let open = at(&state);
        assert!(has(&open, "toggle-library") && has(&open, "toggle-panel"));
        assert!(!has(&open, "toggle-library-open") && !has(&open, "toggle-panel-open"));
        state.update(&limn::UiAction::Clicked { id: "toggle-library".into() });
        state.update(&limn::UiAction::Clicked { id: "toggle-panel".into() });
        let shut = at(&state);
        assert!(has(&shut, "toggle-library-open") && has(&shut, "toggle-panel-open"), "the bar above opens them: {shut:?}");
        assert!(!has(&shut, "downloads") && !has(&shut, "resize:left") && !has(&shut, "resize:right"), "nothing of a shut panel is left");
        assert!(has(&shut, "search") && has(&shut, "home"), "the bars stay");
    }

    #[test]
    fn home_and_the_library_offer_what_was_opened_and_what_is_kept() {
        let mut state = State::default();
        let size = Size::new(1400.0, 900.0);
        let has = |state: &State, id: &str| ids(state, size).iter().any(|found| found == id);
        assert!(has(&state, "filter:all") && has(&state, "filter:albums") && has(&state, "downloads"));
        assert!(!has(&state, "recent:0"), "nothing was opened yet");

        let album = Album { id: "lp".into(), title: "Album lp".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("t1")] };
        state.page = Page::Loading(Target::Album("lp".into()));
        state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album)) });
        assert!(has(&state, "keep"));
        state.update(&limn::UiAction::Clicked { id: "keep".into() });
        assert!(has(&state, "kept:0"), "the library lists it beside its page");
        state.update(&limn::UiAction::Clicked { id: "home".into() });
        assert!(has(&state, "recent:0"), "home offers what was opened");
        state.update(&limn::UiAction::Clicked { id: "filter:artists".into() });
        assert!(!has(&state, "downloads"), "a filter leaves only its kind in the library");
        assert!(ids(&state, Size::new(700.0, 900.0)).iter().any(|found| found == "recent:0"), "and at a narrow window");
    }

    #[test]
    fn the_panels_follow_the_widths_the_user_gave_them_and_spare_the_content() {
        let mut state = State::default();
        let edges = |state: &State, width: f32| {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let x = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect.x).unwrap();
            (x("resize:left"), x("resize:right"))
        };
        assert_eq!(edges(&state, 1400.0), (GAP + 280.0, 1400.0 - GAP - 320.0 - GAP));
        state.left_width = 400.0;
        state.right_width = 440.0;
        assert_eq!(edges(&state, 1400.0), (GAP + 400.0, 1400.0 - GAP - 440.0 - GAP));
        let (left, right) = edges(&state, 1040.0);
        assert!(right - (left + GAP) >= CONTENT_MIN_WIDTH, "the content keeps its width: {left}..{right}");
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
        has(&state, &["do:queue:track:0", "do:next:track:0", "do:share:track:0", "do:artist:track:0", "do:save:track:0"]);
        state.update(&limn::UiAction::Clicked { id: "do:share:track:0".into() });
        has(&state, &["share-card", "share-copy"]);
        state.update(&limn::UiAction::Clicked { id: "share-close".into() });
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
