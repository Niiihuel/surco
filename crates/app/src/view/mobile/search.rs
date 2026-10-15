//! A search tab with deliberate keyboard ownership and real catalog results.

use super::*;
use crate::state::mobile::SearchFilter;

pub(super) fn field(state: &State, palette: ComponentPalette) -> Element {
    let editing = state.mobile_ui.search_editing;
    let field = if editing {
        SearchField::new("search", &state.query, "¿Qué querés escuchar?", SCHEME.on_raised())
            .size(ControlSize::Spacious).render().size(None, Some(TAP)).flex(1.0)
    } else {
        Button::new("mobile-search-edit", if state.query.is_empty() { "¿Qué querés escuchar?" } else { &state.query }, palette)
            .icon(Icon::Search).variant(ButtonVariant::Default).size(ControlSize::Spacious).render().size(None, Some(48.0)).flex(1.0)
    };
    let mut row = vec![field];
    if editing {
        // The explicit control is useful for pasted links as well as one-letter queries.
        if !state.query.is_empty() { row.push(icon("mobile-search-clear", Icon::Close, "Borrar búsqueda", palette)); }
        row.push(Button::new("mobile-search-cancel", "Cancelar", palette).variant(ButtonVariant::Ghost).size(ControlSize::Spacious).render().size(None, Some(TAP)));
    }
    let row = Element::row(row).gap(4.0);
    if cfg!(target_os = "ios") {
        Element::column(vec![row,
            Button::new("ios-paste:search", "Pegar", palette)
                .variant(ButtonVariant::Secondary).size(ControlSize::Spacious).render().size(None, Some(TAP)),
        ]).gap(8.0).padding(12.0)
    } else { row.padding(12.0) }
}

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    if state.searching {
        return loading("mobile-search-loading", palette);
    }
    if let Some(error) = &state.search_error {
        return centered(Empty::new("No pudimos buscar", error, palette).icon(Icon::WarningTriangle)
            .action("search-submit", "Volver a intentar", ButtonVariant::Secondary).render().padding(20.0));
    }
    if state.submitted.is_none() {
        return idle(state, width, palette);
    }
    let chosen = state.mobile_ui.search_filter;
    let tab = |key: &str, label: &str, value| Button::new(format!("search-filter:{key}"), label, palette)
        .variant(if chosen == value { ButtonVariant::Default } else { ButtonVariant::Secondary })
        .size(ControlSize::Spacious).render().size(None, Some(TAP));
    let filters = Element::horizontal_scroll_2d("mobile-search-filters", Element::row(vec![
        tab("all", "Todo", SearchFilter::All), tab("tracks", "Canciones", SearchFilter::Tracks),
        tab("artists", "Artistas", SearchFilter::Artists), tab("albums", "Álbumes", SearchFilter::Albums),
    ]).gap(8.0)).drag_to_pan();
    let found = &state.found;
    let mut rows = Vec::new();
    if matches!(chosen, SearchFilter::All | SearchFilter::Tracks) && !found.tracks.is_empty() {
        rows.push(heading("Canciones", palette).padding(8.0));
        rows.extend(found.tracks.iter().enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, &track.artist, palette)));
    }
    if matches!(chosen, SearchFilter::All | SearchFilter::Artists) && !found.artists.is_empty() {
        rows.push(heading("Artistas", palette).padding(8.0));
        rows.extend(found.artists.iter().enumerate().map(|(index, artist)| result(state, format!("artist:{index}"), &artist.name, "Artista", &artist.artwork_url, true, palette)));
    }
    if matches!(chosen, SearchFilter::All | SearchFilter::Albums) && !found.albums.is_empty() {
        rows.push(heading("Álbumes", palette).padding(8.0));
        rows.extend(found.albums.iter().enumerate().map(|(index, album)| result(state, format!("album:{index}"), &album.title, album.artist.as_deref().unwrap_or("Álbum"), &album.artwork_url, false, palette)));
    }
    let list = if rows.is_empty() {
        centered(Empty::new("Sin resultados", "Probá con otro nombre o cambiá el filtro.", palette).icon(Icon::Search).render())
    } else {
        ScrollArea::new("mobile-search-results", Element::column(rows).gap(4.0)).render().flex(1.0)
    };
    Element::column(vec![filters, list]).gap(12.0).padding(8.0).flex(1.0)
}

fn result(state: &State, id: String, title: &str, subtitle: &str, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    let picture = cover(state, artwork, 48.0, palette);
    ListRow::new(id, title, palette).sizing(ListRowSizing { height: Some(72.0), two_line_height: Some(72.0), leading_width: 48.0, ..TRACK_ROW })
        .leading(if round { picture.radius(24.0) } else { picture.radius(4.0) })
        .subtitle(subtitle).title_fade(true).render()
}

fn idle(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut body = Vec::new();
    if !state.collection.searches.is_empty() {
        body.push(Element::row(vec![heading("Búsquedas recientes", palette).flex(1.0),
            icon("recent-search-clear", Icon::Trash, "Borrar búsquedas recientes", palette)]).gap(8.0));
        body.extend(state.collection.searches.iter().enumerate().map(|(index, query)| {
            ListRow::new(format!("recent-search:{index}"), query, palette).sizing(ListRowSizing { height: Some(56.0), ..TRACK_ROW })
                .leading(Element::icon(Icon::Clock).foreground(palette.muted).size(Some(32.0), Some(32.0)))
                .title_fade(true).render()
        }));
    }
    if state.mobile_ui.search_editing {
        body.push(Text::new(if state.query.contains("://") { "Pegaste un enlace. Tocá Buscar en el teclado para abrirlo." } else { "Buscá canciones, artistas y álbumes. También podés pegar un enlace." }, palette).muted().render().padding(8.0));
    } else {
        body.push(heading("Explorá tu próximo sonido", palette));
        body.push(browse(width - 24.0, palette));
    }
    ScrollArea::new("mobile-search-idle", Element::column(body).gap(16.0).padding(12.0)).render().flex(1.0)
}

pub(super) fn browse(width: f32, palette: ComponentPalette) -> Element {
    let tile = |title: &str, query: &str, tint| {
        let copy = Element::column(vec![Text::new(title, palette).font_size(21.0).render(), Element::spacer().flex(1.0),
            Element::icon(Icon::ArrowRight).foreground(palette.text).centered_x()]).gap(8.0).padding(16.0);
        Element::clickable(format!("browse:{query}"), Element::panel(copy).background(tint).radius(10.0))
            .accessible(limn::Access::new(limn::Role::Button).name(format!("Buscar {query}")))
            .size(Some((width - 12.0) / 2.0), Some(116.0))
    };
    Element::column(vec![
        Element::row(vec![tile("Rock", "Rock argentino", Color::rgb(104, 48, 63)), tile("Indie", "Indie latino", Color::rgb(49, 73, 108))]).gap(12.0),
        Element::row(vec![tile("Para bajar\nun cambio", "Acústico relajado", Color::rgb(56, 89, 77)), tile("Subí el\nvolumen", "Electrónica", Color::rgb(95, 65, 111))]).gap(12.0),
    ]).gap(12.0)
}
