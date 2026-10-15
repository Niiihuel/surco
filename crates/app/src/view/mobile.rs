//! Phone layout over Surco's existing pages, actions and playback state.

use super::*;

mod search;
mod home;
mod sheets;
pub(super) mod details;

const TAP: f32 = 44.0;
const NAV_HEIGHT: f32 = 64.0;

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
    header.push(Text::new(title, palette).font_size(24.0).render().centered_y().flex(1.0));
    if state.page == Page::Playing && state.queue_open {
        header.push(icon("queue", Icon::Playlist, if state.queue_open { "Volver a la canción" } else { "Ver cola" }, palette));
    }
    if state.page != Page::Settings && state.page != Page::Playing {
        header.push(icon("settings", Icon::Settings, "Configuración", palette));
    }
    let header = Element::row(header).gap(8.0).padding(16.0);

    let editing = searching && state.mobile_ui.search_editing;
    let mut parts = if editing { Vec::new() } else { vec![header] };
    if searching { parts.push(search::field(state, palette)); }
    if let Some(notice) = &state.mobile_ui.notice {
        parts.push(Element::panel(Element::row(vec![Text::new(notice, palette).render().flex(1.0),
            icon("mobile-notice-close", Icon::Close, "Cerrar aviso", palette)]).gap(8.0).padding(12.0)).background(SCHEME.raised));
    }
    let content = if state.page == Page::Playing && state.queue_open {
        upcoming(state, palette)
    } else if searching {
        search::content(state, window.width, palette)
    } else if state.page == Page::Search {
        home::content(state, window.width, palette)
    } else {
        pane(state, window.width, palette)
    };
    parts.push(content.flex(1.0));
    if state.page != Page::Playing && !editing {
        if let Some(track) = state.current() {
            parts.push(mini_player(state, track, palette));
        }
    }
    if state.page != Page::Playing && !editing { parts.push(navigation(state, palette)); }
    let face = if state.page == Page::Search { if searching { "mobile:search" } else { "mobile:home" } }
        else if state.page == Page::Playing { if state.queue_open { "mobile:queue" } else { "mobile:player" } }
        else { "mobile:page" };
    let content = Element::column(parts).animate_enter(face);
    let content = if state.page == Page::Playing { state.current().map_or(content.clone(), |track| washed(state, &track.artwork_url, content)) } else { content };
    let tree = Element::panel(content).background(SCHEME.canvas).size(Some(window.width), Some(window.height));
    let tree = sheets::render(state, tree, palette);
    let tree = share_dialog(state, tree, window.width, palette);
    let tree = playlist_dialog(state, tree, window.width, palette);
    let tree = missed_dialog(state, import_dialog(state, tree, window.width, palette), window.width, palette);
    let tree = confirm_dialog(state, tree, palette);
    // Settings opens the same equalizer as desktop without squeezing it into a footer.
    Modal::new(tree, state.equalizer_open, "Ecualizador", "Ajustá el sonido de este equipo.", "equalizer-close", palette)
        .width(Modal::fit_width(window.width, 360.0))
        .body(equalizer(state, palette)).render()
}

fn navigation(state: &State, palette: ComponentPalette) -> Element {
    let tab = |id: &str, glyph: Icon, label: &str, selected: bool| {
        let ink = if selected { palette.text } else { palette.muted };
        let copy = Element::column(vec![
            Element::icon(glyph).foreground(ink).centered_x(),
            Text::new(label, palette).role(TextRole::Caption).render().foreground(ink).centered_x(),
        ]).gap(4.0).centered();
        Element::clickable(id, copy).accessible(limn::Access::new(limn::Role::Button).name(label).selected(selected))
            .size(None, Some(NAV_HEIGHT)).flex(1.0)
    };
    Element::row(vec![
        tab("home", Icon::Home, "Inicio", state.page == Page::Search && !state.mobile_search),
        tab("mobile-search", Icon::Search, "Buscar", state.page == Page::Search && state.mobile_search),
        tab("mobile-library", Icon::Playlist, "Biblioteca", matches!(state.page, Page::Library | Page::Downloads | Page::Liked | Page::Playlist(_))),
    ]).background(CANVAS)
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
    Element::panel(Element::column(vec![controls, progress])).background(SCHEME.raised).radius(PANEL_RADIUS).padding(8.0)
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
    ContextMenu::new(format!("mobile-context:{row}"), row_element,
        vec![ContextMenuEntry::Item(ContextMenuItem::new(&more, "Opciones de la canción"))], SCHEME.menu())
        .open_action(more).render()
}

pub(super) fn playing(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(Empty::new("Todavía no hay música", "Elegí una canción para empezar a escuchar.", palette).icon(Icon::MusicNote).render());
    };
    let side = (width - 48.0).clamp(160.0, 380.0);
    let time = Element::row(vec![
        Text::new(clock(f64::from(state.progress()) * state.player.duration), palette).role(TextRole::Caption).muted().render(),
        Element::spacer().flex(1.0),
        Text::new(clock(state.player.duration), palette).role(TextRole::Caption).muted().render(),
    ]);
    let seek = Slider::new("seek", state.progress(), palette).enabled(state.player.duration > 0.0).render().size(None, Some(TAP));
    let play_palette = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let play = IconButton::new("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, play_palette)
        .variant(ButtonVariant::Subtle).round().accessibility_label(if state.player.playing { "Pausar" } else { "Reproducir" })
        .render().size(Some(64.0), Some(64.0)).centered_y();
    let controls = Element::row(vec![
        icon("shuffle", shuffle_step(state).0, shuffle_step(state).1, palette).centered_y(),
        icon("previous", Icon::SkipBack, "Canción anterior", palette).centered_y(),
        play,
        icon("next", Icon::SkipForward, "Siguiente canción", palette).centered_y(),
        icon("repeat", if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, "Repetir", palette).centered_y(),
    ]).gap(8.0).centered_x();
    let liked = state.is_liked(&track.id);
    let details = Element::row(vec![
        Element::column(vec![Text::new(&track.title, palette).font_size(24.0).truncate().render(), Text::new(&track.artist, palette).role(TextRole::Body).muted().truncate().render()]).gap(8.0).flex(1.0),
        icon("like", if liked { Icon::HeartFill } else { Icon::Heart }, if liked { "Quitar de Tus me gusta" } else { "Me gusta" }, palette).centered_y(),
    ]).gap(16.0);
    let tools = Element::row(vec![
        icon("mobile-connections", Icon::Audio, "Conectar a un dispositivo", palette),
        Element::spacer().flex(1.0),
        icon("more:current", Icon::More, "Opciones de la canción", palette),
        icon("queue", Icon::Playlist, "Ver cola", palette),
    ]);
    let lyric_card = Element::panel(Element::column(vec![
        Element::row(vec![Text::new("Letra", palette).role(TextRole::Heading).render().flex(1.0), Element::icon(Icon::ArrowRight).foreground(palette.text).centered_y()]),
        Text::new("Seguí cada palabra mientras escuchás.", palette).muted().render(),
    ]).gap(12.0).padding(20.0)).background(SCHEME.raised).radius(12.0);
    let mut body = vec![cover(state, &track.artwork_url, side, palette).radius(8.0).centered_x(), details, Element::column(vec![seek, time]), controls, tools,
        Element::clickable("lyrics", lyric_card).accessible(limn::Access::new(limn::Role::Button).name("Ver letra")),
    ];
    if let Some(error) = &state.player.error {
        body.push(Alert::new("No se pudo reproducir", error, palette).id("mobile-player-error").render());
    }
    ScrollArea::new("mobile-playing", Element::column(body).gap(24.0).padding(24.0)).render().flex(1.0)
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
        for size in [Size::new(320.0, 360.0), Size::new(390.0, 500.0)] {
            let frame = UiTree::new(super::view(&state, size)).layout(size);
            assert!(!frame.access.iter().any(|node| matches!(node.id.as_str(), "home" | "mobile-library" | "play-pause")));
            for id in ["search", "mobile-search-cancel"] {
                let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
                assert!(rect.x >= 0.0 && rect.x + rect.width <= size.width + 0.1 && rect.height >= TAP, "{id}: {rect:?}");
            }
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
}
