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
#[cfg(test)]
mod native_ui_geometry_tests;
#[cfg(test)]
mod edge_chrome_tests;

const TAP: f32 = 44.0;
const NAV_HEIGHT: f32 = 52.0;
const HEADER_HEIGHT: f32 = MOBILE_HEADER_HEIGHT;
const MINI_HEIGHT: f32 = 64.0;
pub(super) const SCROLL_INDICATOR: limn::ScrollbarVisibility = limn::ScrollbarVisibility::AutoHide { hold_ms: 650, fade_ms: 220 };

pub(super) fn icon(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    IconButton::new(id, glyph, palette).variant(ButtonVariant::Ghost)
        .accessibility_label(label).size(ControlSize::Spacious).render().size(Some(TAP), Some(TAP))
}

pub(super) fn view(state: &State, window: Size) -> Element {
    let palette = SCHEME.palette();
    let insets = state.mobile_ui.content_insets;
    let width = (window.width - insets.left - insets.right).max(TAP);
    let surface = page_surface(state);
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
        Page::Album(album) if state.mobile_ui.compact_header => &album.title,
        Page::Artist(artist) if state.mobile_ui.compact_header => &artist.name,
        Page::Playlist(id) if state.mobile_ui.compact_header => state.collection.playlist(*id).map_or("Playlist", |playlist| playlist.name.as_str()),
        _ => "Tu música",
    };
    let mut header = Vec::new();
    if state.can_go_back() || state.page == Page::Playing {
        header.push(icon(if state.page == Page::Playing && !state.queue_open { "mobile-player-dismiss" } else if state.can_go_back() { "back" } else { "home" }, if state.page == Page::Playing { Icon::ChevronDown } else { Icon::ArrowLeft }, "Volver", palette));
    }
    let compact_player = state.page == Page::Playing && !state.queue_open && state.mobile_ui.compact_header;
    let copy = if compact_player {
        state.current().map_or_else(|| Text::new(title, palette).render(), |track| {
            Element::column(vec![Text::new(&track.title, palette).font_size(17.0).truncate().render(),
                Text::new(&track.artist, palette).font_size(12.0).muted().truncate().render()]).gap(4.0)
        })
    } else { Text::new(title, palette).font_size(if state.page == Page::Playing { 14.0 } else if state.mobile_ui.compact_header { 18.0 } else { 24.0 }).truncate().render() };
    header.push(if state.page == Page::Playing && !compact_player { copy.centered().flex(1.0) } else { copy.centered_y().flex(1.0) });
    if compact_player {
        if let Some(track) = state.current() {
            header.push(icon("mobile-player-header-like", if state.is_liked(&track.id) { Icon::HeartFill } else { Icon::Heart }, if state.is_liked(&track.id) { "Quitar de Tus me gusta" } else { "Me gusta" }, palette).dispatch_as("like"));
        }
        header.push(icon("mobile-player-header-play", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" }, palette).dispatch_as("play-pause"));
    } else if state.mobile_ui.compact_header && matches!(state.page, Page::Album(_) | Page::Artist(_) | Page::Playlist(_)) && !state.page_rows().is_empty() {
        header.push(icon("mobile-detail-header-play", Icon::PlayFill, "Reproducir", palette).dispatch_as("page-play"));
    }
    if state.page == Page::Playing && state.queue_open {
        header.push(icon("queue", Icon::Playlist, if state.queue_open { "Volver a la canción" } else { "Ver cola" }, palette));
    } else if state.page == Page::Playing && !compact_player && state.current().is_some() {
        header.push(icon("mobile-player-header-more", Icon::More, "Opciones de la canción", palette).dispatch_as("more:current"));
    }
    if state.page != Page::Settings && state.page != Page::Playing && !matches!(state.page, Page::Album(_) | Page::Artist(_) | Page::Playlist(_)) {
        header.push(icon("settings", Icon::Settings, "Configuración", palette));
    }
    let editing = searching && state.mobile_ui.search_editing;
    let keyboard = editing || state.mobile_ui.keyboard_visible;
    let header_height = if searching { state.mobile_ui.search_header.unwrap_or(if editing { 0.0 } else { HEADER_HEIGHT }) } else { HEADER_HEIGHT };
    let header = horizontal_inset(Element::row(header).gap(8.0).padding(6.0), 10.0, 10.0)
        .size(None, Some(header_height)).clip_children();
    let header = if compact_player {
        Element::stack(vec![header, Element::column(vec![Element::spacer().flex(1.0),
            Progress::new(state.progress(), palette).height(2.0).render()])])
            .size(None, Some(header_height)).clip_children()
    } else { header };
    let mut top = vec![Element::spacer().size(None, Some(insets.top)), header];
    let mut top_height = insets.top + header_height;
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
    let mut content = if state.page == Page::Playing && state.queue_open {
        upcoming(state, palette)
    } else if searching {
        search::content(state, width, palette)
    } else if state.page == Page::Search {
        home::content(state, width, palette)
    } else {
        page(state, width, palette)
    };
    if state.page == Page::Lyrics {
        // The mobile shell owns the full-height song color, including plain/error states.
        content.style.bottom_background_fade = None;
    }
    let mut bottom = Vec::new();
    let mut bottom_height = insets.bottom;
    if state.page != Page::Playing && !keyboard {
        if let Some(track) = state.current() {
            bottom.push(mini_player(state, track, palette));
            bottom_height += MINI_HEIGHT;
        }
    }
    if state.page != Page::Playing && !keyboard { bottom.push(navigation(state, palette)); bottom_height += NAV_HEIGHT; }
    bottom.push(Element::spacer().size(None, Some(insets.bottom)));
    let content = if state.page == Page::Playing && !state.queue_open { content } else { content.animate_enter(route_key(state)) };
    let header_edge = mix(surface.0, surface.1, (top_height / window.height.max(1.0)).clamp(0.0, 1.0));
    let footer_edge = if matches!(state.page, Page::Playing | Page::Lyrics) { surface.1 } else { SCHEME.canvas };
    let content = chrome::under_bars(content, top_height, bottom_height, header_edge, footer_edge);
    let top = Element::column(top).vertical_gradient(vec![surface.0, header_edge]).block_pointer("mobile-header-bar");
    let footer = Element::column(bottom);
    let footer = if state.page != Page::Playing && !keyboard { footer.background(footer_edge) } else { footer };
    let bars = Element::column(vec![top, Element::spacer().flex(1.0), footer]);
    let content = Element::stack(vec![content, bars]);
    notices.push(content.flex(1.0));
    let content = horizontal_inset(Element::column(notices), insets.left, insets.right);
    let tree = Element::panel(content).background(SCHEME.canvas).vertical_gradient(vec![surface.0, surface.1])
        .size(Some(window.width), Some(window.height));
    let tree = if state.page == Page::Playing { tree.translate(limn::Point::new(0.0, state.mobile_ui.player_offset)) } else { tree };
    let tree = Element::panel(tree).size(Some(window.width), Some(window.height)).clip_children();
    // UIKit owns these surfaces when its bridge is ready. Keep the limn
    // implementation available on other platforms or if the bridge is unavailable.
    if cfg!(target_os = "ios") && state.mobile_ui.native_surfaces { return tree; }
    let tree = sheets::render(state, tree, palette);
    let tree = share_dialog(state, tree, window.width, palette);
    let tree = playlist_dialog(state, tree, window.width, palette);
    let tree = missed_dialog(state, import_dialog(state, tree, window.width, palette), window.width, palette);
    let tree = confirm_dialog(state, tree, palette);
    limn::BottomSheet::new(tree, state.equalizer_open, "Ecualizador", "Ajustá el sonido de este equipo.", "equalizer-close", palette)
        .max_height(680.0).body(equalizer(state, palette)).render()
}

fn horizontal_inset(content: Element, left: f32, right: f32) -> Element {
    Element::row(vec![Element::spacer().size(Some(left), None), content.flex(1.0),
        Element::spacer().size(Some(right), None)])
}

fn mix(from: Color, to: Color, amount: f32) -> Color {
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color::rgb(channel(from.r, to.r), channel(from.g, to.g), channel(from.b, to.b))
}

/// Keep even a very bright cover readable with Surco's secondary text token.
fn cover_tone(tint: Color, amount: f32) -> Color {
    let luminance = |color: Color| {
        let linear = |channel: u8| {
            let s = f32::from(channel) / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    };
    let mut color = mix(SCHEME.canvas, tint, amount);
    for _ in 0..16 {
        if (luminance(SCHEME.muted) + 0.05) / (luminance(color) + 0.05) >= 4.5 { break; }
        color = mix(color, SCHEME.canvas, 0.12);
    }
    color
}

fn page_surface(state: &State) -> (Color, Color) {
    let artwork = match &state.page {
        Page::Playing | Page::Lyrics => state.current().and_then(|track| track.artwork_url.as_deref()),
        Page::Album(album) => album.artwork_url.as_deref(),
        Page::Artist(artist) => artist.artwork_url.as_deref(),
        Page::Playlist(id) => state.collection.playlist(*id).and_then(|playlist| playlist.tracks.first()).and_then(|track| track.artwork_url.as_deref()),
        _ => None,
    };
    let Some(tint) = artwork.and_then(|url| state.tints.get(url)).copied() else { return (SCHEME.canvas, SCHEME.canvas); };
    let bottom = if matches!(state.page, Page::Playing | Page::Lyrics) { cover_tone(tint, 0.28) } else { SCHEME.canvas };
    (cover_tone(tint, 0.60), bottom)
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
            Element::icon(glyph).large_icon(22.0).foreground(ink).centered_x().animate_foreground(format!("mobile-tab:{id}:icon")),
            Text::new(label, palette).font_size(11.0).render().foreground(ink).centered_x().animate_foreground(format!("mobile-tab:{id}:label")),
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
    let tint = track.artwork_url.as_deref().and_then(|url| state.tints.get(url)).copied();
    let surface = tint.map_or(SCHEME.raised, |tint| cover_tone(tint, 0.32));
    let label = Element::row(vec![
        cover(state, &track.artwork_url, 40.0, palette).radius(4.0).centered_y(),
        Element::column(vec![
            Text::new(&track.title, palette).font_size(13.0).truncate().render(),
            Text::new(state.player.error.as_deref().unwrap_or(&track.artist), palette).font_size(11.0).muted().truncate().render(),
        ]).gap(3.0).centered_y().flex(1.0),
    ]).gap(10.0).size(None, Some(TAP));
    let open = Element::clickable("mobile-playing", label)
        .accessible(limn::Access::new(limn::Role::Button).name(format!("Abrir reproductor: {}", track.title))).flex(1.0);
    let controls = Element::row(vec![
        open,
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), "Conectar a un dispositivo", palette),
        icon("mobile-mini-like", if state.is_liked(&track.id) { Icon::HeartFill } else { Icon::Heart },
            if state.is_liked(&track.id) { "Quitar de Tus me gusta" } else { "Me gusta" }, palette).dispatch_as("like"),
        icon("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" }, palette),
    ]).gap(0.0).padding(4.0);
    let progress = Progress::new(state.progress(), palette).height(2.0).render();
    let card = Element::panel(Element::column(vec![controls, progress])).background(surface).radius(8.0).clip_children();
    Element::column(vec![Element::spacer().size(None, Some(4.0)), horizontal_inset(card, 8.0, 8.0),
        Element::spacer().size(None, Some(6.0))]).size(None, Some(MINI_HEIGHT)).block_pointer("mobile-mini-bar")
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
    use surco_core::ArtistRef;

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
    fn the_player_scroll_edge_uses_its_header_tint_instead_of_a_black_band() {
        let mut state = playing_state();
        state.page = Page::Playing;
        state.player.current.as_mut().unwrap().artwork_url = Some("cover".into());
        state.tints.insert("cover".into(), Color::rgb(200, 100, 50));
        let size = Size::new(390.0, 700.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        tree.handle_event(limn::UiEvent::ScrollDirect { position: limn::Point::new(20.0, 400.0), delta_y: -100.0 });
        let frame = tree.layout(size);
        let edge = frame.commands.iter().find_map(|command| match command {
            limn::DrawCommand::FillVerticalGradient { rect, top, bottom }
                if rect.y == HEADER_HEIGHT && rect.height == 20.0 && bottom.a == 0 => Some(*top),
            _ => None,
        }).expect("the player's top edge is visible after scrolling");
        assert_eq!(edge.a, 255, "the edge masks the song content beneath it");
        assert!(edge.r > edge.g && edge.g > edge.b && edge.b > SCHEME.canvas.b,
            "the edge joins the warm header wash: {edge:?}");
    }

    #[test]
    fn library_import_card_actions_have_phone_sized_touch_targets() {
        let mut state = playing_state();
        state.page = Page::Library;
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            for id in ["import-open", "import-dismiss"] {
                let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
                assert!(rect.width >= TAP && rect.height >= TAP, "{id}: {rect:?}");
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width && rect.y + rect.height <= size.height - NAV_HEIGHT - MINI_HEIGHT);
            }
        }
    }

    #[test]
    fn library_import_card_reserves_a_native_paste_control_without_replacing_import() {
        let mut state = playing_state();
        state.page = Page::Library;
        state.mobile_ui.native_surfaces = true;
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let paste = frame.access.iter().find(|node| node.id == "ios-paste:import-card").unwrap();
            let import = frame.access.iter().find(|node| node.id == "import-open").unwrap();
            assert_eq!(paste.role, limn::Role::Group);
            assert!(paste.rect.width >= 112.0 && paste.rect.height >= TAP);
            assert!(paste.rect.x >= import.rect.x + import.rect.width);
            assert!(paste.rect.x + paste.rect.width <= width);
            assert!(paste.rect.y + paste.rect.height <= size.height - NAV_HEIGHT - MINI_HEIGHT);
        }
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
    fn home_track_tiles_keep_the_cover_clear_and_hold_the_correct_song() {
        let mut state = playing_state();
        let track = state.current().unwrap().clone();
        state.because.push((track.clone(), vec![track.clone()]));
        let tile = tile(&state, "rec:0:0".into(), &track.title, Some(track.artist.clone()), &None, false, SCHEME.palette());
        let mut tree = UiTree::new(tile);
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(TILE_WIDTH, 240.0));
        assert!(!frame.access.iter().any(|node| node.id.starts_with("more:")),
            "the artwork has no permanent overflow button");
        let point = limn::Point::new(30.0, 60.0);
        assert_eq!(frame.context_menu_at(point), Some("tile-context:rec:0:0:song"));
        let actions = tree.handle_event(limn::UiEvent::ContextMenu { position: point });
        assert_eq!(actions, vec![UiAction::Clicked { id: "more:rec:0:0".into() }]);
        state.update(&actions[0]);
        assert_eq!(state.mobile_ui.context.as_ref().unwrap().track.id, track.id);
    }

    #[test]
    fn home_album_and_artist_cards_offer_hold_actions_in_quick_access_and_shelves() {
        let mut state = playing_state();
        state.page = Page::Search;
        state.collection.recent = vec![
            Entry::Album(AlbumRef { id: "album".into(), title: "Un álbum".into(), artist: Some("Una banda".into()), year: String::new(), artwork_url: None }),
            Entry::Artist(ArtistRef { id: "artist".into(), name: "Una artista".into(), subscribers: None, artwork_url: None }),
        ];
        state.collection.kept = state.collection.recent.clone();
        for index in 0..2 {
            let row = format!("recent:{index}");
            let size = Size::new(390.0, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let rect = frame.access.iter().find(|node| node.id == row).unwrap().rect;
            let actions = tree.handle_event(limn::UiEvent::ContextMenu {
                position: limn::Point::new(rect.x + 20.0, rect.y + 20.0),
            });
            assert_eq!(actions, vec![UiAction::Clicked { id: format!("entry-more:{row}") }]);
            let entry = &state.collection.kept[index];
            let row = format!("home-kept:{index}");
            let mut tree = UiTree::new(tile(&state, row.clone(), entry.title(), None, entry.artwork(), false, SCHEME.palette()));
            tree.set_reduced_motion(true);
            tree.layout(Size::new(TILE_WIDTH, 240.0));
            assert_eq!(tree.handle_event(limn::UiEvent::ContextMenu { position: limn::Point::new(20.0, 60.0) }),
                vec![UiAction::Clicked { id: format!("entry-more:{row}") }]);
        }
    }

    #[test]
    fn phone_home_shelves_start_without_a_permanent_scroll_indicator() {
        let mut state = playing_state();
        state.collection.heard = (0..6).map(|index| Track { id: format!("song-{index}"), ..state.current().unwrap().clone() }).collect();
        let size = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        assert!(frame.regions.iter().any(|region| region.id == "home-heard" && region.kind == limn::HitKind::Scroll));
        assert!(!frame.commands.iter().any(|command| matches!(command,
            limn::DrawCommand::FillRect { rect, radius, .. } if rect.height == 3.0 && *radius == 1.5)),
            "the horizontal indicator stays hidden until interaction");
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
            state.mobile_ui.compact_header = true;
            tree.set_root(super::view(&state, size));
            let compact = tree.layout(size);
            let id = "mobile-detail-header-play";
            assert_eq!(compact.regions.iter().filter(|region| region.id == id).count(), 1);
            let button = compact.access.iter().find(|node| node.id == id).unwrap().rect;
            let point = limn::Point::new(button.x + 20.0, button.y + 20.0);
            tree.handle_event(limn::UiEvent::PointerDown(point));
            assert_eq!(tree.handle_event(limn::UiEvent::PointerUp(point)), vec![UiAction::Clicked { id: "page-play".into() }]);
        }
    }

    #[test]
    fn player_translation_moves_header_and_hits_together_without_duplicate_controls() {
        let mut state = playing_state();
        state.page = Page::Playing;
        let size = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let first = tree.layout(size);
        let original = first.access.iter().find(|node| node.id == "mobile-player-dismiss").unwrap().rect;
        state.mobile_ui.player_offset = 100.0;
        state.mobile_ui.compact_header = true;
        state.collection.liked.push(state.current().unwrap().clone());
        tree.set_root(super::view(&state, size));
        let moved = tree.layout(size);
        let close = moved.access.iter().find(|node| node.id == "mobile-player-dismiss").unwrap().rect;
        assert_eq!(close.y, original.y + 100.0);
        assert_ne!(moved.hit_test(limn::Point::new(original.x + 20.0, original.y + 20.0)), Some("mobile-player-dismiss"));
        assert_eq!(moved.hit_test(limn::Point::new(close.x + 20.0, close.y + 20.0)), Some("mobile-player-dismiss"));
        for id in ["mobile-player-header-play", "mobile-player-header-like", "play-pause", "like"] {
            assert!(moved.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count() <= 1);
        }
        for id in ["mobile-player-header-play", "mobile-player-header-like"] {
            let control = moved.access.iter().find(|node| node.id == id).unwrap().rect;
            assert!(control.width >= TAP && control.height >= TAP && control.y >= 100.0 && control.y + control.height <= 176.0);
        }
        assert_eq!(moved.access.iter().find(|node| node.id == "mobile-player-header-like").unwrap().name, "Quitar de Tus me gusta");
        assert!(state.player.playing);
    }
}
