//! The window, described from the state. Rebuilt on every frame.

mod mobile;
mod equalizer;
pub(crate) use mobile::route_key as mobile_route_key;
#[cfg(any(target_os = "ios", test))]
pub(crate) use mobile::{header_buttons, surface_tint, tabs, title as mobile_title};

use surco_audio::equalizer::BAND_HZ;
use surco_core::{Album, AlbumRef, Artist, Repeat, Track};
use limn::{
    ActionRow, ActionRowSizing, Alert, AlertDialog, Button, ButtonGroup, ButtonGroupSizing, ButtonVariant, Color, Component,
    ComponentPalette, ContextMenu, ContextMenuEntry, ContextMenuItem, ControlColors, ControlSize, DropdownMenu, DropdownMenuEntry,
    DropdownMenuItem, DropdownOption, Element, Empty, FontFamily, FormField, FormFieldSizing, Icon, IconButton, LevelBars, Link, ListRow, MediaCard, MediaTile,
    Modal, PageColumn, PageHeader, Popover, PopoverPlacement, Progress, RadiusRole, ResizeHandle, ListRowSizing, Scheme,
    Markdown, ScrollArea, SearchField, Select, SelectorSizing, Size, SkeletonGroup, Slider, Spinner, Switch, Tab, Table, TableColumn, TableRow, TableSizing, Tabs, Text,
    TextField, TextRole, Ticker, Toast, TooltipPlacement, TooltipSpec,
};

use crate::backend::{Release, Target};
use crate::clip::Showing;
use crate::state::{
    clock, greeting, large_cover, share_link, Confirm, Entry, Filter, LibraryView, Lyrics, Page, Playlist, Quality, Side, Sort, State, Updating, CROSSFADE_MAX, EQUALIZER_DB, RELEASE_LINK, SHELF,
};
use crate::update::Note;

/// The app's colors, one per role: near-black surfaces told apart by luminance, white ink.
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
/// The page's search field, fully open and as it starts to grow.
const PAGE_SEARCH_WIDTH: f32 = 220.0;
const PAGE_SEARCH_SHUT: f32 = 32.0;
/// Where the window presents the frames of the clip of what plays.
pub const CLIP_SURFACE: &str = "now-clip";
/// The glow of the video behind it, where it does not fill the window.
pub const CLIP_BACKDROP: &str = "now-clip-glow";
/// The mark of the lyrics: words in a bubble.
const LYRICS: &str = include_str!("../assets/icons/message-square-quote.svg");
// A route chooser is distinct from a waveform; UIKit supplies airplay.audio on iOS.
const AUDIO_ROUTE: &str = include_str!("../assets/icons/airplay-audio.svg");
/// A shelf with more tiles than this gets arrows to move along it.
const SHELF_FITS: usize = 4;
/// What dims a cover under what is drawn over it.
const COVER_SCRIM: Color = Color::rgba(0, 0, 0, 150);
/// A bar of the playing row's wave, and the space between two.
const WAVE_BAR: f32 = 3.0;
const WAVE_GAP: f32 = 2.0;
/// A line of lyrics reads from across the room; the furthest off are this much out of focus.
const LYRIC_SIZE: f32 = 24.0;
/// In a pane this wide or wider the lines grow to this.
const LYRIC_WIDE_FROM: f32 = 640.0;
const LYRIC_SIZE_WIDE: f32 = 32.0;
/// A phone holds the lyrics close: fewer, larger lines.
const LYRIC_SIZE_PHONE: f32 = 28.0;
const LYRIC_GAP: f32 = 18.0;
const LYRIC_BLUR: f32 = 2.2;
/// How far up the panel the cover's colour reaches behind the lyrics.
const LYRIC_WASH: f32 = 520.0;
/// How far down a page the colour of its cover reaches, from behind its header.
const HEADER_WASH: f32 = 360.0;
pub(crate) const MOBILE_HEADER_HEIGHT: f32 = 56.0;
const SYSTEM_OUTPUT: &str = "Predeterminado del sistema";

fn segments(group: &str, width: f32, tabs: Vec<Tab>) -> Element {
    Tabs::new(tabs, segmented_palette()).id(group).size(ControlSize::Compact).fill_width(width).render()
}

fn menu_sizing(state: &State) -> SelectorSizing {
    if state.mobile { SelectorSizing { row_height: 44.0, detail_row_height: 60.0, label_size: 15.0, ..SelectorSizing::COMPACT } }
    else { SelectorSizing::COMPACT }
}

/// The window is laid out as Spotify's: a bar with the search across the top, three rounded
/// panels a gap apart on a black floor, and the playback bar across the bottom. The gaps
/// beside the middle panel are the handles that resize the other two.
const CANVAS: Color = Color::rgb(0, 0, 0);
const CHOSEN: Color = Color::rgb(31, 31, 31);
/// The lifted surface of a phone promotion card, as Spotify's.
const PROMO: Color = Color::rgb(36, 36, 36);
const GAP: f32 = 8.0;
const PANEL_RADIUS: f32 = 8.0;
const TOP_BAR_HEIGHT: f32 = 56.0;
const SEARCH_MAX_WIDTH: f32 = 474.0;
/// The room each end of the top bar keeps: three header buttons, for the bell, the
/// settings and the way back to a panel that was shut.
const TOP_BAR_END: f32 = 80.0;
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
/// The popover: ten band columns of 34 inside its padding.
const EQUALIZER_WIDTH: f32 = 364.0;
const TILE_WIDTH: f32 = 148.0;
/// The space between two tiles of a shelf, the room under them for the shelf's bar, and
/// how far an arrow carries the shelf: three tiles.
const TILE_GAP: f32 = 12.0;
const SHELF_BAR_ROOM: f32 = 12.0;
pub const SHELF_STEP: f32 = 3.0 * (TILE_WIDTH + TILE_GAP);
const COVER: f32 = 40.0;
const NUMBER_WIDTH: f32 = 24.0;
const DURATION_WIDTH: f32 = 56.0;
const MENU_WIDTH: f32 = 272.0;
const DOWNLOAD: &str = include_str!("../assets/icons/download.svg");
/// limn's shuffle with a spark over where its arrows cross.
const SMART_SHUFFLE: &str = include_str!("../assets/icons/shuffle-spark.svg");
/// An entry of the library: 64 high around a 48 tile.
const LIBRARY_ROW: ListRowSizing = ListRowSizing {
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
    ..ListRowSizing::COMPACT
};
const LIBRARY_TILE: f32 = 48.0;
/// A quick-access entry of the home page: a short row around the same tile.
const QUICK_ROW: ListRowSizing = ListRowSizing { height: Some(64.0), padding: 8.0, gap: 12.0, leading_width: LIBRARY_TILE, title_size: 14.0, ..ListRowSizing::COMPACT };
/// A track's row: 60 high around a 40 cover, the title at 15 over a muted line at 12.
const TRACK_ROW: ListRowSizing = ListRowSizing {
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
    ..ListRowSizing::COMPACT
};
const ALBUM_COVER: f32 = 168.0;
const ARTIST_PICTURE: f32 = 128.0;
const PAGE_TITLE: f32 = 26.0;

pub fn view(state: &State, window: Size) -> Element {
    if state.mobile {
        return mobile::view(state, window);
    }
    let palette = SCHEME.palette();
    if state.cinema {
        return update_dialog(state, cinema(state, window, palette), palette);
    }
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
        columns.push(surface(pane(state, content, palette)).corner_mask(CANVAS).flex(1.0));
        columns.extend([handle(Side::Right, right_slide), sliding(panel(state, palette), right_whole, right)]);
    } else if state.queue_open && window.width < PANEL_FROM {
        columns.push(surface(panel(state, palette)).flex(1.0));
    } else {
        // A page's wash runs to the pane's edges: the mask keeps its corners round.
        columns.push(surface(pane(state, content, palette)).corner_mask(CANVAS).flex(1.0));
    }
    columns.push(gap());
    let window_tree = Element::panel(Element::column(vec![top_bar(state, window.width, palette), Element::row(columns).flex(1.0), player(state, window.width, palette)]))
        .background(CANVAS)
        .size(Some(window.width), Some(window.height));
    let shared = share_dialog(state, window_tree, window.width, palette);
    let created = playlist_dialog(state, shared, window.width, palette);
    let imported = missed_dialog(state, import_dialog(state, created, window.width, palette), window.width, palette);
    let whole = update_dialog(state, confirm_dialog(state, imported, palette), palette);
    Toast::layer(whole, update_toast(state, palette), PLAYER_HEIGHT + 12.0)
}

/// A later version found while the app is open, said over the window's corner until it
/// is closed or followed. One that has to be taken is said by its dialog.
fn update_toast(state: &State, palette: ComponentPalette) -> Vec<Element> {
    let release = state.update.as_ref().filter(|release| state.update_toast && !release.mandatory);
    let card = |release: &Release| {
        Toast::new(format!("Surco {} está disponible", release.version), palette).description("Mirá qué trae y actualizá cuando quieras.").action("update-toast").dismissible("update-toast-close").render()
    };
    release.map(card).into_iter().collect()
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
    if state.mobile {
        let name = TextField::new(
            "playlist-name",
            &draft.name,
            state.collection.default_name(),
            palette,
        )
        .size(ControlSize::Spacious)
        .render()
        .size(None, Some(44.0));
        let mut fields = FormField::new("Título", name, palette)
            .sizing(DIALOG_FIELD)
            .rows();
        if let Some(track) = &draft.first {
            fields.push(
                Text::new(format!("Empieza con «{}».", track.title), palette)
                    .muted()
                    .render(),
            );
        }
        let cancel = Button::new("playlist-cancel", "Cancelar", palette)
            .variant(ButtonVariant::Outline)
            .render()
            .size(None, Some(44.0));
        let create = Button::new("playlist-create", "Crear", palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            "Nueva playlist",
            "Juntá canciones bajo un nombre.",
            "playlist-cancel",
            palette,
        )
        .max_height(560.0)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, create))
        .initial_focus("playlist-name")
        .return_focus("playlist-new")
        .render();
    }
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

/// Bringing a list in from another app: its link there.
fn import_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some(link) = &state.importing else { return window_tree };
    if state.mobile {
        let field = TextField::new(
            "import-link",
            link,
            "Enlace de playlist o álbum público",
            palette,
        )
        .size(ControlSize::Spacious)
        .render()
        .size(None, Some(44.0));
        let mut fields = FormField::new("Enlace", field, palette).sizing(DIALOG_FIELD).rows();
        if cfg!(any(target_os = "ios", test)) {
            fields.push(Button::new("ios-paste:import-link", "Pegar enlace", palette)
                .variant(ButtonVariant::Secondary).sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round().render());
        }
        let cancel = Button::new("import-cancel", "Cancelar", palette)
            .variant(ButtonVariant::Outline)
            .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round()
            .render()
            .size(None, Some(44.0));
        let go = Button::new("import-go", "Importar", palette)
            .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round()
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            "Importar música",
            if state.import_unknown { "No reconocimos ese enlace." } else { "Spotify · Apple Music · YouTube Music" },
            "import-cancel",
            palette,
        )
        .max_height(560.0)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, go))
        .initial_focus("import-link")
        .render();
    }
    let field = TextField::new("import-link", link, "https://open.spotify.com/playlist/…", palette).size(ControlSize::Regular).render();
    let about = if state.import_unknown {
        "Ese enlace no es de una playlist de Spotify, Apple Music ni YouTube Music."
    } else {
        "De Spotify (hasta 100 canciones), Apple Music o YouTube Music: una playlist o un álbum, que sean públicos."
    };
    let cancel = Button::new("import-cancel", "Cancelar", palette).variant(ButtonVariant::Outline).size(ControlSize::Regular).render();
    let go = Button::new("import-go", "Importar", palette).size(ControlSize::Regular).render();
    let out = Text::new("Para llevar una playlist tuya a otra app, abrila y elegí «Copiar canciones».", palette).role(TextRole::Caption).muted().render();
    let mut fields: Vec<Element> = FormField::new("Enlace de la lista", field, palette).hint(about).sizing(DIALOG_FIELD).rows();
    fields.push(out);
    Modal::new(window_tree, true, "Importar de otras apps", "Queda como una playlist tuya, con las canciones que el catálogo tenga.", "import-cancel", palette)
        .width(Modal::fit_width(width, 460.0).max(240.0))
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, go))
        .initial_focus("import-link")
        .render()
}

/// The songs of a list brought in that the catalog does not have, to be looked for by hand.
fn missed_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some((_, missed)) = state.imported.as_ref().filter(|_| state.missed_open) else { return window_tree };
    if state.mobile {
        let rows = missed
            .iter()
            .map(|song| Text::new(song, palette).font_size(14.0).render())
            .collect();
        let copy = Button::new("import-missed-copy", "Copiar la lista", palette)
            .variant(ButtonVariant::Outline)
            .icon(Icon::Copy)
            .render()
            .size(None, Some(44.0));
        let close = Button::new("import-missed-close", "Cerrar", palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            "No se encontraron",
            "No están en el catálogo con ese nombre. Podés buscarlas a mano y agregarlas.",
            "import-missed-close",
            palette,
        )
        .max_height(600.0)
        .body(Element::column(rows).gap(12.0))
        .footer(dialog_footer(copy, close))
        .render();
    }
    let rows = missed.iter().map(|song| Text::new(song, palette).truncate().render()).collect();
    let copy = Button::new("import-missed-copy", "Copiar la lista", palette).variant(ButtonVariant::Outline).icon(Icon::Copy).size(ControlSize::Regular).render();
    let close = Button::new("import-missed-close", "Cerrar", palette).size(ControlSize::Regular).render();
    Modal::new(window_tree, true, "Canciones que no se encontraron", "No están en el catálogo con ese nombre. Podés buscarlas a mano y agregarlas.", "import-missed-close", palette)
        .width(Modal::fit_width(width, 460.0).max(240.0))
        .body(Element::column(rows).gap(6.0))
        .footer(dialog_footer(copy, close))
        .render()
}

/// The picture on the library's offer to bring music in, drawn for it (`assets/import.svg`)
/// and read once.
fn import_picture() -> Option<limn::RasterImage> {
    use base64::Engine as _;
    use sha2::Digest as _;
    static PICTURE: std::sync::OnceLock<Option<limn::RasterImage>> = std::sync::OnceLock::new();
    let read = || {
        let bytes: &[u8] = include_bytes!("../assets/import.png");
        let digest: String = sha2::Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        limn::RasterImage::new(digest, limn::RasterImageFormat::Png, bytes.len() as u32, encoded.into(), "Música de otras apps que llega a Surco")
    };
    PICTURE.get_or_init(read).clone()
}

/// The library's offer to bring music in from another app, until it is used or put away.
fn import_card(width: f32, palette: ComponentPalette, phone: bool, native: bool) -> Element {
    if phone {
        return phone_import_card(width, palette, native);
    }
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let away = Element::row(vec![Element::spacer().flex(1.0), on_picture("import-dismiss", Icon::Close, "No mostrar más", palette)]);
    // As wide as the card's own room, at the proportions it was drawn at.
    let across = width - 24.0;
    let top = match import_picture() {
        Some(picture) => {
            let picture = Element::raster_image(picture.cover()).size(Some(across), Some(across * 130.0 / 460.0)).radius(limn::tokens().radius.control);
            Element::stack(vec![picture, away.padding(6.0)])
        }
        None => away,
    };
    let title = Text::new("Traé tu música de otras apps", raised).render();
    let about = Text::new("Tus playlists de Spotify, Apple Music y YouTube Music, con su enlace.", raised).role(TextRole::Caption).muted().render();
    let go = Button::new("import-open", "Importar", raised).size(ControlSize::Compact).render();
    Element::panel(Element::column(vec![top, title, about, Element::row(vec![go]).gap(8.0)]).gap(8.0)).padding(12.0).background(CHOSEN).radius(RadiusRole::Card.px())
}

/// The phone's offer, after the Spotify kit's promotion cards: the picture edge to edge
/// over a lifted surface, a bold title, a quiet line and white capsules.
fn phone_import_card(width: f32, palette: ComponentPalette, native: bool) -> Element {
    let raised = ComponentPalette { surface: PROMO, ..palette };
    let close = mobile::icon("import-dismiss", Icon::Close, "No mostrar más", palette).background(Color { a: 140, ..CANVAS }).radius(22.0);
    let away = Element::row(vec![Element::spacer().flex(1.0), close]).padding(4.0);
    let top = match import_picture() {
        Some(picture) => Element::stack(vec![
            Element::raster_image(picture.cover()).size(Some(width), Some(width * 130.0 / 460.0)), away]),
        None => away,
    };
    let title = Text::new("Traé tu música de otras apps", raised).font_size(18.0).render().font_family(FontFamily::SansBold);
    let about = Text::new("Pegá el enlace de una playlist de Spotify, Apple Music o YouTube Music.", raised).font_size(14.0).muted().render();
    let go = Button::new("import-open", "Importar", raised)
        .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round().render();
    let mut actions = vec![go];
    if native && cfg!(any(target_os = "ios", test)) {
        // UIKit draws and handles UIPasteControl in this slot. Pasting prepares
        // the import form; fetching a playlist still requires its explicit submit.
        actions.push(Element::panel(Element::spacer()).size(Some(112.0), Some(44.0))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-paste:import-card").name("Pegar enlace de playlist")));
    }
    let actions = Element::row(actions).gap(8.0);
    let copy = Element::column(vec![title, about, Element::spacer().size(None, Some(6.0)), actions]).gap(6.0).padding(16.0);
    Element::panel(Element::column(vec![top, copy])).background(PROMO).radius(8.0).clip_children()
}

/// What cannot be undone asks before it is done.
fn confirm_dialog(state: &State, window_tree: Element, palette: ComponentPalette) -> Element {
    let (title, about, yes) = match &state.confirm {
        Some(Confirm::DeletePlaylist(id)) => {
            let name = state.collection.playlist(*id).map_or("esta playlist", |playlist| playlist.name.as_str());
            (format!("¿Eliminar «{name}»?"), "La playlist se borra de tu biblioteca. Las canciones no se tocan.", "Eliminar")
        }
        Some(Confirm::ClearDownloads) => ("¿Eliminar las descargas?".to_owned(), "Ya no vas a tener acceso a lo que descargaste en este equipo.", "Eliminar"),
        Some(Confirm::ClearQueue) => ("¿Borrar la cola?".to_owned(), "Se quitan las canciones que agregaste a la cola. Lo que venía sonando sigue.", "Borrar"),
        Some(Confirm::ClearRecent) => ("¿Borrar el historial de reproducciones?".to_owned(), "Los accesos rápidos de Inicio quedan vacíos.", "Borrar"),
        None => (String::new(), "", ""),
    };
    if state.mobile {
        let cancel = Button::new("confirm-cancel", "Cancelar", palette)
            .variant(ButtonVariant::Outline)
            .render()
            .size(None, Some(44.0));
        let confirm = Button::new("confirm-yes", yes, palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            state.confirm.is_some(),
            "Confirmar acción",
            format!("{title}\n\n{about}"),
            "confirm-cancel",
            palette,
        )
        .max_height(480.0)
        .footer(dialog_footer(cancel, confirm))
        .initial_focus("confirm-cancel")
        .backdrop_dismiss(false)
        .render();
    }
    AlertDialog::new(window_tree, state.confirm.is_some(), title, about, "confirm-yes", "confirm-cancel", palette)
        .confirm_label(yes)
        .cancel_label("Cancelar")
        .render()
}

/// The card of a later version: what stands, how far taking it in has got, and what the
/// user can do about it. While it comes down, a bar under the words fills with it.
fn update_card(state: &State, release: &Release, width: f32, palette: ComponentPalette) -> Element {
    let button = |id: &str, label: &str| Button::new(id, label, palette).size(ControlSize::Regular).render().centered_y();
    let version = &release.version;
    let (title, about, action) = match &state.updating {
        Updating::Idle => {
            let about = format!("Surco {version} · tenés la {}", env!("CARGO_PKG_VERSION"));
            ("Actualización disponible".to_owned(), about, Some(button("update-get", if release.program.is_some() { "Actualizar" } else { "Descargar" })))
        }
        // The last of it is checked and put in place: a moment with nothing left to count.
        Updating::Fetching(done) if *done >= 1.0 => (format!("Instalando Surco {version}"), "Un momento…".to_owned(), None),
        Updating::Fetching(_) => (format!("Descargando Surco {version}"), "Podés seguir escuchando mientras tanto.".to_owned(), None),
        Updating::Ready => (format!("Surco {version} está lista"), "Reiniciá para empezar a usarla.".to_owned(), Some(button("update-restart", "Reiniciar ahora"))),
        Updating::Failed(why) => ("No se pudo actualizar".to_owned(), why.clone(), Some(button("update-get", "Reintentar"))),
    };
    let copy = Element::column(vec![Text::new(title, palette).font_size(15.0).render(), Text::new(about, palette).muted().render()]).gap(2.0).flex(1.0);
    let mut head = vec![copy];
    let mut parts = Vec::new();
    if let Updating::Fetching(done) = &state.updating {
        let percent = Element::text(format!("{:.0} %", done * 100.0)).font_family(FontFamily::Monospace).font_size(12.0).foreground(palette.muted);
        head.push(percent.centered_y());
        parts.push(Progress::new(*done, palette).height(6.0).render());
    }
    head.extend(action);
    parts.insert(0, Element::row(head).gap(12.0));
    parts.extend(release_notes(state, release, (width - 32.0).min(NOTES_WIDTH), palette));
    Element::panel(Element::column(parts).gap(12.0)).padding(16.0).background(palette.surface).radius(RadiusRole::Panel.px())
}

/// The widest what a release says of itself is set: a line longer than this is hard to read.
const NOTES_WIDTH: f32 = 600.0;

/// What a release says of itself: its text as it was written, and its pictures as wide
/// as the text, each in a room of a screen's proportions.
fn release_notes(state: &State, release: &Release, width: f32, palette: ComponentPalette) -> Vec<Element> {
    let radius = limn::tokens().radius.control;
    let part = |note: &Note| match note {
        Note::Text(text) => Markdown::new(text, palette).width(width).link_action_prefix(RELEASE_LINK).render().element,
        Note::Picture(address) => match state.artwork.get(address) {
            Some(picture) => Element::raster_image(picture.clone().full_resolution()).size(Some(width), Some(width * 9.0 / 16.0)).radius(radius),
            None => Element::panel(Element::spacer()).background(palette.selected).radius(radius).size(Some(width), Some(width * 9.0 / 16.0)),
        },
    };
    release.notes.iter().map(part).collect()
}

/// A version that may not keep running stands in front of everything until the later one
/// is taken in.
/// How wide the notice of a version that has to be taken is, and how tall the room in it
/// for what the version says of itself, which scrolls.
const UPDATE_DIALOG: f32 = 520.0;
const UPDATE_NOTES_ROOM: f32 = 240.0;

fn update_dialog(state: &State, window_tree: Element, palette: ComponentPalette) -> Element {
    let Some(release) = state.update.as_ref().filter(|release| release.mandatory) else { return window_tree };
    let (about, action) = match &state.updating {
        Updating::Idle if release.program.is_some() => (format!("Esta versión de Surco ya no se puede usar. Instalá la {} para seguir.", release.version), ("update-get", "Actualizar ahora")),
        Updating::Idle => (format!("Esta versión de Surco ya no se puede usar. Descargá la {} para seguir.", release.version), ("update-get", "Descargar")),
        Updating::Fetching(done) => (format!("Descargando la versión {}… {:.0} %", release.version, done * 100.0), ("update-wait", "Descargando…")),
        Updating::Ready => (format!("La versión {} ya está instalada.", release.version), ("update-restart", "Reiniciar ahora")),
        Updating::Failed(why) => (format!("No se pudo actualizar: {why}"), ("update-get", "Reintentar")),
    };
    let mut body = Vec::new();
    if let Updating::Fetching(done) = &state.updating {
        body.push(Progress::new(*done, palette).height(6.0).render());
    }
    let notes = release_notes(state, release, UPDATE_DIALOG - 48.0, palette);
    if !notes.is_empty() {
        body.push(ScrollArea::new("update-notes", Element::column(notes).gap(12.0)).render().size(None, Some(UPDATE_NOTES_ROOM)));
    }
    // No way around it: the other button leads to the release's own page.
    let dialog = AlertDialog::new(window_tree, true, "Actualización obligatoria", about, action.0, "update-page", palette).confirm_label(action.1).cancel_label("Ver la versión").width(UPDATE_DIALOG);
    if body.is_empty() { dialog } else { dialog.body(Element::column(body).gap(12.0)) }.render()
}

/// A release in the feed of what is new: a row around its cover, larger than a track's.
const NEWS_COVER: f32 = 56.0;
const NEWS_ROW: ListRowSizing = ListRowSizing { height: Some(72.0), two_line_height: Some(72.0), leading_width: NEWS_COVER, ..TRACK_ROW };

/// What the artists followed have released, what is still to be looked at first, under
/// the notice of a later version of the app when there is one.
fn news_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut sections = vec![Element::column(vec![
        Text::new("Novedades", palette).font_size(PAGE_TITLE).render(),
        Text::new("Los últimos lanzamientos de los artistas que seguís.", palette).muted().render(),
    ])
    .gap(4.0)];
    if let Some(release) = &state.update {
        let raised = ComponentPalette { surface: CHOSEN, ..palette };
        sections.push(update_card(state, release, width - 24.0, raised));
    }
    let row = |(index, (release, _)): (usize, &(AlbumRef, bool))| {
        let about = [release.artist.as_deref().unwrap_or_default(), release.year.as_str()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
        ListRow::new(format!("news:{index}"), &release.title, palette)
            .sizing(NEWS_ROW)
            .leading(middle(cover(state, &release.artwork_url, NEWS_COVER, palette)))
            .subtitle(about)
            .title_fade(true)
            .radius(RadiusRole::Card)
            .render()
    };
    let news = &state.collection.news;
    let fresh = state.news_fresh.min(news.len());
    if fresh > 0 {
        sections.extend([heading("Nuevo", palette), Element::column(news[..fresh].iter().enumerate().map(row).collect()).gap(2.0)]);
    }
    if news.len() > fresh {
        let earlier = news.iter().enumerate().skip(fresh).map(row).collect();
        sections.extend([heading("Anterior", palette), Element::column(earlier).gap(2.0)]);
    }
    if news.is_empty() && state.update.is_none() {
        let empty = Empty::new("Todavía no hay novedades", "Seguí a tus artistas favoritos y acá te avisamos cuando saquen algo.", palette).icon(Icon::Bell).render();
        return Element::column(vec![sections.remove(0), centered(empty)]).gap(16.0).padding(12.0).flex(1.0);
    }
    ScrollArea::new("news", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
}

/// The narrowest a tile of the library gets, and the width of the menu of its orders and views.
const LIBRARY_GRID_TILE: f32 = 112.0;
/// What the library's row of tools keeps for the cross of its search and the button of its order.
/// The share of its row the library's search has as it starts to open: about a magnifier's width.
const LIBRARY_SEARCH_SHUT: f32 = 0.15;
const LIBRARY_MENU_WIDTH: f32 = 220.0;

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
    if state.mobile {
        let Some(track) = &state.sharing else {
            return window_tree;
        };
        let playing = state.player.playing
            && state
                .current()
                .is_some_and(|current| current.id == track.id);
        let picture = Element::clickable(
            "share-card",
            cover(state, &track.artwork_url, 160.0, palette),
        )
        .accessible(limn::Access::new(limn::Role::Button).name(format!(
            "{} {}",
            if playing { "Pausar" } else { "Reproducir" },
            track.title
        )))
        .centered_x();
        let copy = Button::new(
            "share-copy",
            if state.copied {
                "Enlace copiado"
            } else {
                "Copiar enlace"
            },
            palette,
        )
        .icon(if state.copied {
            Icon::Check
        } else {
            Icon::Copy
        })
        .render()
        .size(None, Some(44.0));
        let body = Element::column(vec![
            picture,
            Text::new(&track.title, palette).font_size(20.0).render(),
            Text::new(&track.artist, palette).muted().render(),
            Text::new(share_link(track), palette)
                .mono()
                .muted()
                .truncate()
                .render(),
        ])
        .gap(12.0);
        return limn::BottomSheet::new(
            window_tree,
            true,
            "Compartir canción",
            "El enlace la abre en YouTube Music; pegado en el buscador de Surco, se abre acá.",
            "share-close",
            palette,
        )
        .max_height(640.0)
        .body(body)
        .footer(copy)
        .render();
    }
    let (card, link) = match &state.sharing {
        Some(track) => (media_card(state, "share-card", track, palette), share_link(track)),
        None => (Element::spacer(), String::new()),
    };
    let copy = if state.copied { Button::new("share-copy", "Enlace copiado", palette).icon(Icon::Check) } else { Button::new("share-copy", "Copiar enlace", palette).icon(Icon::Copy) };
    Modal::new(window_tree, state.sharing.is_some(), "Compartir canción", "El enlace la abre en YouTube Music; pegado en el buscador de Surco, se abre acá.", "share-close", palette)
        .width(Modal::fit_width(width, 420.0))
        .body(Element::column(vec![card.centered_x(), Text::new(link, palette).mono().muted().truncate().render()]).gap(12.0))
        .footer(ButtonGroup::new(vec![copy.size(ControlSize::Regular).render()]).align_end().render())
        .render()
}

/// The side of the chip on the bell's corner.
const BELL_CHIP: f32 = 14.0;

/// Across the top: the way back, home, and the search in the middle of the window. A side
/// panel that is shut is opened from here, on its own side.
fn top_bar(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let field = SearchField::new("search", &state.query, "¿Qué querés escuchar?", SCHEME.on_raised()).width((width - 280.0).clamp(160.0, SEARCH_MAX_WIDTH)).render();
    // What was typed is taken back from a cross at the field's end, filled as the field is so it covers a long text.
    let search = if state.query.is_empty() {
        field
    } else {
        let clear = IconButton::new("search-clear", Icon::Close, SCHEME.on_raised()).variant(ButtonVariant::Subtle).size(ControlSize::Compact).round().tooltip("Borrar").render().centered_y();
        Element::stack(vec![field, Element::row(vec![Element::spacer().flex(1.0), clear]).padding(6.0)])
    }
    .centered_y();
    let mut leading = Vec::new();
    if !state.library_open {
        leading.push(header_button("toggle-library-open", Icon::PanelLeftOpen, TooltipSpec::new("Mostrar tu biblioteca").shortcut("Ctrl+B"), palette).render().centered_y());
    }
    leading.push(header_button("back", Icon::ArrowLeft, TooltipSpec::new("Volver"), palette).enabled(state.can_go_back()).render().centered_y());
    let mut trailing = vec![Element::spacer().flex(1.0)];
    // The bell is full while it has something to say.
    let bell = if state.has_news() { Icon::BellFill } else { Icon::Bell };
    let bell = header_button("news", bell, TooltipSpec::new("Novedades"), palette).active(state.page == Page::News).render();
    // How many things it has to say, on its corner.
    let bell = match state.news_count() {
        0 => bell,
        count => {
            let said = if count > 9 { "9+".to_owned() } else { count.to_string() };
            let chip = Element::panel(Element::text(said).font_size(10.0).foreground(SCHEME.canvas).centered()).background(palette.text).radius(BELL_CHIP / 2.0).size(Some(if count > 9 { BELL_CHIP + 6.0 } else { BELL_CHIP }), Some(BELL_CHIP));
            Element::stack(vec![bell, Element::row(vec![Element::spacer().flex(1.0), chip])])
        }
    };
    trailing.push(bell.centered_y());
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

/// One row of the library.
struct LibraryEntry<'a> {
    /// What a click reports.
    id: String,
    /// What it is wherever it stands, for a pin to hold on to.
    key: String,
    picture: Element,
    /// Its cover, where it has one, and whether that is a face: for the view in tiles.
    artwork: Option<&'a String>,
    round: bool,
    name: &'a str,
    about: String,
    /// Whose it is; empty for what is the user's own.
    creator: &'a str,
    /// Its page is the one shown.
    here: bool,
}

/// The left panel: what the user keeps, under the filters that narrow it. Folded, it is a
/// rail of pictures.
fn library(state: &State, rail: bool, width: f32, palette: ComponentPalette) -> Element {
    let shows = |filter: Filter| state.filter == Filter::All || state.filter == filter;
    let mut entries: Vec<LibraryEntry> = Vec::new();
    if shows(Filter::Lists) {
        let count = match state.saved.len() {
            1 => "Playlist · 1 canción".to_owned(),
            count => format!("Playlist · {count} canciones"),
        };
        entries.push(LibraryEntry { id: "downloads".into(), key: "downloads".into(), picture: library_tile(Icon::Custom(DOWNLOAD), palette), artwork: None, round: false, name: "Descargas", about: count, creator: "", here: state.page == Page::Downloads });
        if !state.collection.liked.is_empty() {
            let count = format!("Playlist · {}", songs(state.collection.liked.len()));
            entries.push(LibraryEntry { id: "liked".into(), key: "liked".into(), picture: library_tile(Icon::HeartFill, palette), artwork: None, round: false, name: "Tus me gusta", about: count, creator: "", here: state.page == Page::Liked });
        }
        for (index, playlist) in state.collection.playlists.iter().enumerate() {
            let picture = match playlist.tracks.first() {
                Some(track) => cover(state, &track.artwork_url, LIBRARY_TILE, palette),
                None => library_tile(Icon::Playlist, palette),
            };
            let about = format!("{} · {}", if playlist.collaborative { "Playlist colaborativa" } else { "Playlist" }, songs(playlist.tracks.len()));
            let artwork = playlist.tracks.first().and_then(|track| track.artwork_url.as_ref());
            entries.push(LibraryEntry { id: format!("playlist:{index}"), key: format!("playlist:{}", playlist.id), picture, artwork, round: false, name: &playlist.name, about, creator: "", here: state.page == Page::Playlist(playlist.id) });
        }
    }
    if shows(Filter::Artists) {
        for (index, artist) in state.collection.artists() {
            let picture = cover(state, &artist.artwork_url, LIBRARY_TILE, palette).radius(LIBRARY_TILE / 2.0);
            let here = matches!(&state.page, Page::Artist(page) if page.id == artist.id);
            entries.push(LibraryEntry { id: format!("kept:{index}"), key: format!("artist:{}", artist.id), picture, artwork: artist.artwork_url.as_ref(), round: true, name: &artist.name, about: "Artista".into(), creator: &artist.name, here });
        }
    }
    if shows(Filter::Albums) {
        for (index, album) in state.collection.albums() {
            let about = album.artist.as_ref().map_or_else(|| "Álbum".to_owned(), |artist| format!("Álbum · {artist}"));
            let here = matches!(&state.page, Page::Album(page) if page.id == album.id);
            let picture = cover(state, &album.artwork_url, LIBRARY_TILE, palette);
            entries.push(LibraryEntry { id: format!("kept:{index}"), key: format!("album:{}", album.id), picture, artwork: album.artwork_url.as_ref(), round: false, name: &album.title, about, creator: album.artist.as_deref().unwrap_or(""), here });
        }
    }
    match state.settings.library_sort {
        Sort::Added => {}
        Sort::Alphabetical => entries.sort_by_cached_key(|entry| entry.name.to_lowercase()),
        Sort::Creator => entries.sort_by_cached_key(|entry| (entry.creator.to_lowercase(), entry.name.to_lowercase())),
    }
    // What is pinned goes first, in the order it was pinned; the rest keeps its sort.
    let pinned = |key: &str| state.collection.pinned.iter().position(|pinned| pinned == key);
    entries.sort_by_key(|entry| pinned(&entry.key).unwrap_or(usize::MAX));
    if rail {
        let pictures = entries.into_iter().map(|entry| Element::clickable(entry.id, entry.picture));
        let shut = header_button("toggle-library", Icon::PanelLeftClose, TooltipSpec::new("Ocultar tu biblioteca").shortcut("Ctrl+B"), palette);
        let head = Element::row(vec![Element::spacer().flex(1.0), shut.render().centered_y(), Element::spacer().flex(1.0)]).size(Some(LIBRARY_TILE), Some(40.0));
        return ScrollArea::new("library", Element::column(std::iter::once(head).chain(pictures).collect()).gap(8.0).padding(8.0)).render();
    }
    // A panel is shut from its own header.
    let mut header = vec![
        Text::new(if state.mobile { "Guardado para vos" } else { "Tu biblioteca" }, palette).font_size(15.0).muted().render().centered_y(),
        Element::spacer().flex(1.0),
    ];
    // A phone makes playlists from the page's own bar.
    if !state.mobile {
        header.push(IconButton::new("playlist-new", Icon::Add, palette).variant(ButtonVariant::Ghost).size(ControlSize::Dense).tooltip("Crear playlist").accessibility_label("Crear playlist").render().centered_y());
        header.insert(0, header_button("toggle-library", Icon::PanelLeftClose, TooltipSpec::new("Ocultar tu biblioteca").shortcut("Ctrl+B"), palette).render().centered_y());
    }
    let header = Element::row(header)
    .gap(8.0)
    .size(None, Some(if state.mobile { 44.0 } else { 40.0 }));
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
    let filters = if state.mobile {
        let pill = |id: &str, label: &str, selected| Button::new(id, label, palette)
            .variant(if selected { ButtonVariant::Default } else { ButtonVariant::Secondary })
            .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Regular.button_sizing() })
            .round().render();
        Element::horizontal_scroll_2d("mobile-library-filters", Element::row(vec![
            pill("filter:all", "Todo", state.filter == Filter::All),
            pill("filter:lists", "Playlists", state.filter == Filter::Lists),
            pill("filter:artists", "Artistas", state.filter == Filter::Artists),
            pill("filter:albums", "Álbumes", state.filter == Filter::Albums),
        ]).gap(8.0)).drag_to_pan().scrollbar_visibility(mobile::SCROLL_INDICATOR)
    } else { filters };
    // A magnifier that opens into the search, and at the far end the order and the view.
    let looking = match state.filter {
        Filter::All => "Buscar en tu biblioteca",
        Filter::Lists => "Buscar en Playlists",
        Filter::Artists => "Buscar en Artistas",
        Filter::Albums => "Buscar en Álbumes",
    };
    let open = state.library_search_slide.value(std::time::Instant::now()).clamp(0.0, 1.0);
    let search_button = |glyph, hint| if state.mobile { mobile::icon("library-search", glyph, hint, palette).radius(22.0).centered_y() }
        else { header_button("library-search", glyph, TooltipSpec::new(hint), palette).render().centered_y() };
    let mut tools = if open > 0.0 {
        // It grows out of where the magnifier was to all the row has left beside the order,
        // however wide the order's name leaves that: the room is shared out, not counted.
        let grown = open.max(LIBRARY_SEARCH_SHUT);
        let field = SearchField::new("library-query", &state.library_search, looking, SCHEME.on_raised()).size(if state.mobile { ControlSize::Spacious } else { ControlSize::Compact }).render();
        let field = if state.mobile { field.size(None, Some(44.0)).radius(22.0) } else { field };
        vec![field.centered_y().flex(grown), search_button(Icon::Close, "Cerrar la búsqueda"), Element::spacer().flex(1.0 - grown)]
    } else {
        vec![search_button(Icon::Search, looking), Element::spacer().flex(1.0)]
    };
    let (sort, view) = (state.settings.library_sort, state.settings.library_view);
    let choice = |id: &str, label: &str, chosen: bool| DropdownMenuEntry::Item(DropdownMenuItem::new(id, label).selected(chosen));
    let choices = vec![
        DropdownMenuEntry::Heading("Clasificar por".into()),
        choice("library-sort:added", "Agregado recientemente", sort == Sort::Added),
        choice("library-sort:alphabetical", "Alfabéticamente", sort == Sort::Alphabetical),
        choice("library-sort:creator", "Creador", sort == Sort::Creator),
        DropdownMenuEntry::Separator,
        DropdownMenuEntry::Heading("Ver como".into()),
        choice("library-view:compact", "Compacta", view == LibraryView::Compact),
        choice("library-view:list", "Lista", view == LibraryView::List),
        choice("library-view:grid", "Cuadrícula", view == LibraryView::Grid),
    ];
    let picker = Button::new("library-sort", sort.label(), palette).icon(Icon::Sort).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render();
    let picker = if state.mobile { picker.size(None, Some(44.0)).radius(22.0) } else { picker };
    // All of its choices at once: past eight a menu becomes a list that scrolls, with a rail's room beside it.
    let all = choices.len();
    tools.push(DropdownMenu::new(picker, choices, state.library_menu, "library-menu-close", SCHEME.menu()).sizing(menu_sizing(state)).width(LIBRARY_MENU_WIDTH).max_visible_items(all).placement(PopoverPlacement::BottomEnd).render().centered_y());
    let tools = Element::row(tools).gap(8.0).size(None, Some(if state.mobile { 44.0 } else { ControlSize::Compact.height() }));

    let wanted = state.library_search.trim().to_lowercase();
    // Tiles share the panel's width, two of them at the least.
    let columns = (((width - 16.0 + GAP) / (LIBRARY_GRID_TILE + GAP)).floor() as usize).max(2);
    let tile_width = (width - 16.0 - GAP * (columns - 1) as f32) / columns as f32;
    let shown: Vec<Element> = entries
        .into_iter()
        .filter(|entry| entry.name.to_lowercase().contains(&wanted) || entry.about.to_lowercase().contains(&wanted))
        .map(|LibraryEntry { id, key, picture, artwork, round, name, about, here, .. }| {
            let held = pinned(&key).is_some();
            let about = if held { format!("Fijado · {about}") } else { about };
            // The page being shown stands out by its fill.
            let palette = if here { ComponentPalette { surface: CHOSEN, ..palette } } else { palette };
            let entry = match view {
                LibraryView::Grid => {
                    let tile = MediaTile::new(id, name, palette).width(tile_width).round(round).subtitle(about);
                    match artwork.and_then(|url| state.artwork.get(url)) {
                        Some(image) => tile.artwork(Element::raster_image(image.clone().cover())),
                        None => tile,
                    }
                    .render()
                }
                // A line each: what it is called, and what it is.
                LibraryView::Compact => {
                    let kind = about.rsplit(" · ").last().unwrap_or_default().to_owned();
                    let sizing = if state.mobile { ListRowSizing { height: Some(44.0), ..ListRowSizing::COMPACT } } else { ListRowSizing::COMPACT };
                    let row = ListRow::new(id, name, palette).sizing(sizing).trailing(Text::new(kind, palette).role(TextRole::Caption).muted().render()).title_fade(true).radius(RadiusRole::Card);
                    if here { row.filled() } else { row }.render()
                }
                LibraryView::List => {
                    let row = ListRow::new(id, name, palette).sizing(LIBRARY_ROW).leading(picture).subtitle(about).title_fade(true).radius(RadiusRole::Card);
                    if here { row.filled() } else { row }.render()
                }
            };
            if state.mobile {
                let more = format!("library-more:{key}");
                let dots = mobile::icon(&more, Icon::More, &format!("Opciones de {name}"), palette);
                let entry = if view == LibraryView::Grid {
                    Element::stack(vec![entry, Element::row(vec![Element::spacer().flex(1.0), dots.background(SCHEME.raised).radius(22.0)]).padding(4.0)])
                } else { Element::row(vec![entry.flex(1.0), dots.centered_y()]).gap(4.0) };
                return ContextMenu::new(format!("library-context:{key}"), entry, Vec::new(), SCHEME.menu()).open_action(more).render();
            }
            let pin = ContextMenuItem::new(format!("pin:{key}"), if held { "Desfijar" } else { "Fijar arriba" });
            ContextMenu::new(format!("library-context:{key}"), entry, vec![ContextMenuEntry::Item(pin)], SCHEME.menu()).render()
        })
        .collect();
    let list = if shown.is_empty() {
        let what = if !wanted.is_empty() {
            format!("No encontramos «{}». Probá escribirlo de otra forma.", state.library_search.trim())
        } else if state.filter == Filter::Artists { "Seguí a un artista desde su página y aparece acá.".to_owned() } else { "Guardá un álbum desde su página y aparece acá.".to_owned() };
        Element::panel(Text::new(what, palette).muted().render()).padding(8.0)
    } else if view == LibraryView::Grid {
        let mut tiles = shown.into_iter().peekable();
        let mut rows = Vec::new();
        while tiles.peek().is_some() {
            rows.push(Element::row(tiles.by_ref().take(columns).collect()).gap(GAP));
        }
        ScrollArea::new("library", Element::column(rows).gap(GAP)).render().flex(1.0)
    } else {
        ScrollArea::new("library", Element::column(shown).gap(2.0)).render().flex(1.0)
    };
    let mut list = list;
    let mut parts = vec![Element::panel(header).padding(8.0), filters, tools];
    if !state.settings.import_dismissed && wanted.is_empty() {
        let card = import_card(width - 16.0, palette, state.mobile, state.mobile_ui.native_surfaces);
        if state.mobile {
            if let limn::ElementKind::Scroll { child, .. } = &mut list.kind {
                let rows = std::mem::replace(child.as_mut(), Element::spacer());
                **child = Element::column(vec![card, rows]).gap(16.0);
            } else {
                list = ScrollArea::new("library", Element::column(vec![card, list]).gap(16.0)).render().flex(1.0);
            }
        } else { parts.push(card); }
    }
    parts.push(list);
    Element::column(parts).gap(6.0).padding(8.0)
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
    let mut sections = if state.mobile { Vec::new() } else { vec![Text::new(greeting(hour), palette).font_size(PAGE_TITLE).render()] };

    // Quick access: a grid of short rows, each a step above the page.
    let columns = if width >= 720.0 { 3 } else { 2 };
    let side = ((width - 24.0 - (columns - 1) as f32 * GAP) / columns as f32).max(120.0);
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let quick = |id: String, name: &str, picture: Element| {
        let element = ListRow::new(&id, name, raised).sizing(QUICK_ROW).leading(picture).title_fade(true).radius(RadiusRole::Card).width(side).filled().render();
        mobile_card_context(state, &id, element)
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
    if state.mobile { tiles.truncate(8); }
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
        sections.push(shelf(state, "home-heard", "Volver a escuchar", heard, palette));
    }
    // The catalog's picks: a mix across what goes with everything played, then who and what
    // to open next, then what goes with each of the latest favourites.
    let mix: Vec<Element> = state.mix.iter().enumerate().map(|(index, track)| pick(format!("mix:{index}"), track)).collect();
    if !mix.is_empty() {
        sections.push(shelf(state, "home-mix", "Hecho para vos", mix, palette));
    }
    let artists: Vec<Element> = state
        .picked_artists
        .iter()
        .enumerate()
        .map(|(index, artist)| tile(state, format!("pick-artist:{index}"), &artist.name, Some("Artista".into()), &artist.artwork_url, true, palette))
        .collect();
    if !artists.is_empty() {
        sections.push(shelf(state, "home-pick-artists", "Artistas para descubrir", artists, palette));
    }
    if !state.picked_albums.is_empty() {
        sections.push(shelf(state, "home-pick-albums", "Discos para vos", release_tiles(state, "pick-album", &state.picked_albums, palette), palette));
    }
    for (shelf_index, (seed, tracks)) in state.because.iter().enumerate() {
        let picks: Vec<Element> = tracks.iter().enumerate().map(|(index, track)| pick(format!("rec:{shelf_index}:{index}"), track)).collect();
        if !picks.is_empty() {
            sections.push(shelf(state, &format!("home-rec:{shelf_index}"), &format!("Porque escuchaste «{}»", seed.title), picks, palette));
        }
    }

    let albums: Vec<Element> = collection
        .albums()
        .map(|(index, album)| {
            tile(state, format!("home-kept:{index}"), &album.title, album.artist.clone(), &album.artwork_url, false, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !albums.is_empty() {
        sections.push(shelf(state, "home-albums", "Tus álbumes", albums, palette));
    }
    let artists: Vec<Element> = collection
        .artists()
        .map(|(index, artist)| {
            tile(state, format!("home-kept:{index}"), &artist.name, Some("Artista".into()), &artist.artwork_url, true, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !artists.is_empty() {
        sections.push(shelf(state, "home-artists", "Tus artistas", artists, palette));
    }
    ScrollArea::new("home", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
}

/// A page's content under the colour of its cover, which gives way to the surface below
/// its header.
fn washed(state: &State, artwork: &Option<String>, content: Element) -> Element {
    match artwork.as_deref().and_then(|url| state.tints.get(url)) {
        Some(tint) => content.top_background_fade(Color { a: 150, ..*tint }, HEADER_WASH),
        None => content,
    }
}

fn centered(element: Element) -> Element {
    Element::column(vec![Element::spacer().flex(1.0), element.centered_x(), Element::spacer().flex(1.0)]).flex(1.0)
}

fn heading(title: &str, palette: ComponentPalette) -> Element {
    Text::new(title, palette).role(TextRole::Heading).render()
}

/// A heading over a row of tiles that scrolls sideways inside the page: by dragging it, by
/// the trackpad, or a few tiles at a time with the arrows beside the heading.
fn shelf(state: &State, id: &str, title: &str, tiles: Vec<Element>, palette: ComponentPalette) -> Element {
    let arrow = |way: &str, icon: Icon, hint: &str| {
        IconButton::new(format!("shelf-{way}:{id}"), icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Dense).tooltip(hint).render().centered_y()
    };
    let mut header = vec![heading(title, palette).flex(1.0)];
    // A shelf that fits has nowhere to go.
    if !state.mobile && tiles.len() > SHELF_FITS {
        header.extend([arrow("back", Icon::ArrowLeft, "Anteriores"), arrow("on", Icon::ArrowRight, "Siguientes")]);
    }
    // Under the tiles, room for the strip's own bar: a tile lit under the pointer stops short of it.
    let tiles = Element::column(vec![Element::row(tiles).gap(TILE_GAP), Element::spacer().size(None, Some(SHELF_BAR_ROOM))]);
    let strip = Element::horizontal_scroll_2d(id, tiles).drag_to_pan().foreground(palette.muted);
    let strip = if state.mobile { strip.scrollbar_visibility(mobile::SCROLL_INDICATOR) } else { strip };
    Element::column(vec![Element::row(header).gap(4.0), strip]).gap(8.0)
}

fn tile(state: &State, id: String, title: &str, subtitle: Option<String>, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    let row = id.clone();
    let tile = MediaTile::new(id, title, palette).width(TILE_WIDTH).round(round);
    let tile = match artwork.as_ref().and_then(|url| state.artwork.get(url)) {
        Some(image) => tile.artwork(Element::raster_image(image.clone().cover())),
        None => tile,
    };
    let element = match subtitle {
        Some(subtitle) => tile.subtitle(subtitle),
        None => tile,
    }
    .render();
    mobile_card_context(state, &row, element)
}

fn mobile_card_context(state: &State, row: &str, element: Element) -> Element {
    if !state.mobile { return element; }
    let Some(action) = state.card_context_action(row) else { return element; };
    let target = state.track_at(row).map_or_else(|| format!("tile-context:{row}"),
        |track| format!("tile-context:{row}:{}", track.id));
    // Touch and hold owns contextual actions; keep the cover and its primary
    // tap target clear. UIKit supplies the iOS menu, with this retained target
    // also serving keyboard and non-iOS context requests.
    ContextMenu::new(target, element,
        vec![ContextMenuEntry::Item(ContextMenuItem::new(&action, "Opciones"))], SCHEME.menu())
        .open_action(action).render()
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
pub(crate) struct Action {
    /// What a click on it reports.
    pub(crate) id: String,
    pub(crate) label: &'static str,
    icon: Icon,
    /// A second line: what the action leads to.
    pub(crate) detail: Option<String>,
}

/// What a row's menu offers for its track. `None` is a divider.
pub(crate) fn track_actions(state: &State, row: &str, track: &Track) -> Vec<Option<Action>> {
    let item = |action: &str, label, icon, detail| Some(Action { id: format!("do:{action}:{row}"), label, icon: Icon::Custom(icon), detail });
    let mut actions = vec![
        item("queue", "Agregar a la cola", include_str!("../assets/icons/list-plus.svg"), None),
        item("next", "Poner a continuación", include_str!("../assets/icons/list-start.svg"), None),
        item("share", "Compartir", include_str!("../assets/icons/share-2.svg"), None),
        Some(if state.is_liked(&track.id) {
            Action { id: format!("do:like:{row}"), label: "Quitar de Tus me gusta", icon: Icon::HeartFill, detail: None }
        } else {
            Action { id: format!("do:like:{row}"), label: "Me gusta", icon: Icon::Heart, detail: None }
        }),
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

/// What plays says so with its own sound: a bar for each band of the engine's meter, on
/// springs. With movement reduced it is the still glyph.
fn wave(state: &State, id: String, palette: ComponentPalette) -> Element {
    if state.settings.reduce_motion {
        return Element::icon(Icon::Audio).foreground(palette.text).size(Some(16.0), Some(16.0));
    }
    let bars = LevelBars { id, levels: state.levels.to_vec(), bar: WAVE_BAR, gap: WAVE_GAP, color: palette.text, rest: palette.text };
    let width = state.levels.len() as f32 * (WAVE_BAR + WAVE_GAP) - WAVE_GAP;
    Element::level_bars(bars).size(Some(width), Some(16.0))
}

/// A track's cover as its row shows it. Over it, what plays wears its own sound; under
/// the pointer every cover says what a click on the row does: it pauses what plays and
/// plays the rest.
fn sleeve(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> Element {
    let current = state.current().is_some_and(|current| current.id == track.id);
    let over = |content: Element| {
        Element::panel(content.centered()).background(COVER_SCRIM).radius(limn::tokens().radius.control).size(Some(COVER), Some(COVER))
    };
    let does = Element::icon(if current && state.player.playing { Icon::PauseFill } else { Icon::PlayFill }).foreground(palette.text);
    let mut layers = vec![cover(state, &track.artwork_url, COVER, palette)];
    if current {
        layers.push(over(wave(state, format!("wave:{row}"), palette)).conceal_on_hover(format!("sounding:{row}")));
    }
    layers.push(over(does).reveal_on_hover(format!("does:{row}")));
    Element::stack(layers)
}

/// The dots of a row with the menu they open, and the same actions for the menu the row
/// opens where it is pressed.
fn row_menu(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> (Element, Vec<ContextMenuEntry>) {
    let open = state.menu.as_deref() == Some(row);
    let actions = track_actions(state, row, track);
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
    let pressed = actions.into_iter().map(|action| match action {
        Some(action) => ContextMenuEntry::Item(ContextMenuItem::new(action.id, action.label).icon(action.icon)),
        None => ContextMenuEntry::Separator,
    });
    (menu, pressed.collect())
}

/// After a row's copy: that the track is kept on disk or on its way there, and how long
/// it lasts.
fn row_facts(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> Vec<Element> {
    let mut facts = Vec::new();
    if state.saving.contains(&track.id) {
        facts.push(Spinner::new(format!("saving:{row}"), palette.muted).size(12.0).render().centered_y());
    } else if state.is_saved(&track.id) {
        facts.push(Element::icon(Icon::Custom(DOWNLOAD)).foreground(palette.muted).size(Some(14.0), Some(14.0)).centered_y());
    }
    if track.duration_ms > 0 {
        let length = Element::text(clock(track.duration_ms as f64 / 1000.0)).font_family(FontFamily::Monospace).font_size(12.0).foreground(palette.muted);
        facts.push(length.centered_x().size(Some(DURATION_WIDTH), None).centered_y());
    }
    facts
}

/// A track among what a search found: its cover, what it is and whose, and after them
/// what is known of it.
fn track_row(state: &State, row: String, track: &Track, subtitle: &str, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::track_row(state, row, track, subtitle, palette); }
    let (menu, pressed) = row_menu(state, &row, track, palette);
    let mut list_row = ListRow::new(&row, &track.title, palette)
        .sizing(TRACK_ROW)
        .leading(middle(sleeve(state, &row, track, palette)))
        .subtitle(subtitle)
        .title_fade(true)
        .radius(RadiusRole::Card);
    let facts = row_facts(state, &row, track, palette);
    if !facts.is_empty() {
        list_row = list_row.trailing(middle(Element::row(facts).gap(8.0)));
    }
    // The dots show while the pointer is on the row, or while their menu is open. A trailing
    // action sits flush with the row's edge: the row's own padding keeps them off it.
    let dots = Element::row(vec![menu, Element::spacer().size(Some(TRACK_ROW.padding), None)]).reveal_on_hover(format!("dots:{row}"));
    let element = list_row.trailing_action(format!("more:{row}"), middle(dots)).render().hover_region();
    ContextMenu::new(format!("context:{row}"), element, pressed, SCHEME.menu()).render()
}

/// The width a page's table has in a pane `width` wide: the pane's, less the page's margins
/// and the room its scroll area keeps for its bar.
fn table_width(width: f32) -> f32 {
    width - 24.0 - 14.0
}

/// The narrowest a title gets, the table width under which the album gives its place,
/// and the room of the last column, for a row's dots.
const TITLE_MIN: f32 = 160.0;
const ALBUM_FROM: f32 = 520.0;
const MORE_WIDTH: f32 = 36.0;

/// A page's tracks as a table `width` wide: what each is, the album it is on and how
/// long it lasts, under heads that order them. `leaving` names the rows of a list that
/// can lose one, which then closes where it was.
/// How far apart a page's song rows stand, each row and the space after it.
pub(crate) fn track_pitch(state: &State) -> f32 {
    if state.mobile { MOBILE_TRACK_PITCH } else { TRACK_TABLE_ROW }
}
pub(crate) const MOBILE_TRACK_PITCH: f32 = 72.0;
const TRACK_TABLE_ROW: f32 = 60.0;
/// A list up to this long is built whole; a longer one only around what is in view.
const BUILT_WHOLE: usize = 120;
/// Rows are built in blocks this long, so scrolling rebuilds the list seldom.
const ROW_BLOCK: usize = 16;

/// Which of `count` rows `pitch` apart are built: those in view and a screen's worth on
/// either side. `window` is where the list's top was against its scroll area at the last
/// layout, and how tall the area is; a short list, or one not measured yet, is built whole.
pub(crate) fn track_range(count: usize, pitch: f32, window: Option<(f32, f32)>) -> std::ops::Range<usize> {
    let Some((top, height)) = window.filter(|_| count > BUILT_WHOLE && pitch > 0.0) else { return 0..count };
    let row = |at: f32| (at / pitch).max(0.0) as usize;
    let first = (row(-top - height) / ROW_BLOCK * ROW_BLOCK).min(count);
    let last = (row(-top + 2.0 * height) + 1).div_ceil(ROW_BLOCK) * ROW_BLOCK;
    first..last.clamp(first, count)
}

/// Rows of a song list with the ones `range` leaves out stood in for by space of their
/// height, `gap` apart, so the list is as tall and its rows where they would be.
fn windowed(rows: Vec<Element>, range: std::ops::Range<usize>, count: usize, pitch: f32, gap: f32) -> Element {
    let room = |rows: usize| Element::spacer().size(None, Some((rows as f32 * pitch - gap).max(0.0)));
    let mut column = Vec::with_capacity(rows.len() + 2);
    if range.start > 0 { column.push(room(range.start)); }
    column.extend(rows);
    if range.end < count { column.push(room(count - range.end)); }
    Element::column(column).gap(gap).anchor(TRACK_LIST)
}

/// The top of a page's song rows, as the Host measures it to know which are in view.
pub(crate) const TRACK_LIST: &str = "track-list";

fn track_table(state: &State, rows: &[(usize, &Track)], album: bool, leaving: Option<String>, width: f32, palette: ComponentPalette) -> Element {
    let count = rows.len();
    let range = track_range(count, track_pitch(state), state.track_window);
    if state.mobile {
        let built = rows[range.clone()].iter().map(|(place, track)| mobile::track_row(state, format!("page-track:{place}"), track, &track.artist, palette)).collect();
        return windowed(built, range, count, MOBILE_TRACK_PITCH, 4.0);
    }
    let first = range.start;
    let rows = &rows[range.clone()];
    let mut columns = vec![TableColumn::fixed("index", "#", NUMBER_WIDTH).align_end(), TableColumn::flexible("title", "Título", TITLE_MIN).sortable()];
    if album {
        columns.push(TableColumn::fixed("album", "Álbum", state.album_width).sortable().resizable(ALBUM_FROM - TITLE_MIN - 280.0).optional(state.album_shown).hide_below(ALBUM_FROM));
    }
    columns.push(TableColumn::fixed("duration", "Duración", DURATION_WIDTH + 22.0).icon(Icon::Clock).sortable().align_end().optional(state.duration_shown));
    columns.push(TableColumn::fixed("more", "", MORE_WIDTH));
    let mut menus = Vec::new();
    let listed: Vec<TableRow> = rows
        .iter()
        .enumerate()
        .map(|(position, (place, track))| {
            let position = first + position;
            let row = format!("page-track:{place}");
            let current = state.current().is_some_and(|current| current.id == track.id);
            // Numbered as they read, whatever their place in the list's own order.
            let number = Element::text((position + 1).to_string()).font_family(FontFamily::Monospace).font_size(12.0).foreground(if current { palette.text } else { palette.muted });
            let copy = Element::column(vec![
                Text::new(&track.title, palette).font_size(TRACK_ROW.title_size).truncate().render(),
                Text::new(&track.artist, palette).font_size(TRACK_ROW.subtitle_size).muted().truncate().render(),
            ])
            .gap(TRACK_ROW.copy_gap)
            .flex(1.0)
            .centered_y();
            let mut cells = vec![number, Element::row(vec![sleeve(state, &row, track, palette).centered_y(), copy]).gap(TRACK_ROW.gap).flex(1.0)];
            if album {
                cells.push(Text::new(track.album.as_deref().unwrap_or_default(), palette).muted().truncate().render());
            }
            cells.push(Element::row(row_facts(state, &row, track, palette)).gap(8.0));
            let (menu, pressed) = row_menu(state, &row, track, palette);
            menus.push(pressed);
            cells.push(menu.reveal_on_hover(format!("dots:{row}")));
            TableRow::new(&row, cells).controls([format!("more:{row}")])
        })
        .collect();
    let sizing = TableSizing { row_height: TRACK_TABLE_ROW, ..TableSizing::ROOMY };
    let (header, built) = Table::new("tracks", columns, width, palette)
        .rows(listed)
        .sort(state.page_sort.clone())
        .menu_palette(SCHEME.menu())
        .radius(RadiusRole::Card)
        .sizing(sizing)
        .into_parts();
    let body = built.into_iter().zip(menus).zip(rows).map(|((element, pressed), (place, track))| {
        let element = ContextMenu::new(format!("context:page-track:{place}"), element.hover_region(), pressed, SCHEME.menu()).render();
        match &leaving {
            Some(list) => Element::column(vec![element]).animate_height(format!("{list}:{}", track.id), false),
            None => element,
        }
    });
    let body = windowed(body.collect(), range.clone(), count, TRACK_TABLE_ROW, 0.0);
    // The rows a column remembers are named by where the built window starts: rows that only
    // scroll out of it are not removed and must not close, while one taken out of the list
    // closes where it was. limn forgets a name the frame no longer lays out.
    let body = match &leaving {
        Some(list) => body.keep_leaving(format!("{list}:rows:{}", range.start)),
        None => body,
    };
    Element::column(vec![header, body]).gap(sizing.header_gap)
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
        Page::Liked => "page:liked".to_owned(),
        Page::Playlist(id) => format!("page:playlist:{id}"),
        Page::Library => "page:library".to_owned(),
        Page::Playing => "page:playing".to_owned(),
        Page::Settings => "page:settings".to_owned(),
        Page::Lyrics => "page:lyrics".to_owned(),
        Page::News => "page:news".to_owned(),
        Page::Equalizer => "page:equalizer".to_owned(),
    };
    page(state, width, palette).animate_enter(entering)
}

fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    match &state.page {
        Page::Search if state.submitted.is_none() && !state.searching => home(state, width, palette),
        Page::Search => results(state, palette),
        // Each of its songs is looked for in the catalog, which takes a while.
        Page::Loading(Target::Import(_)) => {
            let (said, done) = match state.import_progress {
                Some((done, of)) if of > 0 => (format!("Buscando las canciones en el catálogo: {done} de {of}."), done as f32 / of as f32),
                _ => ("Leyendo la lista…".to_owned(), 0.0),
            };
            let about = Text::new("Podés seguir escuchando o ir a otra página: la playlist aparece en tu biblioteca cuando termina.", palette).role(TextRole::Caption).muted().render();
            let progress = Element::column(vec![Text::new(said, palette).render(), Progress::new(done, palette).height(6.0).render(), about]).gap(8.0);
            Element::column(vec![Element::panel(progress).padding(12.0), loading("opening", palette)])
        }
        Page::Loading(_) => loading("opening", palette),
        Page::Failed(_, message) => centered(
            Empty::new("No se pudo abrir", message, palette)
                .icon(Icon::WarningTriangle)
                .action("retry", "Reintentar", ButtonVariant::Secondary)
                .render(),
        ),
        Page::Artist(artist) => artist_page(state, artist, width, palette),
        Page::Album(album) => album_page(state, album, width, palette),
        Page::Song(track) => centered(
            Element::column(vec![
                media_card(state, "song-card", track, palette).centered_x(),
                Text::new("Tocá la tapa para escucharla.", palette).muted().render().centered_x(),
            ])
            .gap(12.0),
        ),
        Page::Downloads => downloads_page(state, width, palette),
        Page::Library => library(state, false, width, palette),
        Page::Playing => mobile::playing(state, width, palette),
        Page::Liked => liked_page(state, width, palette),
        Page::Lyrics => lyrics(state, width, 0.0, palette),
        Page::News => news_page(state, width, palette),
        Page::Playlist(id) => match state.collection.playlist(*id) {
            Some(playlist) => playlist_page(state, playlist, width, palette),
            None => centered(Empty::new("Esta playlist ya no existe", "Fue eliminada de tu biblioteca.", palette).icon(Icon::Playlist).render()),
        },
        Page::Settings => settings_page(state, width, palette),
        Page::Equalizer => mobile::settings::equalizer(state, width),
    }
}

/// "1 canción", "12 canciones".
fn songs(count: usize) -> String {
    if count == 1 { "1 canción".to_owned() } else { format!("{count} canciones") }
}

/// One of the user's playlists: what it is, what to do with it, and its tracks in order.
fn playlist_page(state: &State, playlist: &Playlist, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::playlist(state, playlist, width, palette); }
    let minutes = playlist.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000;
    let length = if minutes >= 60 { format!("{} h {} min", minutes / 60, minutes % 60) } else { format!("{minutes} min") };
    let picture = match playlist.tracks.first() {
        Some(track) => cover(state, &track.artwork_url, ALBUM_COVER, palette),
        None => Element::panel(Element::icon(Icon::Playlist).large_icon(48.0).foreground(palette.muted).centered())
            .background(CHOSEN)
            .radius(limn::tokens().radius.control)
            .size(Some(ALBUM_COVER), Some(ALBUM_COVER)),
    };
    // Icons, as the play and the shuffle beside them: the row has to fit a narrow pane.
    let own = |id: &str, icon: Icon, hint: &str| IconButton::new(id, icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).tooltip(hint).render();
    let copied = state.list_copied == Some(playlist.id);
    let share = own("playlist-share", if copied { Icon::Check } else { Icon::Custom(include_str!("../assets/icons/share-2.svg")) }, if copied { "Enlace copiado" } else { "Compartir" });
    let exported = state.list_exported == Some(playlist.id);
    let export = own("playlist-export", if exported { Icon::Check } else { Icon::Custom(include_str!("../assets/icons/upload.svg")) }, if exported { "Canciones copiadas: pegalas en la otra app" } else { "Copiar canciones para otra app" });
    let delete = own("playlist-delete", Icon::Trash, "Eliminar");
    let actions = if playlist.tracks.is_empty() {
        // Nothing to play, to share or to search yet.
        Element::row(vec![delete])
    } else {
        page_actions(state, vec![share, export, delete], palette)
    };
    let missed = state.imported.as_ref().filter(|(id, missed)| *id == playlist.id && !missed.is_empty()).map(|(_, missed)| {
        let said = if missed.len() == 1 { "1 canción no se encontró".to_owned() } else { format!("{} canciones no se encontraron", missed.len()) };
        Button::new("import-missed", said, palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render()
    });
    let mut about = vec![
        Text::eyebrow(if playlist.collaborative { "PLAYLIST COLABORATIVA" } else { "PLAYLIST" }, palette).render(),
        Text::new(&playlist.name, palette).font_size(PAGE_TITLE).truncate().render(),
        Text::new(format!("{} · {length}", songs(playlist.tracks.len())), palette).muted().render(),
    ];
    // What a list brought in from another app left behind, a click from being read.
    about.extend(missed.map(|missed| Element::row(vec![missed])));
    let header = Element::row(vec![picture, Element::column(about).gap(10.0).flex(1.0).centered_y()]).gap(20.0);
    let list = if playlist.tracks.is_empty() {
        Element::panel(
            Empty::new("Todavía no tiene canciones", "Buscá una canción y elegí «Agregar a» en su menú.", palette).icon(Icon::MusicNote).render().centered_x(),
        )
        .padding(24.0)
    } else {
        track_table(state, &state.page_rows(), true, Some(format!("playlist-row:{}", playlist.id)), table_width(width), palette)
    };
    let cover = playlist.tracks.first().and_then(|track| track.artwork_url.clone());
    ScrollArea::new("playlist", washed(state, &cover, Element::column(vec![header, actions, list]).gap(16.0).padding(12.0))).render().flex(1.0)
}

/// A size on disk as people read it.
fn megabytes(bytes: u64) -> String {
    let megabytes = bytes as f64 / 1_048_576.0;
    if megabytes >= 1024.0 { format!("{:.1} GB", megabytes / 1024.0) } else { format!("{megabytes:.0} MB") }
}

/// A settings row's measures: its name over what it does, a control at the end.
const SETTINGS_ROW: ActionRowSizing = ActionRowSizing { gap: 24.0, copy_gap: 8.0, icon_column: 20.0, min_height: 56.0, min_height_with_subtitle: 64.0 };

/// What the user can set: one page of groups, each row its name and what it does beside
/// its control. A narrow page stacks the control under the copy.
fn settings_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::settings::page(state, width); }
    let column = PageColumn::new("settings", width, palette).max_width(900.0).gutter(24.0, 32.0, 720.0).gap(32.0).reserve_rail(true);
    // A switch or a short button fits beside its row's copy in less room than a field would.
    let compact = column.content_width() < 600.0;
    let picker_width = (column.content_width() - 20.0).min(288.0);
    let (card, control) = (SCHEME.palette(), SCHEME.on_raised());
    let row = |title: &str, about: &str| ActionRow::new(title, card).subtitle(about).wrap_title(true).sizing(SETTINGS_ROW);
    let switch = |title: &str, about: &str, id: &str, on: bool| row(title, about)
        .trailing(Switch::new(id, on, control).size(ControlSize::Spacious).accessibility_label(title).render())
        .stacked(compact).render();
    let action = |id: &str, label: &str, enabled: bool| Button::new(id, label, control).variant(ButtonVariant::Outline).size(ControlSize::Spacious).enabled(enabled).render();
    let group = |title: &str, rows: Vec<Element>| Element::column(vec![
        Text::new(title, card).role(TextRole::Heading).render(),
        Element::column(rows).gap(8.0),
    ]).gap(16.0);
    let choices = |group: &str, tabs: Vec<Tab>| Tabs::new(tabs, segmented_palette()).id(group).size(ControlSize::Spacious).fill_width(picker_width).render();

    let equalizer = equalizer::summary(state);
    let downloads = format!("{} · {}", songs(state.saved.len()), megabytes(state.saved_bytes));
    let device = |id: &str, name: &str| DropdownOption::new(format!("setting:output:{id}"), name).selected(state.output.as_deref().unwrap_or_default() == id);
    let system_output = state.outputs.iter().find(|output| output.is_default)
        .map_or_else(|| SYSTEM_OUTPUT.to_owned(), |output| format!("Sistema: {}", output.name));
    let devices = std::iter::once(device("", &system_output)).chain(state.outputs.iter().map(|output| {
        let name = if output.is_default { format!("{} · predeterminado", output.name) } else { output.name.clone() };
        device(&output.id, &name)
    }));
    let playing_through = match &state.output {
        Some(id) => state.outputs.iter().find(|output| output.id == *id).map_or("Salida guardada no disponible", |output| output.name.as_str()),
        None => system_output.as_str(),
    };
    let output = Select::new("setting:output", playing_through, devices.collect(), state.output_open, "setting:output-dismiss", control)
        .menu_palette(SCHEME.menu())
        .accessibility_label("Dispositivo de salida")
        .size(ControlSize::Spacious)
        .width(picker_width)
        .render();
    let chosen = state.settings.quality;
    let quality = choices(
        "setting-quality",
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
    let output_status = if state.outputs_loading {
        "Buscando salidas de audio…".to_owned()
    } else if let Some(error) = &state.outputs_error {
        format!("No pudimos consultar las salidas. {error}")
    } else if state.outputs.is_empty() {
        "No se encontraron salidas adicionales. Podés seguir usando la predeterminada del sistema.".to_owned()
    } else {
        format!("{} {} en este equipo.", state.outputs.len(), if state.outputs.len() == 1 { "salida disponible" } else { "salidas disponibles" })
    };
    let mut connections = vec![
        row("Dispositivo de salida", "Elegí por dónde escuchar: parlantes, auriculares o una salida Bluetooth conectada al sistema.")
            .trailing(output).stacked(compact).render(),
        row("Salidas disponibles", &output_status)
            .trailing(action("setting:outputs-refresh", if state.outputs_loading { "Buscando…" } else { "Actualizar" }, !state.outputs_loading))
            .stacked(compact).render(),
        row("Conectar por Bluetooth", "Emparejá tus auriculares o parlantes desde la configuración Bluetooth del sistema. Después, actualizá las salidas y elegilos arriba.").render(),
    ];
    if let Some(notice) = &state.output_notice {
        connections.push(Alert::new("Salida de audio", notice, card).id("setting:output-notice").compact(compact).render());
    }
    let library_sort = choices("setting-library-sort", vec![
        Tab::new("library-sort:added", "Recientes", 0.0).active(state.settings.library_sort == Sort::Added),
        Tab::new("library-sort:alphabetical", "A–Z", 0.0).active(state.settings.library_sort == Sort::Alphabetical),
        Tab::new("library-sort:creator", "Creador", 0.0).active(state.settings.library_sort == Sort::Creator),
    ]);
    let library_view = choices("setting-library-view", vec![
        Tab::new("library-view:compact", "Compacta", 0.0).active(state.settings.library_view == LibraryView::Compact),
        Tab::new("library-view:list", "Lista", 0.0).active(state.settings.library_view == LibraryView::List),
        Tab::new("library-view:grid", "Cuadrícula", 0.0).active(state.settings.library_view == LibraryView::Grid),
    ]);
    let visualization = vec![
        switch("Mostrar biblioteca", "Dejá tus playlists, artistas y álbumes a mano en el panel izquierdo.", "setting:library-panel", state.library_open),
        switch("Mostrar lo que suena", "Mostrá la tapa, la letra y el artista en el panel derecho.", "setting:player-panel", state.panel_open),
        switch("Videos en lo que suena", "Muestra un tramo del video de la canción, sin sonido, donde va su tapa.", "setting:clips", state.settings.clips),
        switch("Reducir movimiento", "Muestra los cambios de una vez, sin animaciones.", "setting:motion", state.settings.reduce_motion),
    ];
    let body = vec![
        PageHeader::new("Configuración", card).description("Los cambios se guardan en este equipo.").stacked(compact).render(),
        group(
            "Reproducción",
            vec![
                switch("Reproducción automática", "Cuando termina lo que estás escuchando, sigue con algo similar.", "setting:autoplay", state.settings.autoplay),
                row("Crossfade entre canciones", "Mezcla el final de una canción con el comienzo de la siguiente.").trailing(crossfade).stacked(compact).render(),
            ],
        ),
        group("Conexiones y dispositivos", connections),
        group(
            "Calidad de audio",
            vec![
                row("Calidad de reproducción", &format!("{} Cambia a partir de la próxima canción.", ceiling)).trailing(quality).stacked(compact).render(),
                switch("Normalizar el volumen", "Baja las canciones más fuertes para que todas suenen al mismo nivel.", "setting:normalize", state.settings.normalize),
                switch("Audio mono", "Los dos parlantes reproducen lo mismo.", "setting:mono", state.settings.mono),
                row("Ecualizador", &format!("Diez bandas y un previo. Ahora: {equalizer}.")).trailing(action("equalizer", "Ajustar", true)).stacked(compact).render(),
            ],
        ),
        group("Visualización", visualization),
        group(
            "Biblioteca",
            vec![
                row("Orden de la biblioteca", "Elegí qué aparece primero entre tus álbumes, artistas y playlists.").trailing(library_sort).stacked(compact).render(),
                row("Vista de la biblioteca", "Cambiá cuánto detalle ves en el panel izquierdo.").trailing(library_view).stacked(compact).render(),
                row("Importar música de otras apps", "Trae playlists de Spotify, Apple Music y YouTube Music con su enlace.").trailing(action("import-open", "Importar", true)).stacked(compact).render(),
            ],
        ),
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
                row("Borrar abiertos recientemente", "Quita de Inicio los álbumes y artistas que abriste en este equipo.")
                    .trailing(action("recent-clear", "Borrar", !state.collection.recent.is_empty()))
                    .stacked(compact)
                    .render(),
            ],
        ),
        group(
            "Acerca de",
            vec![
                row("Versión", "Surco para escritorio.").value(env!("CARGO_PKG_VERSION")).stacked(compact).render(),
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
        sections.push(shelf(state, "found-artists", "Artistas", tiles.collect(), palette));
    }
    if !found.albums.is_empty() {
        sections.push(shelf(state, "found-albums", "Álbumes", release_tiles(state, "album", &found.albums, palette), palette));
    }
    if !found.tracks.is_empty() {
        let rows = found.tracks.iter().enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, &track.artist, palette));
        sections.extend([heading("Canciones", palette), Element::column(rows.collect()).gap(2.0)]);
    }
    ScrollArea::new("results", Element::column(sections).gap(12.0).padding(12.0)).render().flex(1.0)
}

/// The shuffle's three steps as its button shows them: the icon of the one it is on, and
/// what a press does next.
fn shuffle_step(state: &State) -> (Icon, &'static str) {
    match (state.player.shuffle, state.player.smart) {
        (false, _) => (Icon::Shuffle, "Activar aleatorio"),
        (true, false) => (Icon::Shuffle, "Activar aleatorio con recomendaciones"),
        (true, true) => (Icon::Custom(SMART_SHUFFLE), "Desactivar aleatorio"),
    }
}

/// A page's actions, in a row of their own under its header: its play, the one filled
/// control, the step of the shuffle, the page's `own`, and the search of its tracks at the
/// far end.
fn page_actions(state: &State, own: Vec<Element>, palette: ComponentPalette) -> Element {
    let round = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let play = if state.mobile {
        mobile::controls::playback("page-play", false, 56.0, 28.0, palette)
    } else {
        IconButton::new("page-play", Icon::PlayFill, round).variant(ButtonVariant::Subtle).size(ControlSize::Spacious).round().tooltip("Reproducir").render().center_content()
    };
    let (icon, hint) = shuffle_step(state);
    let shuffle = if state.mobile {
        let action = mobile::controls::action("page-shuffle", icon, hint, 22.0, palette);
        if state.player.shuffle { action.background(palette.selected) } else { action }
    } else {
        IconButton::new("page-shuffle", icon, palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).accessibility_label(hint).active(state.player.shuffle).render()
    };
    let mut actions = vec![play, shuffle.centered_y()];
    actions.extend(own.into_iter().map(Element::centered_y));
    if state.mobile {
        actions.push(Element::spacer().flex(1.0));
        return Element::column(vec![Element::row(actions).gap(12.0), Element::row(page_search(state, palette)).gap(8.0)]).gap(8.0);
    }
    actions.extend(page_search(state, palette));
    Element::row(actions).gap(8.0)
}

/// The actions of a page the library can hold: `keep` names keeping it, before and after.
fn play_buttons(state: &State, keep: Option<(&str, &str)>, palette: ComponentPalette) -> Element {
    let keep = keep.map(|(keep, kept)| {
        let (label, icon) = if state.keeps_page() { (kept, Icon::Check) } else { (keep, Icon::Add) };
        Button::new("keep", label, palette).icon(icon).variant(ButtonVariant::Ghost).render()
    });
    page_actions(state, keep.into_iter().collect(), palette)
}

/// The search of the page's tracks, at the far end of its actions: a magnifier that grows
/// into a field, with the cross that shuts it where the magnifier was.
fn page_search(state: &State, palette: ComponentPalette) -> Vec<Element> {
    let open = state.page_search_slide.value(std::time::Instant::now()).clamp(0.0, 1.0);
    let toggle = |icon: Icon, hint: &str| {
        if state.mobile { mobile::icon("page-search", icon, hint, palette).radius(22.0).centered_y() }
        else { header_button("page-search", icon, TooltipSpec::new(hint), palette).render().centered_y() }
    };
    let mut parts = vec![Element::spacer().flex(1.0)];
    if open > 0.0 {
        let width = PAGE_SEARCH_SHUT + (PAGE_SEARCH_WIDTH - PAGE_SEARCH_SHUT) * open;
        let field = SearchField::new("page-query", &state.page_search, "Buscar en esta página", SCHEME.on_raised())
            .size(ControlSize::Compact)
            .width(width)
            .render();
        let field = if state.mobile { field.size(Some(width), Some(44.0)).radius(22.0) } else { field };
        let field = field.centered_y()
            // In a narrow pane it makes do with what the row has left.
            .shrink_to_fit();
        parts.extend([field, toggle(Icon::Close, "Cerrar la búsqueda")]);
    } else {
        parts.push(toggle(Icon::Search, "Buscar en esta página"));
    }
    // Beside it, the order the page's tracks are listed in.
    let sorted = state.page_sort.as_ref().map_or("own", |sort| sort.key.as_str());
    let order = |key: &str, label: &str| DropdownMenuEntry::Item(DropdownMenuItem::new(format!("page-sort:{key}"), label).selected(sorted == key));
    let orders = vec![order("own", "Orden de la lista"), order("title", "Título"), order("album", "Álbum"), order("duration", "Duración")];
    let button = if state.mobile { mobile::icon("page-sort", Icon::Sort, "Ordenar", palette).radius(22.0) }
        else { header_button("page-sort", Icon::Sort, TooltipSpec::new("Ordenar"), palette).active(state.sort_open).render() };
    parts.push(DropdownMenu::new(button, orders, state.sort_open, "page-sort-close", SCHEME.menu()).sizing(menu_sizing(state)).width(SORT_MENU_WIDTH).placement(PopoverPlacement::BottomEnd).render().centered_y());
    parts
}

const SORT_MENU_WIDTH: f32 = 200.0;

fn artist_page(state: &State, artist: &Artist, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::artist(state, artist, width, palette); }
    let mut about = vec![Text::new(&artist.name, palette).font_size(PAGE_TITLE).truncate().render()];
    if let Some(subscribers) = &artist.subscribers {
        about.push(Text::new(format!("{subscribers} suscriptores"), palette).muted().render());
    }
    let header = Element::row(vec![
        cover(state, &artist.artwork_url, ARTIST_PICTURE, palette).radius(ARTIST_PICTURE / 2.0),
        // The copy gives way in a narrow pane; the picture keeps its size.
        Element::column(about).gap(10.0).flex(1.0).centered_y(),
    ])
    .gap(20.0);
    let mut sections = vec![header, play_buttons(state, Some(("Seguir", "Siguiendo")), palette)];
    if !artist.songs.is_empty() {
        sections.extend([heading("Canciones", palette), track_table(state, &state.page_rows(), true, None, table_width(width), palette)]);
    }
    if !artist.albums.is_empty() {
        sections.push(shelf(state, "artist-albums", "Álbumes", release_tiles(state, "page-album", &artist.albums, palette), palette));
    }
    if !artist.singles.is_empty() {
        let tiles = release_tiles(state, "page-single", &artist.singles, palette);
        sections.push(shelf(state, "artist-singles", "Sencillos y EP", tiles, palette));
    }
    ScrollArea::new("artist", washed(state, &artist.artwork_url, Element::column(sections).gap(12.0).padding(12.0))).render().flex(1.0)
}

fn downloads_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
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
        Text::new("Descargas", palette).font_size(PAGE_TITLE).render(),
        Text::new(count, palette).muted().render(),
        play_buttons(state, None, palette),
    ])
    .gap(10.0)];
    sections.extend(failure);
    // A download that is removed closes where it was.
    sections.push(track_table(state, &state.page_rows(), true, Some("saved-row".into()), table_width(width), palette));
    ScrollArea::new("downloads", Element::column(sections).gap(16.0).padding(12.0)).render().flex(1.0)
}

/// The tracks marked with the heart, the last one first.
fn liked_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let liked = &state.collection.liked;
    if liked.is_empty() {
        return centered(Empty::new("Todavía no marcaste nada", "El corazón de la barra de abajo guarda acá lo que te gusta.", palette).icon(Icon::Heart).render());
    }
    let header = Element::column(vec![
        Text::new("Tus me gusta", palette).font_size(PAGE_TITLE).render(),
        Text::new(songs(liked.len()), palette).muted().render(),
        play_buttons(state, None, palette),
    ])
    .gap(10.0);
    // A track that loses its heart closes where it was.
    let list = track_table(state, &state.page_rows(), true, Some("liked-row".into()), table_width(width), palette);
    ScrollArea::new("liked", Element::column(vec![header, list]).gap(16.0).padding(12.0)).render().flex(1.0)
}

fn album_page(state: &State, album: &Album, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::album(state, album, width, palette); }
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
            Text::new(&album.title, palette).font_size(PAGE_TITLE).truncate().render(),
            Link::new("page-artist", &album.artist, palette).ink(palette.text).font_size(14.0).render(),
            Text::new(facts.join(" · "), palette).muted().render(),
        ])
        .gap(10.0)
        .flex(1.0)
        .centered_y(),
    ])
    .gap(20.0);
    // Every track is of this album: its table needs no column to say so.
    let tracks = track_table(state, &state.page_rows(), false, None, table_width(width), palette);
    let sections = vec![header, play_buttons(state, Some(("Guardar", "En tu biblioteca")), palette), tracks];
    ScrollArea::new("album", washed(state, &album.artwork_url, Element::column(sections).gap(16.0).padding(12.0))).render().flex(1.0)
}

/// A panel's head: what it shows, and a muted line about it.
fn panel_header(title: &str, subtitle: &str, palette: ComponentPalette) -> Element {
    let copy = Element::column(vec![Text::new(title, palette).font_size(20.0).truncate().render(), Text::new(subtitle, palette).muted().truncate().render()]).gap(2.0);
    // A panel is shut from its own header.
    let shut = header_button("toggle-panel", Icon::PanelRightClose, TooltipSpec::new("Ocultar este panel"), palette).render();
    inset(Element::row(vec![copy.flex(1.0), shut]))
}

/// A card of the panel of what plays: a step above it, with a margin of its own.
fn panel_card(parts: Vec<Element>, width: f32) -> Element {
    Element::panel(Element::column(parts).gap(8.0).padding(12.0)).background(CHOSEN).radius(RadiusRole::Panel.px()).size(Some(width), None)
}

/// How many lines of the lyrics the panel of what plays shows.
const LYRICS_SHOWN: usize = 5;

/// The lines around the one being sung, and the way to all of them.
fn lyrics_preview(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let lines = state.lines();
    let sung = state.active_line();
    // From the line before the one sung, and never past the last few.
    let from = sung.unwrap_or(0).saturating_sub(1).min(lines.len().saturating_sub(LYRICS_SHOWN));
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let mut parts = vec![Text::eyebrow("LETRA", raised).render()];
    parts.extend(lines.iter().enumerate().skip(from).take(LYRICS_SHOWN).map(|(index, line)| {
        let text = Text::new(&line.text, raised).font_size(15.0);
        if sung == Some(index) { text } else { text.muted() }.render()
    }));
    let all = Button::new("lyrics", "Mostrar la letra", raised).variant(ButtonVariant::Outline).size(ControlSize::Compact).render();
    parts.push(Element::row(vec![all, Element::spacer().flex(1.0)]));
    (!lines.is_empty()).then(|| panel_card(parts, width))
}

/// What plays after this, and the way to the whole queue.
fn next_up(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let next = state.player.upcoming.first()?;
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let queue = Link::new("next-queue", "Abrir la cola", raised).font_size(12.0).render();
    let head = Element::row(vec![Text::eyebrow("A CONTINUACIÓN", raised).render().centered_y(), Element::spacer().flex(1.0), queue.centered_y()]);
    let row = ListRow::new("upcoming:0", &next.title, raised)
        .sizing(TRACK_ROW)
        .leading(middle(cover(state, &next.artwork_url, COVER, raised)))
        .subtitle(&next.artist)
        .title_fade(true)
        .radius(RadiusRole::Card)
        .render();
    Some(panel_card(vec![head, row], width))
}

/// The height of the artist's picture in the card that presents them, and how much of
/// what their page says of them the card quotes.
const ABOUT_PICTURE: f32 = 150.0;
const ABOUT_WORDS: usize = 220;

/// Who plays what plays, as their own page presents them, with the way to follow them.
fn about_artist(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let (artist, words) = state.about_artist()?;
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let follows = state.is_followed(artist);
    let follow = Button::new("about-follow", if follows { "Siguiendo" } else { "Seguir" }, raised).variant(ButtonVariant::Outline).size(ControlSize::Compact).render();
    let name = Element::clickable("about-artist", Text::new(&artist.name, raised).font_size(16.0).truncate().render());
    let mut copy = vec![Text::eyebrow("SOBRE EL ARTISTA", raised).render(), Element::row(vec![name.flex(1.0).centered_y(), follow.centered_y()]).gap(8.0)];
    if let Some(subscribers) = &artist.subscribers {
        copy.push(Text::new(format!("{subscribers} suscriptores"), raised).muted().render());
    }
    if let Some(words) = words.as_deref().map(str::trim).filter(|words| !words.is_empty()) {
        // The opening of it, cut at a word.
        let quoted = match words.char_indices().nth(ABOUT_WORDS) {
            Some((end, _)) => format!("{}…", words[..end].rsplit_once(char::is_whitespace).map_or(&words[..end], |(kept, _)| kept)),
            None => words.to_owned(),
        };
        copy.push(Text::new(quoted, raised).role(TextRole::Caption).muted().render());
    }
    let mut parts = Vec::new();
    if let Some(image) = artist.artwork_url.as_ref().and_then(|url| state.artwork.get(url)) {
        parts.push(Element::raster_image(image.clone().cover()).size(Some(width), Some(ABOUT_PICTURE)));
    }
    parts.push(Element::column(copy).gap(6.0).padding(12.0));
    Some(Element::panel(Element::column(parts)).background(CHOSEN).radius(RadiusRole::Panel.px()).clip_children().size(Some(width), None))
}

/// A round button that stands on a picture, over a scrim of its own.
fn on_picture(id: &str, icon: Icon, hint: &str, palette: ComponentPalette) -> Element {
    let scrim = ComponentPalette { surface: COVER_SCRIM, hover: COVER_SCRIM, selected: COVER_SCRIM, ..palette };
    IconButton::new(id, icon, scrim).variant(ButtonVariant::Subtle).size(ControlSize::Compact).round().tooltip(hint).render()
}

/// Turns the videos of what plays off and on, from where they are seen.
fn footage_switch(state: &State, palette: ComponentPalette) -> Element {
    let (icon, hint) = if state.settings.clips { (Icon::Image, "Ver la tapa") } else { (Icon::Video, "Ver el video") };
    on_picture("cover-clips", icon, hint, palette)
}

/// The bar over what plays while it fills the window.
const CINEMA_TOP: f32 = 48.0;

/// The room over the playback bar, which what plays has while it fills the window.
pub fn cinema_room(window: Size) -> Size {
    Size::new(window.width, window.height - PLAYER_HEIGHT)
}

/// The size a film takes while it fills the window: all of it, as large as fits over the
/// playback bar.
pub fn cinema_picture(film: &surco_video::Film, window: Size) -> Size {
    let (width, height) = (film.width.max(1) as f32, film.height.max(1) as f32);
    let scale = (window.width / width).min((window.height - PLAYER_HEIGHT - CINEMA_TOP) / height);
    Size::new(width * scale, height * scale)
}

/// What plays over the whole window: its video where it has one that keeps time, its
/// cover where not, and under it the bar that plays it.
fn cinema(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let room = cinema_room(window);
    let leave = on_picture("cinema", Icon::Compact, "Salir de pantalla completa (Esc)", palette);
    let top = Element::row(vec![Element::spacer().flex(1.0), footage_switch(state, palette).centered_y(), leave.centered_y()]).gap(6.0).padding(GAP).size(None, Some(CINEMA_TOP));
    let over = |picture: Element| Element::column(vec![top, centered(picture)]);
    // Around a video, its own glow; around a cover, the cover's colour.
    let stage = match (state.screening(), state.current()) {
        (Some((_, footage, Showing::Large)), _) => {
            let size = cinema_picture(&footage.large.0, window);
            let film = Element::surface(CLIP_SURFACE, "Video de la canción").size(Some(size.width), Some(size.height));
            let glow = Element::surface(CLIP_BACKDROP, "").size(Some(room.width), Some(room.height));
            Element::stack(vec![glow, over(film)]).clip_children()
        }
        (_, Some(track)) => {
            let side = (room.height - CINEMA_TOP).min(room.width) * 0.8;
            let stage = over(sole_cover(state, &largest(state, track), side, palette).radius(RadiusRole::Panel.px()));
            match track.artwork_url.as_ref().and_then(|url| state.tints.get(url)) {
                Some(tint) => stage.top_background_fade(Color { a: 150, ..*tint }, room.height),
                None => stage,
            }
        }
        _ => over(Element::spacer()),
    };
    Element::panel(Element::column(vec![stage.size(Some(room.width), Some(room.height)), player(state, window.width, palette)])).background(CANVAS).size(Some(window.width), Some(window.height))
}

/// The right panel's own margin. Its lists do without it: they run to the panel's edge, so
/// their scrollbar sits where the middle panel's does, and set their rows in themselves.
const PANEL_PAD: f32 = 16.0;

fn inset(part: Element) -> Element {
    let gutter = || Element::spacer().size(Some(PANEL_PAD), None);
    Element::row(vec![gutter(), part.flex(1.0), gutter()])
}

/// The right-hand column: what plays, or what comes after it while the queue is open.
fn panel(state: &State, palette: ComponentPalette) -> Element {
    let content = if state.queue_open {
        vec![panel_header("Lo que viene", "La cola, en su orden real", palette), upcoming(state, palette)]
    } else {
        match state.current() {
            Some(track) => {
                let side = state.right_width - 32.0;
                let link = |id: &str, action: &str, text: Element| inset(Element::clickable(id, text).dispatch_as(action));
                let picture = {
                        // Not whole: a release's pictures beside it would make three.
                        let cover = cover(state, &largest(state, track), side, palette).radius(RadiusRole::Panel.px());
                        // The song's own video goes over its cover, which stays until the first frame;
                        // under the pointer, what to see there and the way to see it large.
                        let mut layers = vec![cover];
                        if state.screening().is_some() {
                            layers.push(Element::surface(CLIP_SURFACE, "Video de la canción"));
                        }
                        let controls = Element::row(vec![Element::spacer().flex(1.0), footage_switch(state, palette), on_picture("cinema", Icon::Fullscreen, "Pantalla completa", palette)]).gap(6.0).padding(8.0);
                        layers.push(Element::column(vec![controls, Element::spacer().flex(1.0)]).reveal_on_hover("cover-controls"));
                        let cover = Element::stack(layers).size(Some(side), Some(side)).radius(RadiusRole::Panel.px()).corner_mask(palette.surface).hover_region();
                        Element::row(vec![Element::spacer().size(Some(PANEL_PAD), None), cover])
                };
                let mut playing = vec![
                    picture,
                    link("panel-album", "now-album", Text::new(&track.title, palette).font_size(18.0).truncate().render()),
                    link("panel-artist", "now-artist", Text::new(&track.artist, palette).muted().truncate().render()),
                ];
                playing.extend(lyrics_preview(state, side, palette).map(inset));
                playing.extend(about_artist(state, side, palette).map(inset));
                playing.extend(next_up(state, side, palette).map(inset));
                // Under its header the panel scrolls: on a short window the artist is below the fold.
                playing.push(Element::spacer().size(None, Some(PANEL_PAD - 12.0)));
                let below = Element::column(playing).gap(12.0);
                vec![panel_header("Sonando", track.album.as_deref().unwrap_or(&track.artist), palette), ScrollArea::new("playing", below).render().flex(1.0)]
            }
            None => vec![
                panel_header("Sonando", "Nada por ahora", palette),
                centered(Empty::new("Nada sonando", "Elegí una canción y acá aparece.", palette).icon(Icon::MusicNote).render()),
            ],
        }
    };
    let face = if state.queue_open { "panel:queue" } else { "panel:playing" };
    // The column's gap, plus this, is the margin over the header.
    let top = Element::spacer().size(None, Some(PANEL_PAD - 12.0));
    Element::column(std::iter::once(top).chain(content).collect()).gap(12.0).animate_enter(face)
}

/// How bright and how out of focus a line is `distance` lines after the one being sung,
/// negative before it. The line being sung is sharp and in full ink; what is coming fades
/// and blurs the further off it is, and what was sung steps back a little faster.
pub(crate) fn lyric_focus(distance: i64) -> (f32, f32) {
    match distance {
        0 => (1.0, 0.0),
        ahead @ 1.. => ((0.55 - 0.07 * (ahead - 1) as f32).max(0.18), (0.3 + 0.5 * (ahead - 1) as f32).min(LYRIC_BLUR)),
        behind => ((0.42 - 0.08 * (-behind - 1) as f32).max(0.16), (0.6 + 0.5 * (-behind - 1) as f32).min(LYRIC_BLUR)),
    }
}

/// Where the line being sung rests on a phone, below the top of the lyrics' visible `body`:
/// a third of the way down, with what was sung above it and what is coming below.
pub(crate) fn lyric_anchor(body: f32) -> f32 {
    (body / 3.0).max(20.0)
}

/// The lines of what plays down the middle of the window, the one being sung in full ink,
/// over a wash of the cover's own colour. A line that is clicked is sung from its start.
/// Given the height of the `body` they are seen through, as on a phone, the first and the
/// last line can both reach the anchor the list follows the song at.
fn lyrics(state: &State, width: f32, body: f32, palette: ComponentPalette) -> Element {
    let note = |title: &str, about: &str| centered(Empty::new(title, about, palette).icon(Icon::Custom(LYRICS)).render());
    let lines = match &state.lyrics {
        _ if state.current().is_none() => return note("Nada sonando", "Elegí una canción y acá aparece su letra."),
        Some((_, Lyrics::Loading)) | None => return loading("lyrics-loading", palette),
        Some((_, Lyrics::Missing)) => return centered(Element::column(vec![
            Empty::new("Letra no disponible", "El servicio todavía no tiene la letra de esta canción.", palette).icon(Icon::Custom(LYRICS)).render(),
            Button::new("retry-lyrics", "Volver a buscar", palette).variant(ButtonVariant::Outline).size(ControlSize::Spacious).round().render(),
        ]).gap(20.0).padding(20.0)),
        Some((_, Lyrics::Failed { message, .. })) => return centered(Element::column(vec![
            Empty::new("No se pudo cargar la letra", message, palette).icon(Icon::Custom(LYRICS)).render(),
            Button::new("retry-lyrics", "Reintentar", palette).variant(ButtonVariant::Outline).size(ControlSize::Spacious).round().render(),
        ]).gap(20.0).padding(20.0)),
        Some((_, Lyrics::Plain { source, .. })) => {
            let words = std::iter::once(Text::new(format!("Letra sin sincronización · {source}"), palette).muted().render())
                .chain(state.plain_lyrics().iter().map(|line| {
                    if line.is_empty() { Element::spacer().size(None, Some(20.0)) }
                    else { Text::new(line, palette).font_size(LYRIC_SIZE).render() }
                })).collect();
            return ScrollArea::new("lyrics-list", Element::column(words).gap(LYRIC_GAP).padding(PANEL_PAD)).render().flex(1.0);
        }
        Some((_, Lyrics::Lines(_))) => state.lines(),
    };
    // With room, the lines stand larger and further in from the edge.
    let (size, margin) = if width >= LYRIC_WIDE_FROM { (LYRIC_SIZE_WIDE, (width * 0.08).min(96.0)) }
        else if state.mobile { (LYRIC_SIZE_PHONE, PANEL_PAD + 8.0) } else { (LYRIC_SIZE, PANEL_PAD) };
    let (gap, line_height) = (LYRIC_GAP * size / LYRIC_SIZE, size * 1.2);
    // Before the first line everything is still to come.
    let sung = state.active_line().map_or(-1, |index| index as i64);
    let now = std::time::Instant::now();
    let rows = lines.iter().enumerate().map(|(index, line)| {
        let (ink, blur) = lyric_focus(index as i64 - sung);
        let ink = Color { a: (255.0 * ink) as u8, ..palette.text };
        let text = Element::text(&line.text)
            .font_family(FontFamily::SansBold)
            .font_size(size)
            .line_height(line_height)
            .foreground(ink)
            .animate_foreground(format!("lyric-ink:{index}"))
            .soft_focus(format!("lyric:{index}"), blur)
            // The line being sung grows a little from its leading edge; paint only, no re-wrap.
            .animate_scale(format!("lyric-scale:{index}"), if index as i64 == sung { 1.05 } else { 1.0 });
        let row = Element::clickable(format!("lyric:{index}"), text);
        // Still catching up with the list's jump to the line being sung.
        let lag = state.lyric_lag(index, now);
        if lag == 0.0 { row } else { row.translate(limn::Point::new(0.0, lag)) }
    });
    let mut rows: Vec<Element> = rows.collect();
    if body > 0.0 {
        let anchor = lyric_anchor(body);
        rows.insert(0, Element::spacer().size(None, Some((anchor - margin - gap).max(0.0))));
        rows.push(Element::spacer().size(None, Some((body - anchor - margin - gap - line_height).max(0.0))));
    }
    // The list glides to each new line rather than stepping to it.
    let glide = limn::ScrollMotion { duration_ms: limn::tokens().motion.move_ms * 3, easing: limn::Easing::InOutCubic };
    let list = ScrollArea::new("lyrics-list", Element::column(rows).gap(gap).padding(margin)).motion(glide).render().flex(1.0);
    let page = Element::column(vec![list]).flex(1.0);
    match state.current().and_then(|track| state.tints.get(track.artwork_url.as_deref()?)) {
        Some(tint) => page.bottom_background_fade(Color { a: 120, ..*tint }, LYRIC_WASH),
        None => page,
    }
}

fn upcoming(state: &State, palette: ComponentPalette) -> Element {
    let rows: Vec<Element> = state
        .player
        .upcoming
        .iter()
        .enumerate()
        .map(|(index, track)| {
            if state.mobile {
                let subtitle = if state.player.picks.contains(&track.id) { format!("{} · Recomendada", track.artist) } else { track.artist.clone() };
                return mobile::track_row(state, format!("upcoming:{index}"), track, &subtitle, palette);
            }
            let row = ListRow::new(format!("upcoming:{index}"), &track.title, palette)
                .sizing(TRACK_ROW)
                .leading(middle(cover(state, &track.artwork_url, COVER, palette)))
                // A track the shuffle brought in from the catalog says so.
                .subtitle(if state.player.picks.contains(&track.id) { format!("{} · Recomendada", track.artist) } else { track.artist.clone() })
                .title_fade(true)
                .radius(RadiusRole::Card)
                .render();
            // A finger pans the queue; desktop pointers can reorder its rows.
            let row = if state.mobile { row } else { row.draggable().drop_target() };
            let out = ContextMenuItem::new(format!("do:unqueue:upcoming:{index}"), "Quitar de la cola").icon(Icon::Custom(include_str!("../assets/icons/circle-minus.svg")));
            let row = ContextMenu::new(format!("upcoming-context:{index}"), row, vec![ContextMenuEntry::Item(out)], SCHEME.menu()).render();
            // Named by its track, so the row that starts playing closes and the rest glide up.
            Element::column(vec![row]).animate_height(format!("upcoming-row:{}", track.id), false)
        })
        .collect();
    // What was queued by hand is told apart from what the list brings, and can be dropped at once.
    let mut rows = rows;
    let queued = state.player.queued.min(rows.len());
    if queued > 0 {
        let title = |text: &str| Text::new(text, palette).muted().render().centered_y();
        let clear = Button::new("queue-clear", "Borrar", palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render();
        if queued < rows.len() {
            rows.insert(queued, Element::panel(title("A continuación")).padding(PANEL_PAD / 2.0));
        }
        rows.insert(0, Element::panel(Element::row(vec![title("En la cola"), Element::spacer().flex(1.0), clear])).padding(PANEL_PAD / 2.0));
    }
    if rows.is_empty() {
        return centered(Empty::new("Nada en la cola", "Elegí una canción y acá aparece lo que sigue.", palette).icon(Icon::Playlist).render());
    }
    // The rows carry half the margin in themselves.
    let list = Element::column(rows).keep_leaving("upcoming-rows");
    ScrollArea::new("queue-list", Element::column(vec![list]).padding(PANEL_PAD / 2.0)).render().flex(1.0)
}

fn cover(state: &State, artwork: &Option<String>, side: f32, palette: ComponentPalette) -> Element {
    picture(state, artwork, side, false, palette)
}

/// A cover shown large and alone, decoded whole: limn decodes the rest at 384 pixels a side,
/// which three pixels a point blur past 128 points. limn keeps two such at once, so one a page.
fn sole_cover(state: &State, artwork: &Option<String>, side: f32, palette: ComponentPalette) -> Element {
    picture(state, artwork, side, true, palette)
}

/// The playing track's large rendition once it has arrived; until then, the row's.
fn largest(state: &State, track: &Track) -> Option<String> {
    let large = track.artwork_url.as_deref().map(large_cover).filter(|url| state.artwork.contains_key(url));
    large.or_else(|| track.artwork_url.clone())
}

fn picture(state: &State, artwork: &Option<String>, side: f32, whole: bool, palette: ComponentPalette) -> Element {
    let radius = limn::tokens().radius.control;
    match artwork.as_ref().and_then(|url| state.artwork.get(url)) {
        // Covers are square; a video's thumbnail and an artist's banner are not.
        Some(image) if whole => Element::raster_image(image.clone().cover().full_resolution()).size(Some(side), Some(side)).radius(radius),
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
            {
                let liked = state.is_liked(&track.id);
                IconButton::new("like", if liked { Icon::HeartFill } else { Icon::Heart }, palette)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Compact)
                    .tooltip(if liked { "Quitar de Tus me gusta" } else { "Me gusta" })
                    .render()
                    .centered_y()
            },
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
        transport("shuffle", shuffle_step(state).0, shuffle_step(state).1).active(state.player.shuffle).render(),
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
        equalizer::popover(state, SCHEME.menu()),
        state.equalizer_open,
        "equalizer-close",
        SCHEME.menu(),
    )
    .placement(PopoverPlacement::TopEnd)
    .width(EQUALIZER_WIDTH)
    .render()
    .centered_y();
    let mut controls = Vec::new();
    if wide {
        controls.push(Element::spacer().flex(1.0));
    }
    controls.extend([
        side("lyrics", Icon::Custom(LYRICS), "Letra", state.lyrics_open()),
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
    use surco_core::{ArtistRef, Found};
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
    fn a_page_keeps_a_search_at_the_end_of_its_actions_that_opens_into_a_field_and_narrows_its_rows() {
        let size = Size::new(1400.0, 900.0);
        let album = Album { id: "lp".into(), title: "Album".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("one"), track("two")] };
        // Without motion the field is open on the frame of the click.
        let mut state = State::default();
        state.page = Page::Album(album);
        state.motion = false;
        let has = |state: &State, id: &str| ids(state, size).iter().any(|found| found == id);
        assert!(has(&state, "page-search") && !has(&state, "page-query"), "a magnifier, shut");
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        assert!(has(&state, "page-query") && has(&state, "page-search"), "the field, and the cross that shuts it");
        assert!(has(&state, "page-track:0") && has(&state, "page-track:1"));
        state.update(&limn::UiAction::InputChanged { id: "page-query".into(), value: "title TWO".into() });
        assert!(!has(&state, "page-track:0") && has(&state, "page-track:1"), "the rows keep their own numbers");
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        assert!(!has(&state, "page-query") && has(&state, "page-track:0"), "shut, the page lists everything again");
    }

    #[test]
    fn a_pages_search_fits_whole_in_a_narrow_pane_beside_its_other_actions() {
        let album = Album { id: "lp".into(), title: "Album".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("one")] };
        let mut state = State::default();
        state.page = Page::Album(album);
        state.motion = false;
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        for width in [1100.0, 1900.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            let frame = tree.layout(size);
            let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
            let (keep, field, shut, pane) = (rect("keep"), rect("page-query"), rect("page-search"), rect("album"));
            assert!(field.width >= 120.0, "room to type at {width}: {field:?}");
            assert!(keep.x + keep.width <= field.x && field.x + field.width <= shut.x, "side by side at {width}: {keep:?} {field:?} {shut:?}");
            assert!(shut.x + shut.width <= pane.x + pane.width - 12.0, "inside the pane at {width}: {shut:?}");
        }
    }

    #[test]
    fn a_pages_table_has_heads_that_order_it_and_an_album_boundary_to_drag() {
        let song = |id: &str| Track { album: Some("Disco".into()), ..track(id) };
        let artist = Artist { id: "who".into(), name: "Who".into(), subscribers: None, artwork_url: None, songs: vec![song("a"), song("b")], albums: Vec::new(), singles: Vec::new() };
        let mut state = State::default();
        state.page = Page::Artist(artist);
        let found = ids(&state, Size::new(1600.0, 800.0));
        for id in ["tracks:sort:title", "tracks:sort:album", "tracks:sort:duration", "tracks:resize:album", "page-track:0", "more:page-track:1", "page-sort"] {
            assert!(found.iter().any(|found| found == id), "{id} among {found:?}");
        }
        // In a narrow pane the album gives its place to the title and there is nothing to drag.
        let found = ids(&state, Size::new(1100.0, 800.0));
        assert!(found.iter().any(|id| id == "tracks:sort:title") && !found.iter().any(|id| id.starts_with("tracks:") && id.contains("album")), "{found:?}");
    }

    #[test]
    fn the_library_lists_as_lines_rows_or_tiles_and_the_panel_previews_lyrics_and_what_is_next() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        for view in ["compact", "list", "grid"] {
            state.update(&limn::UiAction::Clicked { id: format!("library-view:{view}") });
            assert!(ids(&state, size).iter().any(|id| id == "downloads"), "{view} still lists what the library holds");
        }
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, playing: true, ..PlayerView::default() });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(vec![surco_core::lyrics::Line { at_ms: 0, text: "one".into() }]) });
        let found = ids(&state, size);
        // The card of the lines leads to all of them, and the one of what is next plays it or opens the queue.
        for id in ["lyrics", "upcoming:0", "next-queue"] {
            assert!(found.iter().filter(|found| *found == id).count() >= 1, "{id} among {found:?}");
        }
    }

    #[test]
    fn the_buttons_of_the_panel_of_what_plays_do_what_they_say_when_pressed() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, playing: true, ..PlayerView::default() });
        // A press as the window gives it: down and up in the middle of the control.
        let press = |state: &mut State, id: &str| {
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let rect = frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap_or_else(|| panic!("no {id}"));
            let middle = limn::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
            tree.handle_event(limn::UiEvent::PointerMove(middle));
            tree.layout(size);
            tree.handle_event(limn::UiEvent::PointerDown(middle));
            for action in tree.handle_event(limn::UiEvent::PointerUp(middle)) {
                state.update(&action);
            }
        };
        assert!(state.settings.clips);
        press(&mut state, "cover-clips");
        assert!(!state.settings.clips, "the cover is shown in the video's place");
        press(&mut state, "cover-clips");
        assert!(state.settings.clips);
        press(&mut state, "cinema");
        assert!(state.cinema, "what plays takes the whole window");
        press(&mut state, "cinema");
        assert!(!state.cinema, "and gives it back from the same button");
        press(&mut state, "next-queue");
        assert!(state.queue_open, "the link under what comes next opens the queue");
    }

    #[test]
    fn the_card_of_a_later_version_shows_a_bar_while_it_comes_down_and_a_button_otherwise() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        let release = Release { version: "9.0.0".into(), page: "https://example/9".into(), program: Some(("https://example/p".into(), "https://example/s".into())), mandatory: false, notes: Vec::new() };
        state.apply(Event::Update(release));
        let shown = |state: &State| {
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let texts: Vec<String> = frame.commands.iter().filter_map(|command| match command {
                limn::DrawCommand::Text { text, .. } => Some(text.clone()),
                _ => None,
            }).collect();
            (texts, frame.regions.iter().map(|region| region.id.clone()).collect::<Vec<_>>())
        };
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Surco 9.0.0 está disponible") && ids.iter().any(|id| id == "update-toast"), "found, it is said over the window: {texts:?}");
        state.update(&limn::UiAction::Clicked { id: "update-toast".into() });
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Actualización disponible") && ids.iter().any(|id| id == "update-get") && !ids.iter().any(|id| id == "update-toast"));
        state.apply(Event::UpdateProgress(0.42));
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Descargando Surco 9.0.0") && texts.iter().any(|text| text == "42 %"), "{texts:?}");
        assert!(!ids.iter().any(|id| id == "update-get" || id == "update-restart"), "nothing to press while it comes down");
        state.apply(Event::UpdateInstalled);
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Surco 9.0.0 está lista") && ids.iter().any(|id| id == "update-restart"));
    }

    #[test]
    fn a_shut_panel_keeps_the_button_that_opens_it_inside_the_window() {
        let mut state = State::default();
        state.update(&limn::UiAction::Clicked { id: "toggle-panel".into() });
        for width in [1100.0, 1400.0, 2200.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            let frame = tree.layout(size);
            let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap_or_else(|| panic!("no {id} at {width}"));
            let (bell, settings, open) = (rect("news"), rect("settings"), rect("toggle-panel-open"));
            assert!(bell.x + bell.width <= settings.x && settings.x + settings.width <= open.x && open.x + open.width <= width, "at {width}: {bell:?} {settings:?} {open:?}");
        }
    }

    #[test]
    fn the_lyrics_take_the_page_and_leave_the_panel_to_what_plays() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), duration: 200.0, playing: true, ..PlayerView::default() });
        state.update(&limn::UiAction::Clicked { id: "lyrics".into() });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(vec![surco_core::lyrics::Line { at_ms: 0, text: "one".into() }]) });
        let size = Size::new(1400.0, 800.0);
        let mut tree = UiTree::new(view(&state, size));
        let frame = tree.layout(size);
        let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
        assert!(rect("lyric:0").x < rect("panel-album").x, "the line in the middle, what plays still at the side");
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
        assert!(!has(&at(&state, 1400.0), "queue-clear"), "nothing was queued by hand");
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b"), track("c")], queued: 1, duration: 200.0, ..PlayerView::default() });
        assert!(has(&at(&state, 1400.0), "queue-clear") && has(&at(&state, 1400.0), "upcoming:1"));
        // The library's search, once its magnifier opens it, and its order share a row inside the panel.
        assert!(has(&at(&state, 1400.0), "library-search") && !has(&at(&state, 1400.0), "library-query"));
        state.update(&limn::UiAction::Clicked { id: "library-search".into() });
        let mut tree = UiTree::new(view(&state, Size::new(1400.0, 700.0)));
        let frame = tree.layout(Size::new(1400.0, 700.0));
        let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
        let (query, sort, list) = (rect("library-query"), rect("library-sort"), rect("downloads"));
        // A row of the queue taken by the pointer and let go over another is dropped on it.
        let middle = |id: &str| limn::Point::new(rect(id).x + rect(id).width / 2.0, rect(id).y + rect(id).height / 2.0);
        tree.handle_event(limn::UiEvent::PointerDown(middle("upcoming:1")));
        tree.handle_event(limn::UiEvent::PointerMove(middle("upcoming:0")));
        let dropped = tree.handle_event(limn::UiEvent::PointerUp(middle("upcoming:0")));
        assert!(matches!(dropped.as_slice(), [limn::UiAction::Dropped { id, target: Some(target), .. }] if id == "upcoming:1" && target == "upcoming:0"), "{dropped:?}");
        assert!(query.width > 80.0 && query.x + query.width <= sort.x && sort.x + sort.width <= state.left_width + 16.0 && query.y + query.height <= list.y, "{query:?} {sort:?} {list:?}");
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
        state.apply(Event::Outputs(Vec::new()));
        has(&state, &["setting:autoplay", "setting:output", "setting:outputs-refresh"]);
        assert!(ids(&state, Size::new(720.0, 1000.0)).iter().any(|id| id == "setting:autoplay"), "and in a narrow window");
    }

    #[test]
    fn settings_keep_the_output_picker_and_library_choices_inside_a_narrow_page() {
        let state = State::default();
        for width in [360.0, 900.0] {
            let size = Size::new(width, 4000.0);
            let mut tree = UiTree::new(settings_page(&state, width, SCHEME.palette()));
            let frame = tree.layout(size);
            for id in ["setting:output", "setting:outputs-refresh", "library-sort:alphabetical", "library-view:grid", "setting:library-panel", "setting:player-panel", "setting:motion"] {
                let node = frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("{id} at {width}"));
                assert!(node.rect.x >= 0.0 && node.rect.x + node.rect.width <= width, "{id}: {:?}", node.rect);
                assert!(node.actionable, "{id} can be used");
            }
            let autoplay = frame.access.iter().find(|node| node.id == "setting:autoplay").unwrap();
            assert_eq!(autoplay.name, "Reproducción automática");
            assert!(autoplay.rect.height >= 24.0, "the switch is comfortable to target");
        }
    }

    #[test]
    fn connections_explain_loading_empty_and_error_states_without_hiding_the_system_output() {
        let size = Size::new(900.0, 4000.0);
        let shown = |state: &State| {
            let mut tree = UiTree::new(settings_page(state, size.width, SCHEME.palette()));
            tree.layout(size)
        };
        let has_text = |frame: &limn::Frame, text: &str| frame.text_layouts().iter().any(|layout| layout.source.contains(text));
        let mut state = State::default();
        let frame = shown(&state);
        assert!(has_text(&frame, "No se encontraron salidas"));
        assert!(has_text(&frame, "Bluetooth"));
        assert!(frame.access.iter().any(|node| node.id == "setting:output" && node.actionable));

        state.outputs_loading = true;
        let frame = shown(&state);
        assert!(has_text(&frame, "Buscando salidas"));
        assert!(frame.access.iter().any(|node| node.id == "setting:outputs-refresh" && node.disabled));

        state.outputs_loading = false;
        state.outputs_error = Some("El servidor de audio no responde.".into());
        state.output_notice = Some("Volvimos a la salida predeterminada.".into());
        let frame = shown(&state);
        assert!(has_text(&frame, "El servidor de audio no responde."));
        assert!(has_text(&frame, "Volvimos a la salida predeterminada."));
        assert!(frame.access.iter().any(|node| node.id == "setting:outputs-refresh" && node.actionable));

        state.outputs = vec![surco_audio::AudioOutputDevice { id: "speakers".into(), name: "Parlantes del escritorio".into(), is_default: true }];
        let frame = shown(&state);
        let selected = frame.access.iter().find(|node| node.id == "setting:output").unwrap();
        assert_eq!(selected.value_text.as_deref(), Some("Sistema: Parlantes del escritorio"));
        state.output = Some("disconnected".into());
        let frame = shown(&state);
        let selected = frame.access.iter().find(|node| node.id == "setting:output").unwrap();
        assert_eq!(selected.value_text.as_deref(), Some("Salida guardada no disponible"));
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
