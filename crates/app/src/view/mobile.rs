//! Phone layout over Surco's existing pages, actions and playback state.

use super::*;

const TAP: f32 = 44.0;
const NAV_HEIGHT: f32 = 64.0;

fn icon(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
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
        Page::Search => "Surco",
        Page::Downloads => "Descargas",
        Page::Liked => "Tus me gusta",
        _ => "Tu música",
    };
    let mut header = Vec::new();
    if state.can_go_back() {
        header.push(icon("back", Icon::ArrowLeft, "Volver", palette));
    }
    header.push(Text::new(title, palette).font_size(24.0).render().centered_y().flex(1.0));
    if state.page == Page::Playing {
        header.push(icon("queue", Icon::Playlist, if state.queue_open { "Volver a la canción" } else { "Ver cola" }, palette));
    }
    if state.page != Page::Settings {
        header.push(icon("settings", Icon::Settings, "Configuración", palette));
    }
    let header = Element::row(header).gap(8.0).padding(16.0);

    let mut parts = vec![header];
    if state.page == Page::Search {
        let field = SearchField::new("search", &state.query, "Canciones, artistas o un enlace", SCHEME.on_raised())
            .size(ControlSize::Spacious).render().size(None, Some(TAP)).flex(1.0);
        let submit = icon("search-submit", Icon::Search, "Buscar música", palette);
        parts.push(Element::row(vec![field, submit]).gap(8.0).padding(16.0));
    }
    let content = if state.page == Page::Playing && state.queue_open {
        upcoming(state, palette)
    } else if searching && state.submitted.is_none() && !state.searching {
        centered(Empty::new("Encontrá tu próxima canción", "Buscá un tema o artista, o pegá el enlace de una playlist.", palette).icon(Icon::Search).render())
    } else {
        pane(state, window.width, palette)
    };
    parts.push(content.flex(1.0));
    if state.page != Page::Playing {
        if let Some(track) = state.current() {
            parts.push(mini_player(state, track, palette));
        }
    }
    parts.push(navigation(state, palette));
    let tree = Element::panel(Element::column(parts)).background(SCHEME.canvas).size(Some(window.width), Some(window.height));
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
        icon("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" }, palette),
        icon("next", Icon::SkipForward, "Siguiente canción", palette),
    ]).gap(8.0).padding(8.0);
    Element::panel(controls).background(SCHEME.raised).radius(PANEL_RADIUS).padding(8.0)
}

pub(super) fn playing(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(Empty::new("Todavía no hay música", "Elegí una canción para empezar a escuchar.", palette).icon(Icon::MusicNote).render());
    };
    let side = (width - 64.0).clamp(160.0, 300.0);
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
    let mut body = vec![cover(state, &track.artwork_url, side, palette).centered_x(), details, Element::column(vec![seek, time]), controls,
        Button::new("lyrics", "Ver letra", palette).icon(Icon::Custom(LYRICS)).variant(ButtonVariant::Outline).size(ControlSize::Spacious).render().size(None, Some(TAP)),
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
                assert_eq!(frame.access.iter().filter(|node| node.id == "settings").count(), usize::from(state.page != Page::Settings));
                for id in ["home", "mobile-search", "mobile-library", "play-pause", "next"] {
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
    fn phone_pages_use_the_existing_search_and_playback_commands() {
        let mut state = playing_state();
        let click = |state: &mut State, id: &str| state.update(&UiAction::Clicked { id: id.into() });
        click(&mut state, "mobile-search");
        assert!(state.mobile_search && state.page == Page::Search && state.focus.as_deref() == Some("search"));
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
