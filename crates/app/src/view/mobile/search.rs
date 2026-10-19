//! A search tab with deliberate keyboard ownership and real catalog results.
//! Figma iOS kit: 1BzqhExmVONm3WMDZBMqPj / 604:9087, 604:9150, 604:9171.

use super::*;
use crate::state::mobile::SearchFilter;
use crate::state::Searched;

/// One height for the idle and the editing field. Only the header's collapse moves the
/// native bar, so opening the keyboard never makes it jump or resize.
pub(super) const FIELD_HEIGHT: f32 = TAP + 16.0;
/// How many songs all the results show, as Spotify's; the chip shows the rest.
const SONGS: usize = 4;

pub(super) fn field(state: &State, palette: ComponentPalette) -> Element {
    let editing = state.mobile_ui.search_editing;
    let compact = state.mobile_search_compact();
    if cfg!(any(target_os = "ios", test)) && state.mobile_ui.native_surfaces {
        let slot = Element::panel(Element::spacer().size(None, Some(TAP))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-search").name(t!("mobile-search-music"))))
            // Transparent: the native header's blur continues behind the field.
            .padding(8.0);
        return horizontal_inset(slot, 8.0, 8.0).size(None, Some(FIELD_HEIGHT));
    }
    // The cross that empties it is always inside the field, at its end.
    let field = if editing {
        SearchField::new("search", &state.query, t!("common-search-placeholder"), SCHEME.on_raised())
            .size(ControlSize::Spacious).render().size(None, Some(TAP)).round()
    } else {
        Button::new("mobile-search-edit", if state.query.is_empty() { t!("common-search-placeholder") } else { state.query.as_str().into() }, palette)
            .icon(Icon::Search).variant(if compact { ButtonVariant::Secondary } else { ButtonVariant::Default })
            .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render()
    };
    let field = if state.query.is_empty() { field } else { inside_cross(field, "mobile-search-clear", &t!("mobile-clear-search"), palette) }.flex(1.0);
    let mut row = vec![field];
    if editing {
        row.push(Button::new("mobile-search-cancel", t!("common-cancel"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Spacious).round().render().size(None, Some(TAP)));
    }
    horizontal_inset(Element::row(row).gap(4.0).padding(8.0), 8.0, 8.0).size(None, Some(FIELD_HEIGHT))
}

/// Figma "Item Artist" title: section headings are 17pt bold.
fn section(title: &str, palette: ComponentPalette) -> Element {
    Text::new(title, palette).role(TextRole::Heading).font_size(17.0).render().font_family(FontFamily::SansBold)
}

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let pending = state.searching || state.search_due.is_some();
    let has_results = !state.found.tracks.is_empty() || !state.found.artists.is_empty() || !state.found.albums.is_empty();
    if state.submitted.is_none() && !pending && !has_results && state.search_error.is_none() {
        return idle(state, width, palette);
    }
    let chosen = state.search_filter;
    // Spotify's chips: a 32pt pill, lit while chosen, on a touch target of full height.
    let tab = |key: &str, label: &str, value| {
        let on = chosen == value;
        let copy = Text::new(label, palette).font_size(14.0).render().foreground(if on { SCHEME.canvas } else { palette.text }).centered_y();
        let edge = || Element::spacer().size(Some(14.0), None);
        let pill = Element::panel(Element::row(vec![edge(), copy, edge()]))
            .background(if on { palette.text } else { Color::rgb(42, 42, 42) }).pressed_background(if on { ROUND_HELD } else { Color::rgb(62, 62, 62) })
            .radius(16.0).size(None, Some(32.0));
        Element::clickable(format!("search-filter:{key}"), pill.centered_y())
            .accessible(limn::Access::new(limn::Role::Tab).name(label).selected(on)).size(None, Some(TAP))
    };
    // The chips start on the page's 16pt margin and scroll out past both edges.
    let filters = Element::horizontal_scroll_2d("mobile-search-filters", horizontal_inset(Element::row(vec![
        tab("all", &t!("common-all"), SearchFilter::All), tab("tracks", &t!("common-songs-heading"), SearchFilter::Tracks),
        tab("artists", &t!("common-artists"), SearchFilter::Artists), tab("albums", &t!("common-albums"), SearchFilter::Albums),
    ]).gap(8.0), 8.0, 8.0)).drag_to_pan().scrollbar_visibility(SCROLL_INDICATOR);
    let found = &state.found;
    let mut rows = Vec::new();
    // Songs are played as a list's: the big play plays them all, and a song the ones after it.
    let play_all = || {
        let (glyph, hint) = shuffle_step(state);
        let shuffle = controls::action("page-shuffle", glyph, &hint, 22.0, palette);
        let shuffle = if state.player.shuffle { shuffle.background(palette.selected) } else { shuffle };
        Element::row(vec![shuffle.centered_y(), controls::playback("page-play", false, 48.0, 24.0, palette)]).gap(8.0)
    };
    match chosen {
        SearchFilter::All => {
            if !found.tracks.is_empty() {
                rows.push(Element::row(vec![section(&t!("common-songs-heading"), palette).centered_y().flex(1.0), play_all()]).padding(8.0));
                rows.extend(found.tracks.iter().take(SONGS).enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, &track.artist, palette)));
                if found.tracks.len() > SONGS {
                    rows.push(Button::new("search-filter:tracks", t!("mobile-see-all"), palette).variant(ButtonVariant::Ghost)
                        .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render().padding(4.0));
                }
            }
            // The other kinds are shelves to swipe, as on the home.
            if !found.artists.is_empty() {
                let tiles = found.artists.iter().enumerate().map(|(index, artist)| tile(state, format!("artist:{index}"), &artist.name, Some(t!("common-artist").into_owned()), &artist.artwork_url, true, palette));
                rows.push(shelf(state, "found-artists", &t!("common-artists"), tiles.collect(), palette));
            }
            if !found.albums.is_empty() {
                rows.push(shelf(state, "found-albums", &t!("common-albums"), release_tiles(state, "album", &found.albums, palette), palette));
            }
        }
        SearchFilter::Tracks if !found.tracks.is_empty() => {
            rows.push(Element::row(vec![Element::spacer().flex(1.0), play_all()]).padding(8.0));
            rows.extend(state.page_rows().into_iter().map(|(place, track)| track_row(state, format!("page-track:{place}"), track, &track.artist, palette)));
        }
        SearchFilter::Artists => rows.extend(found.artists.iter().enumerate().map(|(index, artist)| result(state, format!("artist:{index}"), &artist.name, &t!("common-artist"), &artist.artwork_url, true, palette))),
        SearchFilter::Albums => rows.extend(found.albums.iter().enumerate().map(|(index, album)| result(state, format!("album:{index}"), &album.title, &album.artist.as_deref().map_or_else(|| t!("common-album"), Cow::Borrowed), &album.artwork_url, false, palette))),
        SearchFilter::Tracks => {}
    }
    if rows.is_empty() {
        if pending {
            // The shape of the rows to come, a result's own: its cover and its two lines,
            // pulsing in step with the covers on their way.
            let pulse = ComponentPalette { raised: palette.selected, ..palette };
            rows.extend([(190.0, 110.0), (150.0, 90.0), (210.0, 130.0), (170.0, 100.0)].into_iter().cycle().take(8).map(|(title, subtitle)| {
                let lines = Element::column(vec![
                    limn::Skeleton::line("cover-coming", pulse, title).height(14.0).render(),
                    limn::Skeleton::line("cover-coming", pulse, subtitle).height(11.0).render(),
                ]).gap(8.0).centered_y();
                Element::row(vec![limn::Skeleton::block("cover-coming", pulse, 48.0, 48.0).radius(4.0).render().centered_y(), lines])
                    .gap(12.0).padding(8.0).size(None, Some(64.0))
            }));
        } else {
            rows.push(if state.search_error.is_some() { Element::spacer().size(None, Some(TAP)) }
                else { Empty::new(t!("common-no-results"), t!("mobile-no-results-detail"), palette).icon(Icon::Search).render().padding(20.0) });
        }
    }
    // This retained scroll and its parent stay mounted throughout debounce,
    // loading and failures; the previous result rows remain usable meanwhile.
    let list = ScrollArea::new("mobile-search-results", Element::column(rows).gap(4.0)).render().flex(1.0);
    // Keep recovery outside the retained scroll: a failure while browsing deep
    // results must offer Retry without asking the user to find the list's top.
    let status = if state.search_error.is_some() {
        Element::row(vec![Element::column(vec![
            Text::new(t!("mobile-search-failed"), palette).font_size(13.0).render(),
            Text::new(t!("mobile-search-failed-detail"), palette).font_size(12.0).muted().render(),
        ]).gap(4.0).flex(1.0),
            Button::new("search-submit", t!("common-retry"), palette).variant(ButtonVariant::Secondary)
                .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render(),
        ]).gap(8.0).padding(8.0)
    } else {
        // Searching says so by the rows' shape, not in words; with nothing to say, the
        // results start right under the chips.
        let said = !pending && state.submitted.is_none() && has_results;
        let copy = if said {
            Text::new(if state.query.contains("://") { t!("mobile-search-open-link") } else { t!("mobile-previous-results") }, palette)
                .font_size(12.0).muted().render()
        } else { Element::spacer() };
        copy.size(None, Some(if said { 20.0 } else { 0.0 }))
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
        body.push(Element::row(vec![section(&t!("mobile-recent-searches"), palette).centered_y().flex(1.0),
            icon("recent-search-clear", Icon::Trash, &t!("mobile-clear-recent-searches"), palette)]).gap(8.0));
        body.push(Element::column(state.collection.searches.iter().enumerate().map(|(index, searched)| recent(state, index, searched, palette)).collect()));
    }
    if state.query.contains("://") {
        body.push(Text::new(t!("mobile-pasted-link"), palette).muted().render());
    }
    // Focusing the field keeps this page as it is: the keyboard rises over it
    // instead of the content being swapped while the header collapses.
    body.push(section(&t!("mobile-browse-all"), palette));
    body.push(browse(usize::MAX, width - 32.0, 2, 98.0, palette));
    ScrollArea::new("mobile-search-idle", Element::column(body).gap(16.0).padding(16.0)).render().flex(1.0)
}

/// Figma "Item / Suggestion, Small": a 24pt glyph and the query, or a result chosen with its
/// picture and what it is, and an x that forgets this one search.
fn recent(state: &State, index: usize, searched: &Searched, palette: ComponentPalette) -> Element {
    let (title, subtitle, artwork, round) = crate::view::remembered(searched);
    let copy = match artwork {
        None => Element::row(vec![Element::icon(Icon::Search).large_icon(24.0).foreground(palette.muted).centered_y(),
            Text::new(title, palette).font_size(15.0).truncate().render().centered_y().flex(1.0)]).gap(12.0),
        Some(artwork) => Element::row(vec![cover(state, artwork, 48.0, palette).radius(if round { 24.0 } else { 4.0 }).centered_y(),
            Element::column(vec![
                Text::new(title, palette).font_size(15.0).truncate().render(),
                Text::new(&subtitle, palette).font_size(13.0).muted().truncate().render(),
            ]).gap(4.0).centered_y().flex(1.0)]).gap(12.0),
    };
    let main = Element::clickable(format!("recent-search:{index}"), copy)
        .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-search-for", query = title))).flex(1.0);
    let forget = icon(&format!("recent-search-remove:{index}"), Icon::Close, &t!("mobile-forget-search", query = title), palette);
    Element::row(vec![main, forget.centered_y()]).gap(8.0).size(None, Some(if artwork.is_some() { 64.0 } else { 56.0 }))
}

/// The genres to browse: the title on the card, what it searches for (the same in every
/// language), and its colour.
type Genre = (fn() -> Cow<'static, str>, &'static str, Color);
const GENRES: [Genre; 35] = [
    (|| t!("mobile-genre-rock"), "Rock argentino", Color::rgb(230, 30, 50)),
    (|| t!("mobile-genre-pop"), "Pop latino", Color::rgb(71, 125, 149)),
    (|| t!("mobile-genre-cumbia"), "Cumbia", Color::rgb(39, 133, 106)),
    (|| t!("mobile-genre-trap"), "Trap argentino", Color::rgb(186, 93, 7)),
    (|| t!("mobile-genre-reggaeton"), "Reggaetón", Color::rgb(220, 20, 140)),
    (|| t!("mobile-genre-indie"), "Indie latino", Color::rgb(96, 129, 8)),
    (|| t!("mobile-genre-electronic"), "Electrónica", Color::rgb(13, 115, 236)),
    (|| t!("mobile-genre-hip-hop"), "Hip hop", Color::rgb(30, 50, 100)),
    (|| t!("mobile-genre-chill"), "Acústico relajado", Color::rgb(141, 103, 171)),
    (|| t!("mobile-genre-workout"), "Música para entrenar", Color::rgb(119, 119, 119)),
    (|| t!("mobile-genre-folklore"), "Folklore argentino", Color::rgb(232, 17, 91)),
    (|| t!("mobile-genre-tango"), "Tango", Color::rgb(176, 40, 151)),
    (|| t!("mobile-genre-cuarteto"), "Cuarteto cordobés", Color::rgb(225, 51, 0)),
    (|| t!("mobile-genre-rnb"), "R&B", Color::rgb(165, 103, 82)),
    (|| t!("mobile-genre-salsa"), "Salsa", Color::rgb(80, 155, 245)),
    (|| t!("mobile-genre-bachata"), "Bachata", Color::rgb(20, 138, 8)),
    (|| t!("mobile-genre-jazz"), "Jazz", Color::rgb(230, 30, 50)),
    (|| t!("mobile-genre-classical"), "Música clásica", Color::rgb(71, 125, 149)),
    (|| t!("mobile-genre-metal"), "Metal", Color::rgb(39, 133, 106)),
    (|| t!("mobile-genre-punk"), "Punk rock", Color::rgb(186, 93, 7)),
    (|| t!("mobile-genre-blues"), "Blues", Color::rgb(220, 20, 140)),
    (|| t!("mobile-genre-soul-funk"), "Soul funk", Color::rgb(96, 129, 8)),
    (|| t!("mobile-genre-reggae"), "Reggae", Color::rgb(13, 115, 236)),
    (|| t!("mobile-genre-k-pop"), "K-pop", Color::rgb(30, 50, 100)),
    (|| t!("mobile-genre-brazilian"), "Música popular brasileña", Color::rgb(141, 103, 171)),
    (|| t!("mobile-genre-lo-fi"), "Lo-fi", Color::rgb(119, 119, 119)),
    (|| t!("mobile-genre-party"), "Música para fiesta", Color::rgb(232, 17, 91)),
    (|| t!("mobile-genre-sleep"), "Música para dormir", Color::rgb(176, 40, 151)),
    (|| t!("mobile-genre-focus"), "Música para concentrarse", Color::rgb(225, 51, 0)),
    (|| t!("mobile-genre-romantic"), "Baladas románticas", Color::rgb(165, 103, 82)),
    (|| t!("mobile-genre-80s"), "Éxitos de los 80", Color::rgb(80, 155, 245)),
    (|| t!("mobile-genre-90s"), "Éxitos de los 90", Color::rgb(20, 138, 8)),
    (|| t!("mobile-genre-video-games"), "Música de videojuegos", Color::rgb(230, 30, 50)),
    (|| t!("mobile-genre-anime"), "Anime openings", Color::rgb(71, 125, 149)),
    (|| t!("mobile-genre-soundtracks"), "Bandas sonoras de películas", Color::rgb(39, 133, 106)),
];

/// The genres by their names and what each searches for.
pub(in crate::view) fn genres() -> impl Iterator<Item = (Cow<'static, str>, &'static str)> {
    GENRES.iter().map(|(name, query, _)| (name(), *query))
}

/// Figma "Card Type - horizontal": a bold title over the genre's color, with an
/// artwork-like tile tucked into the corner and cropped by the card. The first `count`
/// genres, `columns` cards to a row of `width`, each `height` tall; the corner tile grows
/// with the card.
pub(in crate::view) fn browse(count: usize, width: f32, columns: usize, height: f32, palette: ComponentPalette) -> Element {
    let side = (width - 12.0 * (columns - 1) as f32) / columns as f32;
    // A record half out of its sleeve in the tile's corner: the sleeve in the genre's colour,
    // lighter, and the disc behind it, its grooves and its label. All inside the tile, whose
    // clip is square.
    let sleeve = (height * 0.56).round();
    let disc = (sleeve * 0.92).round();
    let (sleeve_x, sleeve_y) = (side - sleeve - 10.0, height - sleeve - 10.0);
    let placed = |piece: Element, x: f32, y: f32| Element::column(vec![Element::row(vec![piece, Element::spacer().flex(1.0)]), Element::spacer().flex(1.0)])
        .translate(limn::Point::new(x, y));
    let round = |across: f32, inside: Element| Element::panel(inside.centered()).size(Some(across), Some(across)).radius(across / 2.0);
    let tile = |&(title, query, tint): &Genre| {
        let groove = Color::rgba(255, 255, 255, 22);
        let hole = Element::spacer().size(Some(disc * 0.05), Some(disc * 0.05)).background(Color::rgb(14, 14, 14)).radius(disc * 0.025);
        let label = round(disc * 0.36, hole).background(mix(tint, Color::rgb(255, 255, 255), 0.35));
        let grooves = [0.52, 0.68, 0.84].into_iter().fold(label, |inside, share| round(disc * share, inside).border(groove, 1.0));
        let record = round(disc, grooves).background(Color::rgb(18, 18, 18))
            .shadow(Color::rgba(0, 0, 0, 90), limn::Point::new(0.0, 2.0), 10.0, 0.0);
        let cover = Element::panel(Element::icon(Icon::MusicNote).large_icon(sleeve * 0.4).foreground(Color::rgba(255, 255, 255, 170)).centered())
            .background(mix(tint, Color::rgb(255, 255, 255), 0.18)).radius(4.0).size(Some(sleeve), Some(sleeve))
            .shadow(Color::rgba(0, 0, 0, 72), limn::Point::new(0.0, 2.0), 10.0, 0.0);
        let corner = Element::stack(vec![
            placed(record, sleeve_x - disc * 0.62, sleeve_y + (sleeve - disc) / 2.0),
            placed(cover, sleeve_x, sleeve_y),
        ]);
        let copy = Text::new(title(), palette).font_size(if height > 120.0 { 22.0 } else { 16.0 }).render().font_family(FontFamily::SansBold).padding(12.0);
        Element::clickable(format!("browse:{query}"), Element::panel(Element::stack(vec![corner, copy])).background(tint).radius(6.0).clip_children())
            .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-search-for", query = query)))
            .size(Some(side), Some(height))
    };
    Element::column(GENRES[..count.min(GENRES.len())].chunks(columns).map(|row| Element::row(row.iter().map(tile).collect()).gap(12.0)).collect()).gap(12.0)
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
    fn genre_titles_follow_the_language_and_their_searches_do_not() {
        let (title, query, _) = GENRES[8];
        assert_eq!((title().as_ref(), query), ("Para bajar\nun cambio", "Acústico relajado"));
        crate::i18n::set(Language::English);
        assert_eq!((title().as_ref(), query), ("Chill\nout", "Acústico relajado"));
        crate::i18n::set(Language::Spanish);
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
        // A chip paints Spotify's 32pt pill inside a touch target of full height.
        let frame = UiTree::new(results(&state)).layout(size);
        let target = frame.regions.iter().find(|region| region.id == "search-filter:all").unwrap().rect;
        assert!(target.width >= TAP && target.height >= TAP);
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::FillRect { rect, radius, .. }
            if rect.height == 32.0 && *radius >= 16.0 && rect.y >= target.y && rect.y + rect.height <= target.y + target.height)));
    }

    #[test]
    fn debounce_and_loading_keep_the_same_scroll_offset_and_selectable_result() {
        let mut state = phone();
        // All of them shows a few songs; their chip lists every one.
        state.search_filter = SearchFilter::Tracks;
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
        let held = original.regions.iter().find(|region| region.id == "page-track:3" && region.kind == HitKind::Button).unwrap().rect;
        state.mobile_query("nueva", Instant::now());
        for loading in [false, true] {
            state.searching = loading;
            tree.set_root(results(&state));
            let frame = tree.layout(size);
            assert_eq!(tree.scroll_offset("mobile-search-results"), offset);
            let current = frame.regions.iter().find(|region| region.id == "page-track:3" && region.kind == HitKind::Button).unwrap().rect;
            assert_eq!(current, held, "status changes must not move the retained result rows");
            assert!(!text(&frame, "Buscando…"), "the rows say it is searching, not words");
            assert!(!text(&frame, "Sin resultados"));
        }
        state.searching = false;
        state.search_due = None;
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
        assert!(frame.regions.iter().any(|region| region.id == "page-track:3" && region.kind == HitKind::Button));
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
        assert!(!text(&frame, "Buscando…"), "the shape of the rows to come says it, not words");
        assert!(!text(&frame, "Sin resultados"));
        assert!(frame.regions.iter().any(|region| region.id == "mobile-search-results" && region.kind == HitKind::Scroll));
        state.search_due = None;
        state.search_error = Some("Sin conexión".into());
        let failed = UiTree::new(results(&state)).layout(Size::new(390.0, 500.0));
        assert!(!text(&failed, "Sin resultados"), "an unavailable answer is not an empty catalog");
        assert!(failed.regions.iter().any(|region| region.id == "search-submit" && region.kind == HitKind::Button));
    }
}
