//! Phone layout over Surco's existing pages, actions and playback state.

use super::*;

mod search;
mod home;
mod player;
mod chrome;
mod sheets;
pub(super) mod details;
#[cfg(test)]
mod dialog_tests;

const TAP: f32 = 44.0;
const NAV_HEIGHT: f32 = 64.0;
const HEADER_HEIGHT: f32 = 76.0;
const MINI_HEIGHT: f32 = 78.0;

pub(super) fn icon(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    IconButton::new(id, glyph, palette).variant(ButtonVariant::Ghost)
        .accessibility_label(label).size(ControlSize::Spacious).render().size(Some(TAP), Some(TAP))
}

pub(super) fn view(state: &State, window: Size) -> Element {
    let palette = SCHEME.palette();
    let searching = state.page == Page::Search && state.mobile_search;
    let title = match &state.page {
        Page::Library => "Tu biblioteca",
        Page::Settings => "Configuración",
        Page::Playing => if state.queue_open { "Cola de reproducción" } else { "Ahora suena" },
        Page::Lyrics => "Letra",
        Page::Search if searching => "Buscar",
        Page::Search => greeting(chrono::Timelike::hour(&chrono::Local::now())),
        Page::Downloads => "Descargas",
        Page::Liked => "Tus me gusta",
        _ => "Tu música",
    };
    let mut header = Vec::new();
    if state.can_go_back() || state.page == Page::Playing {
        header.push(icon(if state.can_go_back() { "back" } else { "home" }, if state.page == Page::Playing { Icon::ChevronDown } else { Icon::ArrowLeft }, "Volver", palette));
    }
    header.push(Text::new(title, palette).font_size(if state.page == Page::Playing { 14.0 } else { 24.0 }).truncate().render().centered_y().flex(1.0));
    if state.page == Page::Playing && state.queue_open {
        header.push(icon("queue", Icon::Playlist, if state.queue_open { "Volver a la canción" } else { "Ver cola" }, palette));
    } else if state.page == Page::Playing && state.current().is_some() {
        header.push(icon("mobile-player-header-more", Icon::More, "Opciones de la canción", palette).dispatch_as("more:current"));
    }
    if state.page != Page::Settings && state.page != Page::Playing && !matches!(state.page, Page::Album(_) | Page::Artist(_) | Page::Playlist(_)) {
        header.push(icon("settings", Icon::Settings, "Configuración", palette));
    }
    let editing = searching && state.mobile_ui.search_editing;
    let keyboard = editing || state.mobile_ui.keyboard_visible;
    let header_height = if searching { if editing { 0.0 } else { HEADER_HEIGHT } } else { HEADER_HEIGHT };
    let header = Element::panel(Element::row(header).gap(8.0).padding(16.0).size(None, Some(HEADER_HEIGHT)))
        .size(None, Some(header_height)).clip_children();
    let mut top = vec![header];
    let mut top_height = header_height;
    if searching {
        top.push(search::field(state, palette));
        top_height += if editing { 68.0 } else { 72.0 } + if cfg!(any(target_os = "ios", test)) { 52.0 } else { 0.0 };
    }
    // The notice participates in flow, so longer messages never cover an action.
    let mut notices = Vec::new();
    if let Some(notice) = &state.mobile_ui.notice {
        notices.push(Element::panel(Element::row(vec![Text::new(notice, palette).render().flex(1.0),
            icon("mobile-notice-close", Icon::Close, "Cerrar aviso", palette)]).gap(8.0).padding(12.0)).background(SCHEME.raised));
    }
    let content = if state.page == Page::Playing && state.queue_open {
        upcoming(state, palette)
    } else if searching {
        search::content(state, window.width, palette)
    } else if state.page == Page::Search {
        home::content(state, window.width, palette)
    } else {
        page(state, window.width, palette)
    };
    let mut bottom = Vec::new();
    let mut bottom_height = 0.0;
    if state.page != Page::Playing && !keyboard {
        if let Some(track) = state.current() {
            bottom.push(mini_player(state, track, palette));
            bottom_height += MINI_HEIGHT;
        }
    }
    if state.page != Page::Playing && !keyboard { bottom.push(navigation(state, palette)); bottom_height += NAV_HEIGHT; }
    let content = if state.page == Page::Playing && !state.queue_open { content } else { content.animate_enter(route_key(state)) };
    let content = chrome::under_bars(content, top_height, bottom_height);
    let bars = Element::column(vec![Element::column(top).block_pointer("mobile-header-bar"), Element::spacer().flex(1.0), Element::column(bottom)]);
    notices.push(Element::stack(vec![content, bars]).flex(1.0));
    let content = Element::column(notices);
    let content = if state.page == Page::Playing { state.current().map_or(content.clone(), |track| washed(state, &track.artwork_url, content)) } else { content };
    let tree = Element::panel(content).background(SCHEME.canvas).size(Some(window.width), Some(window.height));
    let tree = Element::panel(tree).size(Some(window.width), Some(window.height)).clip_children();
    let tree = sheets::render(state, tree, palette);
    let tree = share_dialog(state, tree, window.width, palette);
    let tree = playlist_dialog(state, tree, window.width, palette);
    let tree = missed_dialog(state, import_dialog(state, tree, window.width, palette), window.width, palette);
    let tree = confirm_dialog(state, tree, palette);
    limn::BottomSheet::new(tree, state.equalizer_open, "Ecualizador", "Ajustá el sonido de este equipo.", "equalizer-close", palette)
        .max_height(680.0).body(equalizer(state, palette)).render()
}

fn route_key(state: &State) -> String {
    match &state.page {
        Page::Search => if state.mobile_search { "mobile:search" } else { "mobile:home" }.into(),
        Page::Album(album) => format!("mobile:album:{}", album.id),
        Page::Artist(artist) => format!("mobile:artist:{}", artist.id),
        Page::Playlist(id) => format!("mobile:playlist:{id}"),
        page => format!("mobile:{page:?}"),
    }
}

fn navigation(state: &State, palette: ComponentPalette) -> Element {
    let tab = |id: &str, glyph: Icon, label: &str, selected: bool| {
        let ink = if selected { palette.text } else { palette.muted };
        let copy = Element::column(vec![
            Element::icon(glyph).foreground(ink).centered_x().animate_foreground(format!("mobile-tab:{id}:icon")),
            Text::new(label, palette).role(TextRole::Caption).render().foreground(ink).centered_x().animate_foreground(format!("mobile-tab:{id}:label")),
        ]).gap(4.0).centered();
        Element::clickable(id, copy).accessible(limn::Access::new(limn::Role::Button).name(label).selected(selected))
            .size(None, Some(NAV_HEIGHT)).flex(1.0)
    };
    Element::row(vec![
        tab("home", Icon::Home, "Inicio", state.page == Page::Search && !state.mobile_search),
        tab("mobile-search", Icon::Search, "Buscar", state.page == Page::Search && state.mobile_search),
        tab("mobile-library", Icon::Playlist, "Biblioteca", matches!(state.page, Page::Library | Page::Downloads | Page::Liked | Page::Playlist(_))),
    ]).block_pointer("mobile-nav-bar")
}

fn mini_player(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    let label = Element::row(vec![
        cover(state, &track.artwork_url, TAP, palette),
        Element::column(vec![
            Text::new(&track.title, palette).role(TextRole::Label).truncate().render(),
            Text::new(state.player.error.as_deref().unwrap_or(&track.artist), palette).role(TextRole::Caption).muted().truncate().render(),
        ]).gap(4.0).centered_y().flex(1.0),
    ]).gap(12.0);
    let open = Element::clickable("mobile-playing", label)
        .accessible(limn::Access::new(limn::Role::Button).name(format!("Abrir reproductor: {}", track.title))).flex(1.0);
    let controls = Element::row(vec![
        open,
        icon("mobile-connections", Icon::Audio, "Conectar a un dispositivo", palette),
        icon("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" }, palette),
    ]).gap(8.0).padding(8.0);
    let progress = Progress::new(state.progress(), palette).height(2.0).render();
    Element::panel(Element::column(vec![controls, progress])).background(SCHEME.raised).radius(PANEL_RADIUS).padding(8.0).size(None, Some(MINI_HEIGHT)).block_pointer("mobile-mini-bar")
}

/// Overflow stays visible on touch screens, independently of pointer hover.
pub(super) fn track_row(state: &State, row: String, track: &Track, subtitle: &str, palette: ComponentPalette) -> Element {
    let more = format!("more:{row}");
    let dots = icon(&more, Icon::More, &format!("Opciones de {}", track.title), palette);
    let copy = Element::column(vec![
        Text::new(&track.title, palette).font_size(16.0).truncate().render(),
        Text::new(subtitle, palette).font_size(13.0).muted().truncate().render(),
    ]).gap(5.0).centered_y().flex(1.0);
    let body = Element::row(vec![cover(state, &track.artwork_url, 48.0, palette).radius(4.0), copy]).gap(12.0);
    let main = Element::clickable(&row, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {subtitle}", track.title))).flex(1.0);
    let row_element = Element::row(vec![main, dots.centered_y()]).gap(4.0).padding(8.0).size(None, Some(68.0));
    ContextMenu::new(format!("mobile-context:{row}:{}", track.id), row_element,
        vec![ContextMenuEntry::Item(ContextMenuItem::new(&more, "Opciones de la canción"))], SCHEME.menu())
        .open_action(more).render()
}

pub(super) fn playing(state: &State, width: f32, palette: ComponentPalette) -> Element {
    player::content(state, width, palette)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Command;
    use crate::player::PlayerView;
    use limn::{UiAction, UiTree};

    fn playing_state() -> State {
        let track = Track { id: "song".into(), title: "Una canción con un título largo para el teléfono".into(), artist: "Un artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 };
        let mut state = State::default();
        state.mobile = true;
        state.show(PlayerView { current: Some(track), playing: true, position: 30.0, duration: 180.0, ..PlayerView::default() });
        state
    }

    #[test]
    fn phone_navigation_and_transport_fit_without_desktop_panels() {
        for size in [Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            let mut state = playing_state();
            for page in [Page::Search, Page::Library, Page::Settings, Page::Playing] {
                state.page = page;
                let mut tree = UiTree::new(super::view(&state, size));
                let frame = tree.layout(size);
                assert_eq!(frame.access.iter().filter(|node| node.id == "settings").count(), usize::from(!matches!(state.page, Page::Settings | Page::Playing)));
                let ids = if state.page == Page::Playing { vec!["play-pause", "next", "previous", "queue", "mobile-connections"] } else { vec!["home", "mobile-search", "mobile-library", "play-pause", "mobile-connections"] };
                for id in ids {
                    let nodes: Vec<_> = frame.access.iter().filter(|node| node.id == id).collect();
                    assert_eq!(nodes.len(), 1, "one {id} on {:?} at {size:?}", state.page);
                    // Access nodes deduplicate IDs; hit regions also catch two live
                    // buttons sharing the same retained color/focus state.
                    assert_eq!(frame.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count(), 1,
                        "one hit target for {id} on {:?}", state.page);
                    let rect = nodes[0].rect;
                    assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= size.width + 0.01 && rect.y + rect.height <= size.height + 0.01, "{id}: {rect:?}");
                    assert!(rect.width >= TAP && rect.height >= TAP, "{id} is touch sized: {rect:?}");
                }
                assert!(!frame.regions.iter().any(|region| region.id.starts_with("resize:")));
            }
        }
    }

    #[test]
    fn navigation_labels_are_visible_and_tabs_do_not_overlap() {
        let mut state = playing_state();
        state.player.current = None;
        let mut tree = UiTree::new(super::view(&state, Size::new(390.0, 844.0)));
        let frame = tree.layout(Size::new(390.0, 844.0));
        for label in ["Inicio", "Buscar", "Biblioteca"] {
            let positions: Vec<_> = frame.commands.iter().filter_map(|command| match command {
                limn::DrawCommand::Text { text, position, .. } if text == label => Some(position),
                _ => None,
            }).collect();
            assert_eq!(positions.len(), 1, "{label}: {positions:?}");
        }
        let tabs: Vec<_> = ["home", "mobile-search", "mobile-library"].iter().map(|id| frame.access.iter().find(|node| node.id == *id).unwrap().rect).collect();
        for pair in tabs.windows(2) { assert!(pair[0].x + pair[0].width <= pair[1].x + 0.1, "{tabs:?}"); }
    }

    #[test]
    fn editing_search_reserves_the_keyboard_without_changing_playback() {
        let mut state = playing_state();
        state.mobile_search = true;
        state.mobile_ui.search_editing = true;
        state.page = Page::Search;
        for size in [Size::new(320.0, 360.0), Size::new(390.0, 500.0), Size::new(430.0, 500.0)] {
            let mut tree = UiTree::new(super::view(&state, size));
            let frame = tree.layout(size);
            assert!(!frame.access.iter().any(|node| matches!(node.id.as_str(), "home" | "mobile-library" | "play-pause")));
            for id in ["search", "mobile-search-cancel", "ios-paste:search"] {
                let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
                assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= size.width + 0.1
                    && rect.y + rect.height <= size.height + 0.1 && rect.height >= TAP, "{id}: {rect:?}");
            }
            assert!(tree.focus("search"));
            let frame = tree.layout(size);
            assert!(crate::mobile_input::can_paste(&tree, &frame, "search"));
        }
        assert!(state.player.playing);
        assert_eq!(state.current().unwrap().id, "song");
    }

    #[test]
    fn device_sheet_has_a_real_native_slot_and_closes_without_playing_a_row() {
        let mut state = playing_state();
        state.update(&UiAction::Clicked { id: "mobile-connections".into() });
        state.mobile_ui.route = Some(("Auriculares".into(), "BluetoothA2DP".into()));
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 720.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let slot = frame.access.iter().find(|node| node.id == "ios-route-picker").unwrap().rect;
            assert!(slot.width >= TAP && slot.height >= TAP && slot.x + slot.width <= width);
            assert!(frame.commands.iter().any(|command| matches!(command, limn::DrawCommand::Text { text, .. } if text == "Auriculares")));
        }
        assert!(state.update(&UiAction::Clicked { id: "mobile-connections-close".into() }).is_empty());
        assert!(!state.mobile_ui.connections && state.player.playing);
    }

    #[test]
    fn home_track_tiles_offer_the_same_hold_and_overflow_actions_as_rows() {
        let mut state = playing_state();
        let track = state.current().unwrap().clone();
        state.because.push((track.clone(), vec![track.clone()]));
        let tile = tile(&state, "rec:0:0".into(), &track.title, Some(track.artist.clone()), &None, false, SCHEME.palette());
        let frame = UiTree::new(tile).layout(Size::new(TILE_WIDTH, 240.0));
        let more = frame.access.iter().find(|node| node.id == "more:rec:0:0").unwrap();
        assert!(more.rect.width >= 44.0 && more.rect.height >= 44.0);
        assert!(frame.context_menu_at(limn::Point::new(30.0, 60.0)).is_some());
        state.update(&UiAction::Clicked { id: "more:rec:0:0".into() });
        assert_eq!(state.mobile_ui.context.as_ref().unwrap().track.id, track.id);
    }

    #[test]
    fn phone_pages_use_the_existing_search_and_playback_commands() {
        let mut state = playing_state();
        let click = |state: &mut State, id: &str| state.update(&UiAction::Clicked { id: id.into() });
        click(&mut state, "mobile-search");
        assert!(state.mobile_search && state.page == Page::Search && state.focus.is_none());
        click(&mut state, "mobile-search-edit");
        assert_eq!(state.focus.as_deref(), Some("search"));
        state.update(&UiAction::InputChanged { id: "search".into(), value: "De música ligera".into() });
        assert_eq!(click(&mut state, "search-submit"), vec![Command::Search("De música ligera".into())]);
        click(&mut state, "mobile-library");
        assert_eq!(state.page, Page::Library);
        click(&mut state, "mobile-playing");
        assert_eq!(state.page, Page::Playing);
        assert_eq!(click(&mut state, "play-pause"), vec![Command::PlayPause]);
        assert_eq!(click(&mut state, "next"), vec![Command::Next]);
        click(&mut state, "back");
        assert_eq!(state.page, Page::Library);
        assert_eq!(state.current().unwrap().id, "song", "navigation keeps the same player");
        click(&mut state, "home");
        assert!(state.page == Page::Search && !state.mobile_search);
    }

    #[test]
    fn details_scroll_beneath_fixed_chrome_and_reach_the_last_song() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 600.0);
            let mut state = playing_state();
            state.collection.playlists.push(Playlist { id: 7, name: "Mis viajes".into(), collaborative: false,
                tracks: (0..20).map(|index| Track { id: format!("song-{index}"), ..state.current().unwrap().clone() }).collect() });
            state.page = Page::Playlist(7);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let first = tree.layout(size);
            let scroll = first.regions.iter().find(|region| region.id == "mobile-playlist" && region.kind == limn::HitKind::Scroll).unwrap().rect;
            assert_eq!(scroll.y, 0.0, "the page continues behind the header");
            assert_eq!(scroll.height, size.height);
            let nav = first.access.iter().find(|node| node.id == "mobile-library").unwrap().rect;
            tree.scroll_to_end("mobile-playlist");
            let last = tree.layout(size);
            let song = last.access.iter().find(|node| node.id == "page-track:19").unwrap().rect;
            assert!(song.height >= TAP && song.y + song.height <= nav.y - MINI_HEIGHT + 0.1, "last song stays above transport: {song:?}");
            let title = limn::Point::new(width / 2.0, 36.0);
            assert_eq!(last.press_region(title).unwrap().id, "mobile-header-bar");
            tree.handle_event(limn::UiEvent::PointerDown(title));
            assert!(tree.handle_event(limn::UiEvent::PointerUp(title)).is_empty(), "a bar never plays the song below it");
            let margin = limn::Point::new(4.0, nav.y - 4.0);
            assert_eq!(last.press_region(margin).unwrap().id, "mobile-mini-bar");
            tree.handle_event(limn::UiEvent::PointerDown(margin));
            assert!(tree.handle_event(limn::UiEvent::PointerUp(margin)).is_empty());

        }
    }

}
