//! Phone player content. The mobile shell owns its header and dismissal gesture.

use super::*;

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(Empty::new("Todavía no hay música", "Elegí una canción para empezar a escuchar.", palette)
            .icon(Icon::MusicNote).render());
    };
    let side = (width - 48.0).clamp(160.0, 380.0);
    let liked = state.is_liked(&track.id);
    let details = Element::row(vec![
        Element::column(vec![
            Text::new(&track.title, palette).font_size(26.0).truncate().render(),
            Text::new(&track.artist, palette).font_size(16.0).muted().truncate().render(),
        ]).gap(6.0).flex(1.0).centered_y(),
        icon("like", if liked { Icon::HeartFill } else { Icon::Heart },
            if liked { "Quitar de Tus me gusta" } else { "Me gusta" }, palette).centered_y(),
    ]).gap(12.0).anchor("mobile-player-title");
    let mut controls = vec![details];
    if let Some(error) = &state.player.error {
        controls.push(Alert::new("No se pudo reproducir", error, palette).id("mobile-player-error").render());
    }
    controls.extend([timeline(state, palette), transport(state, palette), tools(palette)]);
    let main = Element::column(vec![
        cover(state, &track.artwork_url, side, palette).radius(8.0).centered_x()
            .animate_enter(format!("mobile-player-cover:{}", track.id)),
        Element::column(controls).gap(12.0),
    ]).gap(32.0);
    let mut body = vec![main, lyric_preview(state, palette)];
    if let Some(artist) = artist_card(state, width - 48.0, palette) {
        body.push(artist);
    }
    ScrollArea::new("mobile-playing", Element::column(body).gap(24.0).padding(24.0))
        .appearance(limn::ScrollbarAppearance::overlay().buttons(false).rail_width(4.0).thumb_width(2.0))
        .render().flex(1.0)
}

fn timeline(state: &State, palette: ComponentPalette) -> Element {
    let duration = state.player.duration.max(0.0);
    let progress = state.progress();
    let elapsed = if duration > 0.0 { f64::from(progress) * duration } else { state.player.position.max(0.0) };
    // Ceiling keeps the countdown consistent with the elapsed clock's whole seconds,
    // including the tiny rounding error introduced by the slider's f32 fraction.
    let remaining = if duration > 0.0 { format!("-{}", clock((duration - elapsed).max(0.0).ceil())) } else { "–:––".into() };
    let time = |value: String| Text::new(value, palette).mono().font_size(12.0).muted().render();
    Element::column(vec![
        Slider::new("seek", progress, palette).enabled(duration > 0.0)
            .accessibility_label("Posición de reproducción").render().size(None, Some(TAP)),
        Element::row(vec![time(clock(elapsed)), Element::spacer().flex(1.0), time(remaining)]),
    ])
}

fn transport(state: &State, palette: ComponentPalette) -> Element {
    let control = |id: &str, glyph, label: &str, glyph_size, active| {
        let action = controls::action(id, glyph, label, glyph_size, palette);
        if active { action.background(palette.selected) } else { action }.centered_y()
    };
    let play = controls::playback("play-pause", state.player.playing, 72.0, 32.0, palette);
    Element::row(vec![
        control("shuffle", shuffle_step(state).0, shuffle_step(state).1, 24.0, state.player.shuffle),
        Element::spacer().flex(1.0),
        control("previous", Icon::SkipBack, "Canción anterior", 28.0, false),
        Element::spacer().flex(1.0), play, Element::spacer().flex(1.0),
        control("next", Icon::SkipForward, "Siguiente canción", 28.0, false),
        Element::spacer().flex(1.0),
        control("repeat", if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, "Repetir", 24.0, state.player.repeat != Repeat::Off),
    ])
}

fn tools(palette: ComponentPalette) -> Element {
    Element::row(vec![
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), "Conectar a un dispositivo", palette),
        Element::spacer().flex(1.0),
        icon("do:share:current", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), "Compartir canción", palette),
        icon("queue", Icon::Playlist, "Ver cola", palette),
        icon("more:current", Icon::More, "Opciones de la canción", palette),
    ]).gap(8.0)
}

fn lyric_preview(state: &State, palette: ComponentPalette) -> Element {
    let card = ComponentPalette { surface: SCHEME.raised, ..palette };
    let mut parts = vec![Text::new("Vista previa de la letra", card).font_size(16.0).render()];
    let lines = state.lines();
    if !lines.is_empty() {
        let active = state.active_line();
        let from = active.unwrap_or(0).saturating_sub(1).min(lines.len().saturating_sub(4));
        parts.extend(lines.iter().enumerate().skip(from).take(4).map(|(index, line)| {
            let text = Text::new(if line.text.trim().is_empty() { "♪" } else { &line.text }, card)
                .font_size(22.0).line_height(30.0);
            if active == Some(index) { text } else { text.muted() }.render()
        }));
    } else if !state.plain_lyrics().is_empty() {
        parts.extend(state.plain_lyrics().iter().filter(|line| !line.is_empty()).take(4)
            .map(|line| Text::new(line, card).font_size(22.0).line_height(30.0).render()));
        let source = match &state.lyrics { Some((_, Lyrics::Plain { source, .. })) => source.as_str(), _ => "" };
        parts.push(Text::new(format!("Letra sin sincronización · {source}"), card).font_size(14.0).muted().render());
    } else {
        let current = state.current().map(|track| track.id.as_str());
        let known = state.lyrics.as_ref().filter(|(id, _)| Some(id.as_str()) == current);
        let message = match known {
            Some((_, Lyrics::Loading)) => "Buscando la letra…",
            Some((_, Lyrics::Missing | Lyrics::Lines(_) | Lyrics::Plain { .. })) => "No hay letra disponible.",
            Some((_, Lyrics::Failed { message, .. })) => message,
            None => "Abrí la letra para buscarla.",
        };
        parts.push(Text::new(message, card).font_size(16.0).muted().render());
        if matches!(known, Some((_, Lyrics::Failed { .. } | Lyrics::Missing))) {
            parts.push(Button::new("retry-lyrics", "Reintentar", card).variant(ButtonVariant::Outline)
                .size(ControlSize::Spacious).render().size(None, Some(TAP)));
        }
    }
    parts.push(Element::row(vec![
        Button::new("lyrics", "Mostrar la letra", card).variant(ButtonVariant::Outline)
            .size(ControlSize::Spacious).render().size(None, Some(TAP)),
        Element::spacer().flex(1.0),
    ]));
    Element::panel(Element::column(parts).gap(18.0).padding(20.0)).background(card.surface).radius(16.0)
}

fn artist_card(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let (artist, biography) = state.about_artist()?;
    let card = ComponentPalette { surface: SCHEME.raised, ..palette };
    let followed = state.is_followed(artist);
    let name = Element::clickable("about-artist", Text::new(&artist.name, card).font_size(20.0).truncate().render().centered_y())
        .accessible(limn::Access::new(limn::Role::Button).name(format!("Ver artista: {}", artist.name)))
        .size(None, Some(TAP)).flex(1.0);
    let follow = Button::new("about-follow", if followed { "Siguiendo" } else { "Seguir" }, card)
        .variant(ButtonVariant::Outline).size(ControlSize::Spacious).render().size(None, Some(TAP));
    let mut copy = vec![
        Text::new("Acerca del artista", card).font_size(16.0).render(),
        Element::row(vec![name, follow]).gap(12.0),
    ];
    if let Some(subscribers) = &artist.subscribers {
        copy.push(Text::new(format!("{subscribers} suscriptores"), card).muted().render());
    }
    if let Some(biography) = biography.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        let excerpt = match biography.char_indices().nth(ABOUT_WORDS) {
            Some((end, _)) => format!("{}…", biography[..end].rsplit_once(char::is_whitespace).map_or(&biography[..end], |(words, _)| words)),
            None => biography.to_owned(),
        };
        copy.push(Text::new(excerpt, card).role(TextRole::Body).muted().render());
    }
    let mut parts = Vec::new();
    if let Some(image) = artist.artwork_url.as_ref().and_then(|url| state.artwork.get(url)) {
        parts.push(Element::raster_image(image.clone().cover()).size(Some(width), Some(180.0)));
    }
    parts.push(Element::column(copy).gap(12.0).padding(20.0));
    Some(Element::panel(Element::column(parts)).background(card.surface).radius(16.0)
        .corner_mask(palette.surface).size(Some(width), None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{DrawCommand, Frame, UiAction, UiTree};
    use surco_core::lyrics::Line;

    fn state() -> State {
        let track = Track {
            id: "song".into(), title: "Una canción con un título largo y acentos para el teléfono".into(),
            artist: "Un artista".into(), artist_id: Some("artist".into()),
            album: None, album_id: None, artwork_url: None, duration_ms: 180_000,
        };
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Playing;
        state.show(PlayerView { current: Some(track), playing: true, position: 30.0, duration: 180.0, ..PlayerView::default() });
        state
    }

    fn frame(state: &State) -> Frame {
        let mut tree = UiTree::new(content(state, 390.0, SCHEME.palette()));
        tree.set_reduced_motion(true);
        tree.layout(Size::new(390.0, 3000.0))
    }

    fn has_text(frame: &Frame, expected: &str) -> bool {
        // Wrapped paragraphs paint one command per line. Keep the full expected
        // sentence, comparing words across those visual line boundaries.
        let painted = frame.commands.iter().filter_map(|command| match command {
            DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }).collect::<Vec<_>>().join(" ");
        let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        words(&painted).contains(&words(expected))
    }

    #[test]
    fn preview_shows_current_synced_lines_and_remaining_time_without_old_track_text() {
        let mut state = state();
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![
            Line { at_ms: 5_000, text: "Primera línea real".into() },
            Line { at_ms: 25_000, text: "Segunda línea real".into() },
            Line { at_ms: 45_000, text: "Tercera línea real".into() },
        ])));
        let shown = frame(&state);
        assert!(has_text(&shown, "Segunda línea real"), "the live lyrics replace a generic invitation");
        assert!(has_text(&shown, "0:30") && has_text(&shown, "-2:30"));
        state.lyrics.as_mut().unwrap().0 = "previous-song".into();
        let stale = frame(&state);
        assert!(!has_text(&stale, "Segunda línea real"), "a previous track cannot appear in the new card");
        assert!(stale.access.iter().any(|node| node.id == "lyrics"));
    }

    #[test]
    fn lyric_loading_and_missing_states_are_honest_and_keep_the_full_page_available() {
        let mut state = state();
        for (lyrics, expected) in [(Lyrics::Loading, "Buscando la letra…"), (Lyrics::Missing, "No hay letra disponible.")] {
            state.lyrics = Some(("song".into(), lyrics));
            let shown = frame(&state);
            assert!(has_text(&shown, expected));
            assert!(shown.access.iter().any(|node| node.id == "lyrics" && !node.disabled));
        }
    }

    #[test]
    fn lyric_preview_exposes_failures_with_retry_and_plain_words_without_seek_targets() {
        let mut state = state();
        state.lyrics = Some(("song".into(), Lyrics::Failed { message: "Sin conexión al servicio".into(), retry_at: std::time::Instant::now() }));
        let failed = frame(&state);
        assert!(has_text(&failed, "Sin conexión al servicio"));
        assert!(failed.access.iter().any(|node| node.id == "retry-lyrics" && !node.disabled));
        state.lyrics = Some(("song".into(), Lyrics::Plain { lines: vec!["Texto disponible".into(), "Otro verso".into()], source: "Test".into() }));
        let plain = frame(&state);
        assert!(has_text(&plain, "Texto disponible") && has_text(&plain, "Letra sin sincronización"));
        assert!(!plain.regions.iter().any(|region| region.id.starts_with("lyric:")));
        state.lyrics.as_mut().unwrap().0 = "old-song".into();
        assert!(!has_text(&frame(&state), "Texto disponible"));
    }

    #[test]
    fn player_actions_stay_unique_touch_sized_and_reachable_in_short_phone_viewports() {
        let state = state();
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 360.0);
            let mut tree = UiTree::new(content(&state, width, SCHEME.palette()));
            tree.set_reduced_motion(true);
            tree.layout(size);
            for id in ["like", "seek", "shuffle", "previous", "play-pause", "next", "repeat", "mobile-connections", "do:share:current", "queue", "more:current", "lyrics"] {
                tree.reveal(id);
                let shown = tree.layout(size);
                let nodes: Vec<_> = shown.access.iter().filter(|node| node.id == id).collect();
                assert_eq!(nodes.len(), 1, "one {id} at {width}");
                let rect = nodes[0].rect;
                assert!(rect.width >= 43.99 && rect.height >= 43.99, "{id}: {rect:?}");
                assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= width + 0.01 && rect.y + rect.height <= size.height + 0.01, "reachable {id}: {rect:?}");
                assert_eq!(shown.regions.iter().filter(|region| region.id == id).count(), 1, "unique retained control {id}");
            }
        }
    }

    #[test]
    fn audio_route_button_uses_the_connection_glyph() {
        let shown = frame(&state());
        let button = shown.regions.iter().find(|region| region.id == "mobile-connections").unwrap();
        let expected = Icon::Custom(include_str!("../../../assets/icons/airplay-audio.svg"));
        let glyphs: Vec<_> = shown.commands.iter().filter_map(|command| match command {
            DrawCommand::Icon { rect, icon, .. } if button.rect.contains(limn::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)) => Some(*icon),
            _ => None,
        }).collect();
        assert_eq!(glyphs, [expected]);
    }

    #[test]
    fn sharing_keeps_the_existing_current_track_action_and_unknown_duration_disables_seek() {
        let mut state = state();
        state.player.duration = 0.0;
        let shown = frame(&state);
        assert!(shown.access.iter().any(|node| node.id == "seek" && node.disabled));
        assert!(shown.access.iter().any(|node| node.id == "do:share:current" && !node.disabled));
        let current = state.current().cloned();
        state.update(&UiAction::Clicked { id: "do:share:current".into() });
        assert_eq!(state.sharing, current);
    }

    #[test]
    fn artist_card_uses_only_the_matching_catalog_result_and_keeps_its_actions_touch_sized() {
        let mut state = state();
        assert!(!has_text(&frame(&state), "Acerca del artista"));
        state.about = Some(("Un artista".into(), Some((surco_core::ArtistRef {
            id: "artist".into(), name: "Un artista".into(), subscribers: None, artwork_url: None,
        }, Some("Biografía real recibida del catálogo.".into())))));
        let shown = frame(&state);
        assert!(has_text(&shown, "Acerca del artista"));
        assert!(has_text(&shown, "Biografía real recibida del catálogo."));
        let size = Size::new(320.0, 360.0);
        let mut tree = UiTree::new(content(&state, size.width, SCHEME.palette()));
        tree.set_reduced_motion(true);
        tree.layout(size);
        for id in ["about-artist", "about-follow"] {
            tree.reveal(id);
            let shown = tree.layout(size);
            let rect = shown.access.iter().find(|node| node.id == id).unwrap().rect;
            assert!(rect.width >= 43.99 && rect.height >= 43.99 && rect.x >= 0.0
                && rect.x + rect.width <= size.width + 0.01 && rect.y >= 0.0
                && rect.y + rect.height <= size.height + 0.01, "{id}: {rect:?}");
        }
        state.about.as_mut().unwrap().0 = "Otro artista".into();
        assert!(!has_text(&frame(&state), "Biografía real recibida del catálogo."));
    }
}
