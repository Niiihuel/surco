//! A search tab with deliberate keyboard ownership and real catalog results.
//! Figma iOS kit: 1BzqhExmVONm3WMDZBMqPj / 604:9087, 604:9150, 604:9171.

use super::*;
use crate::state::mobile::SearchFilter;

/// One height for the idle and the editing field. Only the header's collapse moves the
/// native bar, so opening the keyboard never makes it jump or resize.
pub(super) const FIELD_HEIGHT: f32 = TAP + 16.0;

pub(super) fn field(state: &State, palette: ComponentPalette) -> Element {
    let editing = state.mobile_ui.search_editing;
    let compact = state.mobile_search_compact();
    if cfg!(any(target_os = "ios", test)) && state.mobile_ui.native_surfaces {
        let slot = Element::panel(Element::spacer().size(None, Some(TAP))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-search").name("Buscar música")))
            // Transparent: the native header's blur continues behind the field.
            .padding(8.0);
        return horizontal_inset(slot, 8.0, 8.0).size(None, Some(FIELD_HEIGHT));
    }
    let field = if editing {
        SearchField::new("search", &state.query, "¿Qué querés escuchar?", SCHEME.on_raised())
            .size(ControlSize::Spacious).render().radius(TAP / 2.0).size(None, Some(TAP)).flex(1.0)
    } else {
        Button::new("mobile-search-edit", if state.query.is_empty() { "¿Qué querés escuchar?" } else { &state.query }, palette)
            .icon(Icon::Search).variant(if compact { ButtonVariant::Secondary } else { ButtonVariant::Default })
            .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render().flex(1.0)
    };
    let mut row = vec![field];
    if editing {
        // The explicit control is useful for pasted links as well as one-letter queries.
        if !state.query.is_empty() { row.push(icon("mobile-search-clear", Icon::Close, "Borrar búsqueda", palette)); }
        row.push(Button::new("mobile-search-cancel", "Cancelar", palette).variant(ButtonVariant::Ghost).size(ControlSize::Spacious).render().size(None, Some(TAP)));
    }
    horizontal_inset(Element::row(row).gap(4.0).padding(8.0), 8.0, 8.0).size(None, Some(FIELD_HEIGHT))
}

/// Figma "Item Artist" title: section headings are 17pt bold.
fn section(title: &str, palette: ComponentPalette) -> Element {
    Text::new(title, palette).role(TextRole::Heading).font_size(17.0).render().font_family(FontFamily::SansBold)
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
    // The chips start on the page's 16pt margin and scroll out past both edges.
    let filters = Element::horizontal_scroll_2d("mobile-search-filters", horizontal_inset(Element::row(vec![
        tab("all", "Todo", SearchFilter::All), tab("tracks", "Canciones", SearchFilter::Tracks),
        tab("artists", "Artistas", SearchFilter::Artists), tab("albums", "Álbumes", SearchFilter::Albums),
    ]).gap(8.0), 8.0, 8.0)).drag_to_pan().scrollbar_visibility(SCROLL_INDICATOR);
    let found = &state.found;
    let mut rows = Vec::new();
    if matches!(chosen, SearchFilter::All | SearchFilter::Tracks) && !found.tracks.is_empty() {
        rows.push(section("Canciones", palette).padding(8.0));
        rows.extend(found.tracks.iter().enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, &track.artist, palette)));
    }
    if matches!(chosen, SearchFilter::All | SearchFilter::Artists) && !found.artists.is_empty() {
        rows.push(section("Artistas", palette).padding(8.0));
        rows.extend(found.artists.iter().enumerate().map(|(index, artist)| result(state, format!("artist:{index}"), &artist.name, "Artista", &artist.artwork_url, true, palette)));
    }
    if matches!(chosen, SearchFilter::All | SearchFilter::Albums) && !found.albums.is_empty() {
        rows.push(section("Álbumes", palette).padding(8.0));
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
    let status = if state.search_error.is_some() {
        Element::row(vec![Element::column(vec![
            Text::new("No pudimos actualizar", palette).font_size(13.0).render(),
            Text::new("Intentá de nuevo en un momento.", palette).font_size(12.0).muted().render(),
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
    // Figma "Item / Music, Small": 48pt artwork, 12pt to the copy, 64pt tall.
    ListRow::new(id, title, palette).sizing(ListRowSizing { height: Some(64.0), two_line_height: Some(64.0), gap: 12.0, leading_width: 48.0, ..TRACK_ROW })
        .leading(if round { picture.radius(24.0) } else { picture.radius(4.0) })
        .subtitle(subtitle).title_fade(true).render()
}

fn idle(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut body = Vec::new();
    if !state.collection.searches.is_empty() {
        body.push(Element::row(vec![section("Búsquedas recientes", palette).centered_y().flex(1.0),
            icon("recent-search-clear", Icon::Trash, "Borrar búsquedas recientes", palette)]).gap(8.0));
        body.push(Element::column(state.collection.searches.iter().enumerate().map(|(index, query)| recent(index, query, palette)).collect()));
    }
    if state.query.contains("://") {
        body.push(Text::new("Pegaste un enlace. Tocá Buscar en el teclado para abrirlo.", palette).muted().render());
    }
    // Focusing the field keeps this page as it is: the keyboard rises over it
    // instead of the content being swapped while the header collapses.
    body.push(section("Explorá tu próximo sonido", palette));
    body.push(browse(width - 32.0, palette));
    ScrollArea::new("mobile-search-idle", Element::column(body).gap(16.0).padding(16.0)).render().flex(1.0)
}

/// Figma "Item / Suggestion, Small": a 24pt glyph, the query, and an x that
/// forgets this one search.
fn recent(index: usize, query: &str, palette: ComponentPalette) -> Element {
    let copy = Element::row(vec![Element::icon(Icon::Search).large_icon(24.0).foreground(palette.muted).centered_y(),
        Text::new(query, palette).font_size(15.0).truncate().render().centered_y().flex(1.0)]).gap(12.0);
    let main = Element::clickable(format!("recent-search:{index}"), copy)
        .accessible(limn::Access::new(limn::Role::Button).name(format!("Buscar {query}"))).flex(1.0);
    let forget = icon(&format!("recent-search-remove:{index}"), Icon::Close, &format!("Quitar {query} de las búsquedas recientes"), palette);
    Element::row(vec![main, forget.centered_y()]).gap(8.0).size(None, Some(56.0))
}

/// Figma "Card Type - horizontal": a bold title over the genre's color, with an
/// artwork-like tile tucked into the corner and cropped by the card.
pub(super) fn browse(width: f32, palette: ComponentPalette) -> Element {
    let tile = |title: &str, query: &str, tint: Color| {
        // shortcut: limn has no rotation, so the Figma kit's 25° tilt is set square;
        // tilt it once Element can rotate.
        let art = Element::panel(Element::icon(Icon::MusicNote).large_icon(28.0).foreground(Color::rgba(255, 255, 255, 190)).centered())
            .background(mix(tint, Color::rgb(255, 255, 255), 0.18)).radius(4.0).size(Some(64.0), Some(64.0))
            .shadow(Color::rgba(0, 0, 0, 64), limn::Point::new(0.0, 0.0), 10.0, 0.0).translate(limn::Point::new(14.0, 10.0));
        let corner = Element::column(vec![Element::spacer().flex(1.0), Element::row(vec![Element::spacer().flex(1.0), art])]);
        let copy = Text::new(title, palette).font_size(16.0).render().font_family(FontFamily::SansBold).padding(12.0);
        Element::clickable(format!("browse:{query}"), Element::panel(Element::stack(vec![corner, copy])).background(tint).radius(6.0).clip_children())
            .accessible(limn::Access::new(limn::Role::Button).name(format!("Buscar {query}")))
            .size(Some((width - 12.0) / 2.0), Some(98.0))
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
    fn native_search_slot_keeps_its_geometry_while_the_keyboard_opens() {
        let mut state = phone();
        state.mobile_ui.native_surfaces = true;
        let slot = |state: &State| UiTree::new(field(state, SCHEME.palette())).layout(Size::new(390.0, FIELD_HEIGHT))
            .access.iter().find(|node| node.id == "ios-search").unwrap().rect;
        let idle = slot(&state);
        assert_eq!((idle.x, idle.y, idle.width, idle.height), (16.0, 8.0, 358.0, TAP), "the kit's 16pt margins");
        state.mobile_ui.search_editing = true;
        assert_eq!(slot(&state), idle, "focus restyles the native bar but never moves or resizes it");
        state.mobile_ui.search_editing = false;
        state.query = "cerati".into();
        assert_eq!(slot(&state), idle, "a compact query keeps the same slot");
    }

    #[test]
    fn recent_searches_are_aligned_suggestion_rows_that_can_be_forgotten_one_by_one() {
        let mut state = phone();
        state.collection.searches = vec!["Soda Stereo".into()];
        let mut tree = UiTree::new(results(&state));
        let frame = tree.layout(Size::new(390.0, 700.0));
        let target = |id: &str| frame.regions.iter().find(|region| region.id == id && region.kind == HitKind::Button).unwrap().rect;
        let row = target("recent-search:0");
        let forget = target("recent-search-remove:0");
        assert_eq!((row.x, row.height), (16.0, 56.0));
        assert!(forget.width >= TAP && forget.height >= TAP && forget.x >= row.x + row.width && forget.x + forget.width <= 374.01);
        let glyph = frame.commands.iter().find_map(|command| match command {
            DrawCommand::Icon { rect, icon: Icon::Search, .. } => Some(*rect), _ => None }).unwrap();
        assert_eq!((glyph.x, glyph.width), (16.0, 24.0), "a full-size glyph on the margin, not a small one adrift");
        assert!((glyph.y + glyph.height / 2.0 - (row.y + row.height / 2.0)).abs() < 0.5);
        let title = frame.commands.iter().find_map(|command| match command {
            DrawCommand::Text { text, position, .. } if text == "Soda Stereo" => Some(*position), _ => None }).unwrap();
        assert_eq!(title.x, 52.0, "the title starts 12pt after the glyph");
        let point = Point::new(forget.x + forget.width / 2.0, forget.y + forget.height / 2.0);
        tree.handle_event(UiEvent::PointerDown(point));
        assert!(tree.handle_event(UiEvent::PointerUp(point)).iter().any(|action|
            matches!(action, UiAction::Clicked { id } if id == "recent-search-remove:0")));
        state.mobile_ui.search_editing = true;
        let editing = UiTree::new(results(&state)).layout(Size::new(390.0, 700.0));
        assert!(editing.regions.iter().any(|region| region.id == "browse:Rock argentino"),
            "focusing the field does not swap the page under the rising keyboard");
        assert!(!editing.commands.iter().any(|command| matches!(command, DrawCommand::Icon { icon: Icon::ArrowRight, .. })));
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
        let diagnostic = "ytmdapi internal_parse_error: respuesta inesperada";
        state.search_error = Some(diagnostic.into());
        tree.set_root(results(&state));
        let frame = tree.layout(size);
        assert_eq!(tree.scroll_offset("mobile-search-results"), offset, "failure retains the reader's list position");
        assert_eq!(state.search_error.as_deref(), Some(diagnostic), "the raw diagnostic remains available to the engine");
        assert!(!frame.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text.contains("ytmdapi") || text.contains("internal_parse_error"))),
            "provider diagnostics do not leak into the user-facing recovery copy");
        assert!(text(&frame, "Intentá de nuevo en un momento."));
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
