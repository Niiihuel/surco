//! A search tab with deliberate keyboard ownership and real catalog results.
//! Figma iOS kit: 1BzqhExmVONm3WMDZBMqPj / 604:9087, 604:9150, 604:9171.

use super::*;
use crate::state::mobile::SearchFilter;

pub(super) fn field_height(state: &State) -> f32 {
    if state.mobile_search_compact() { 56.0 } else { 68.0 }
}

pub(super) fn field(state: &State, palette: ComponentPalette) -> Element {
    let editing = state.mobile_ui.search_editing;
    let compact = state.mobile_search_compact();
    let padding = if compact { 6.0 } else { 12.0 };
    if cfg!(any(target_os = "ios", test)) && state.mobile_ui.native_surfaces {
        return Element::panel(Element::spacer().size(None, Some(TAP))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-search").name("Buscar música")))
            .padding(padding).background(if compact { Color::rgb(33, 33, 33) } else { SCHEME.canvas })
            .size(None, Some(field_height(state)));
    }
    let field = if editing {
        SearchField::new("search", &state.query, "¿Qué querés escuchar?", SCHEME.on_raised())
            .size(ControlSize::Spacious).render().radius(TAP / 2.0).size(None, Some(TAP)).flex(1.0)
    } else {
        Button::new("mobile-search-edit", if state.query.is_empty() { "¿Qué querés escuchar?" } else { &state.query }, palette)
            .icon(Icon::Search).variant(ButtonVariant::Default)
            .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render().flex(1.0)
    };
    let mut row = vec![field];
    if editing {
        // The explicit control is useful for pasted links as well as one-letter queries.
        if !state.query.is_empty() { row.push(icon("mobile-search-clear", Icon::Close, "Borrar búsqueda", palette)); }
        row.push(Button::new("mobile-search-cancel", "Cancelar", palette).variant(ButtonVariant::Ghost).size(ControlSize::Spacious).render().size(None, Some(TAP)));
    }
    Element::row(row).gap(4.0).padding(padding).size(None, Some(field_height(state)))
}

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let pending = state.searching || state.mobile_ui.pending.is_some();
    let has_results = !state.found.tracks.is_empty() || !state.found.artists.is_empty() || !state.found.albums.is_empty();
    if state.submitted.is_none() && !pending && !has_results && state.search_error.is_none() {
        return idle(state, width, palette);
    }
    let chosen = state.mobile_ui.search_filter;
    let tab = |key: &str, label: &str, value| Button::new(format!("search-filter:{key}"), label, palette)
        .variant(if chosen == value { ButtonVariant::Default } else { ButtonVariant::Secondary })
        .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render();
    let filters = Element::horizontal_scroll_2d("mobile-search-filters", Element::row(vec![
        tab("all", "Todo", SearchFilter::All), tab("tracks", "Canciones", SearchFilter::Tracks),
        tab("artists", "Artistas", SearchFilter::Artists), tab("albums", "Álbumes", SearchFilter::Albums),
    ]).gap(8.0)).drag_to_pan().scrollbar_visibility(SCROLL_INDICATOR);
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
    if rows.is_empty() {
        rows.push(if pending || state.search_error.is_some() { Element::spacer().size(None, Some(TAP)) }
            else { Empty::new("Sin resultados", "Probá con otro nombre o cambiá el filtro.", palette).icon(Icon::Search).render().padding(20.0) });
    }
    // This retained scroll and its parent stay mounted throughout debounce,
    // loading and failures; the previous result rows remain usable meanwhile.
    let list = ScrollArea::new("mobile-search-results", Element::column(rows).gap(4.0)).render().flex(1.0);
    // Keep recovery outside the retained scroll: a failure while browsing deep
    // results must offer Retry without asking the user to find the list's top.
    let status = if let Some(error) = &state.search_error {
        Element::row(vec![Element::column(vec![
            Text::new("No pudimos actualizar", palette).font_size(13.0).render(),
            Text::new(error, palette).font_size(12.0).muted().render(),
        ]).gap(4.0).flex(1.0),
            Button::new("search-submit", "Reintentar", palette).variant(ButtonVariant::Secondary)
                .size(ControlSize::Spacious).render().size(None, Some(TAP)),
        ]).gap(8.0).padding(8.0)
    } else {
        let copy = if pending { Text::new("Buscando…", palette).font_size(12.0).muted().render() }
            else if state.submitted.is_none() && has_results {
                Text::new(if state.query.contains("://") { "Tocá Buscar para abrir el enlace." } else { "Resultados anteriores" }, palette)
                    .font_size(12.0).muted().render()
            } else { Element::spacer() };
        copy.size(None, Some(20.0))
    };
    Element::column(vec![filters, status, list]).gap(8.0).padding(8.0).flex(1.0)
}

fn result(state: &State, id: String, title: &str, subtitle: &str, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    let picture = cover(state, artwork, 48.0, palette);
    ListRow::new(id, title, palette).sizing(ListRowSizing { height: Some(68.0), two_line_height: Some(68.0), leading_width: 48.0, ..TRACK_ROW })
        .leading(if round { picture.radius(24.0) } else { picture.radius(4.0) })
        .subtitle(subtitle).title_fade(true).render()
}

fn idle(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut body = Vec::new();
    if !state.collection.searches.is_empty() {
        body.push(Element::row(vec![Text::new("Búsquedas recientes", palette).role(TextRole::Heading).font_size(18.0).render().flex(1.0),
            icon("recent-search-clear", Icon::Trash, "Borrar búsquedas recientes", palette)]).gap(8.0));
        body.extend(state.collection.searches.iter().enumerate().map(|(index, query)| {
            ListRow::new(format!("recent-search:{index}"), query, palette).sizing(ListRowSizing { height: Some(68.0), ..TRACK_ROW })
                .leading(Element::icon(Icon::Clock).foreground(palette.muted).size(Some(32.0), Some(32.0)))
                .title_fade(true).render()
        }));
    }
    if state.mobile_ui.search_editing {
        body.push(Text::new(if state.query.contains("://") { "Pegaste un enlace. Tocá Buscar en el teclado para abrirlo." } else { "Buscá canciones, artistas y álbumes. También podés pegar un enlace." }, palette).muted().render().padding(8.0));
    } else {
        body.push(Text::new("Explorá tu próximo sonido", palette).role(TextRole::Heading).font_size(17.0).render());
        body.push(browse(width - 24.0, palette));
    }
    ScrollArea::new("mobile-search-idle", Element::column(body).gap(16.0).padding(12.0)).render().flex(1.0)
}

pub(super) fn browse(width: f32, palette: ComponentPalette) -> Element {
    let tile = |title: &str, query: &str, tint| {
        let copy = Element::column(vec![Text::new(title, palette).role(TextRole::Heading).font_size(17.0).render(), Element::spacer().flex(1.0),
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

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{DrawCommand, Frame, HitKind, Point, UiAction, UiEvent, UiTree};
    use std::time::Instant;

    fn phone() -> State {
        let mut state = State::default();
        state.mobile = true;
        state.mobile_search = true;
        state.page = Page::Search;
        state
    }

    fn results(state: &State) -> Element { content(state, 390.0, SCHEME.palette()) }

    fn text(frame: &Frame, expected: &str) -> bool {
        frame.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text == expected))
    }

    #[test]
    fn native_search_has_one_touch_sized_slot_and_a_working_limn_fallback() {
        let mut state = phone();
        let size = Size::new(390.0, 100.0);
        for editing in [false, true] {
            state.mobile_ui.search_editing = editing;
            state.mobile_ui.native_surfaces = true;
            let native = UiTree::new(field(&state, SCHEME.palette())).layout(size);
            let slots: Vec<_> = native.access.iter().filter(|node| node.id == "ios-search").collect();
            assert_eq!(slots.len(), 1);
            assert_eq!(slots[0].role, limn::Role::Group);
            assert!(slots[0].rect.width >= TAP && slots[0].rect.height >= TAP);
            assert!(!native.regions.iter().any(|region| region.kind == HitKind::TextInput));
            assert!(!native.access.iter().any(|node| node.id == "ios-paste:search"));
            state.mobile_ui.native_surfaces = false;
            let fallback = UiTree::new(field(&state, SCHEME.palette())).layout(size);
            assert!(!fallback.access.iter().any(|node| node.id == "ios-search"));
            assert!(fallback.access.iter().any(|node| node.id == if editing { "search" } else { "mobile-search-edit" }));
        }
    }

    #[test]
    fn search_and_filter_capsules_keep_their_full_touch_geometry() {
        let mut state = phone();
        let size = Size::new(390.0, 500.0);
        let assert_capsule = |frame: &Frame, id: &str| {
            let target = frame.regions.iter().find(|region| region.id == id).unwrap().rect;
            assert!(target.width >= TAP && target.height >= TAP);
            assert!(frame.commands.iter().any(|command| matches!(command,
                DrawCommand::FillRect { rect, radius, .. } if *rect == target && *radius >= rect.height / 2.0)),
                "{id} rounds its actual painted touch height, not the smaller default control height");
        };
        assert_capsule(&UiTree::new(field(&state, SCHEME.palette())).layout(size), "mobile-search-edit");
        state.mobile_ui.search_editing = true;
        assert_capsule(&UiTree::new(field(&state, SCHEME.palette())).layout(size), "search");
        state.submitted = Some("consulta".into());
        assert_capsule(&UiTree::new(results(&state)).layout(size), "search-filter:all");
    }

    #[test]
    fn debounce_and_loading_keep_the_same_scroll_offset_and_selectable_result() {
        let mut state = phone();
        state.query = "anterior".into();
        state.submitted = Some(state.query.clone());
        state.found.tracks = (0..30).map(|index| Track {
            id: format!("held-{index}"), title: format!("Resultado {index}"), artist: "Artista".into(),
            artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1,
        }).collect();
        let size = Size::new(390.0, 500.0);
        let mut tree = UiTree::new(results(&state));
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        let scroll = frame.regions.iter().find(|region| region.id == "mobile-search-results" && region.kind == HitKind::Scroll).unwrap().rect;
        tree.handle_event(UiEvent::ScrollDirect { position: Point::new(scroll.x + 20.0, scroll.y + 80.0), delta_y: -150.0 });
        let original = tree.layout(size);
        let offset = tree.scroll_offset("mobile-search-results");
        assert!(offset > 0.0);
        let held = original.regions.iter().find(|region| region.id == "track:3" && region.kind == HitKind::Button).unwrap().rect;
        state.mobile_query("nueva", Instant::now());
        for loading in [false, true] {
            state.searching = loading;
            tree.set_root(results(&state));
            let frame = tree.layout(size);
            assert_eq!(tree.scroll_offset("mobile-search-results"), offset);
            let current = frame.regions.iter().find(|region| region.id == "track:3" && region.kind == HitKind::Button).unwrap().rect;
            assert_eq!(current, held, "status changes must not move the retained result rows");
            assert!(text(&frame, "Buscando…"));
            assert!(!text(&frame, "Sin resultados"));
        }
        state.searching = false;
        state.mobile_ui.pending = None;
        state.search_error = Some("Sin conexión".into());
        tree.set_root(results(&state));
        let frame = tree.layout(size);
        assert_eq!(tree.scroll_offset("mobile-search-results"), offset, "failure retains the reader's list position");
        assert!(frame.regions.iter().any(|region| region.id == "track:3" && region.kind == HitKind::Button));
        let retry = frame.regions.iter().find(|region| region.id == "search-submit" && region.kind == HitKind::Button)
            .expect("recovery remains visible even when the results are scrolled").rect;
        assert!(retry.width >= TAP && retry.height >= TAP);
        assert!(retry.y >= 0.0 && retry.y + retry.height <= size.height);
        let point = Point::new(retry.x + retry.width / 2.0, retry.y + retry.height / 2.0);
        assert_eq!(frame.press_region(point).unwrap().id, "search-submit");
        tree.handle_event(UiEvent::PointerDown(point));
        assert!(tree.handle_event(UiEvent::PointerUp(point)).iter().any(|action|
            matches!(action, UiAction::Clicked { id } if id == "search-submit")), "the visible Retry control receives the tap");
    }

    #[test]
    fn pending_or_failed_first_query_does_not_claim_that_the_catalog_has_no_results() {
        let mut state = phone();
        state.mobile_query("nueva", Instant::now());
        let frame = UiTree::new(results(&state)).layout(Size::new(390.0, 500.0));
        assert!(text(&frame, "Buscando…"));
        assert!(!text(&frame, "Sin resultados"));
        assert!(frame.regions.iter().any(|region| region.id == "mobile-search-results" && region.kind == HitKind::Scroll));
        state.mobile_ui.pending = None;
        state.search_error = Some("Sin conexión".into());
        let failed = UiTree::new(results(&state)).layout(Size::new(390.0, 500.0));
        assert!(!text(&failed, "Sin resultados"), "an unavailable answer is not an empty catalog");
        assert!(failed.regions.iter().any(|region| region.id == "search-submit" && region.kind == HitKind::Button));
    }
}
