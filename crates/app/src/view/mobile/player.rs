//! Phone player content. The mobile shell owns its header and dismissal gesture.

use super::*;

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else {
        return centered(Empty::new(t!("mobile-no-music"), t!("mobile-no-music-detail"), palette)
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
            &if liked { t!("mobile-unlike") } else { t!("common-like") }, palette).centered_y(),
    ]).gap(12.0).anchor("mobile-player-title");
    let mut controls = vec![details];
    if let Some(error) = &state.player.error {
        controls.push(Alert::new(t!("mobile-play-failed"), error, palette).id("mobile-player-error").render());
    }
    controls.extend([timeline(state, palette), transport(state, palette), tools(state, palette)]);
    let main = Element::column(vec![
        sung_over(state, sole_cover(state, &largest(state, track), side, palette).radius(8.0), side, palette).centered_x()
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

pub(super) fn timeline(state: &State, palette: ComponentPalette) -> Element {
    let duration = state.player.duration.max(0.0);
    let progress = state.progress();
    let elapsed = if duration > 0.0 { f64::from(progress) * duration } else { state.player.position.max(0.0) };
    // Ceiling keeps the countdown consistent with the elapsed clock's whole seconds,
    // including the tiny rounding error introduced by the slider's f32 fraction.
    let remaining = if duration > 0.0 { format!("-{}", clock((duration - elapsed).max(0.0).ceil())) } else { "–:––".into() };
    let time = |value: String| Text::new(value, palette).mono().font_size(12.0).muted().render();
    Element::column(vec![
        Slider::new("seek", progress, palette).enabled(duration > 0.0)
            .accessibility_label(t!("mobile-playback-position")).render().size(None, Some(TAP)),
        Element::row(vec![time(clock(elapsed)), Element::spacer().flex(1.0), time(remaining)]),
    ])
}

pub(super) fn transport(state: &State, palette: ComponentPalette) -> Element {
    let control = |id: &str, glyph, label: &str, glyph_size, active| {
        let action = controls::action(id, glyph, label, glyph_size, palette);
        if active { action.background(palette.selected) } else { action }.centered_y()
    };
    let play = controls::playback("play-pause", state.player.playing, 72.0, 32.0, palette);
    Element::row(vec![
        control("shuffle", shuffle_step(state).0, &shuffle_step(state).1, 24.0, state.player.shuffle),
        Element::spacer().flex(1.0),
        control("previous", Icon::SkipBack, &t!("mobile-previous-song"), 28.0, false),
        Element::spacer().flex(1.0), play, Element::spacer().flex(1.0),
        control("next", Icon::SkipForward, &t!("common-next-song"), 28.0, false),
        Element::spacer().flex(1.0),
        control("repeat", if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, &t!("mobile-repeat"), 24.0, state.player.repeat != Repeat::Off),
    ])
}

fn tools(state: &State, palette: ComponentPalette) -> Element {
    Element::row(vec![
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), &t!("mobile-connect-device"), palette),
        Element::spacer().flex(1.0),
        icon("do:share:current", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), &t!("common-share-song"), palette),
        icon("sleep", Icon::Custom(crate::view::glyph::SLEEP), &crate::view::sleep_hint(state), palette),
        icon("queue", Icon::Playlist, &t!("mobile-show-queue"), palette),
        icon("more:current", Icon::More, &t!("common-song-options"), palette),
    ]).gap(8.0)
}

/// What the glass player's foot stands on under its cover: the song's name, the timeline, the
/// transport and its tools, with the gaps between them.
const GLASS_CONTROLS: f32 = 300.0;

/// The full player under Liquid Glass, as dnmusic's iPhone player: no page to scroll, the cover
/// in all the room the controls leave (stepping back while paused), the song under it, and the
/// lyrics, where it plays, the timer and the queue spread along its foot.
pub(super) fn glass(state: &State, width: f32, height: f32, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else { return content(state, width, palette) };
    let side = (width - 48.0).min(380.0).min(height - GLASS_CONTROLS - 32.0).max(120.0);
    let rest = side * state.cover_rest(std::time::Instant::now());
    let cover = sole_cover(state, &largest(state, track), rest, palette).radius(10.0)
        .animate_enter(format!("mobile-player-cover:{}", track.id));
    let stage = Element::column(vec![Element::spacer().flex(1.0), cover.centered_x(), Element::spacer().flex(1.0)])
        .size(None, Some(side)).flex(1.0);
    let liked = state.is_liked(&track.id);
    let details = Element::row(vec![
        Element::column(vec![
            Text::new(&track.title, palette).font_size(22.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(&track.artist, palette).font_size(17.0).muted().truncate().render(),
        ]).gap(3.0).flex(1.0).centered_y(),
        icon("like", if liked { Icon::HeartFill } else { Icon::Heart },
            &if liked { t!("mobile-unlike") } else { t!("common-like") }, palette).centered_y(),
    ]).gap(12.0).anchor("mobile-player-title");
    let mut controls = vec![details];
    if let Some(error) = &state.player.error {
        controls.push(Alert::new(t!("mobile-play-failed"), error, palette).id("mobile-player-error").render());
    }
    controls.extend([timeline(state, palette), transport(state, palette), foot(state, palette)]);
    Element::column(vec![stage, Element::column(controls).gap(12.0)]).gap(16.0)
        .padding(24.0).flex(1.0)
}

/// The glass player's foot, the same under its cover and its lyrics: the lyrics' button, lit
/// while they are open, where it plays, the timer, and the queue (the lyrics share the song there).
pub(super) fn foot(state: &State, palette: ComponentPalette) -> Element {
    let open = state.page == Page::Lyrics;
    let lyrics = icon("lyrics", Icon::Custom(crate::view::LYRICS), &if open { t!("mobile-close-lyrics") } else { t!("mobile-show-lyrics") }, palette).radius(22.0);
    let last = if open {
        icon("do:share:current", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), &t!("common-share-song"), palette)
    } else { icon("queue", Icon::Playlist, &t!("mobile-show-queue"), palette) };
    let tools = [
        if open { lyrics.background(Color::rgba(255, 255, 255, 40)) } else { lyrics },
        icon("mobile-connections", Icon::Custom(AUDIO_ROUTE), &t!("mobile-connect-device"), palette),
        icon("sleep", Icon::Custom(crate::view::glyph::SLEEP), &crate::view::sleep_hint(state), palette),
        last,
    ];
    let mut row = vec![Element::spacer().flex(1.0)];
    for tool in tools { row.extend([tool, Element::spacer().flex(1.0)]); }
    Element::row(row)
}

const LYRIC_CARD_LINE: f32 = 30.0;
const LYRIC_CARD_GAP: f32 = 18.0;

/// The line being sung over the foot of the cover, as words over a song's video, on a scrim.
fn sung_over(state: &State, cover: Element, side: f32, palette: ComponentPalette) -> Element {
    let Some((index, line)) = state.active_line().and_then(|index| state.lines().get(index).map(|line| (index, line)))
        .filter(|(_, line)| !line.text.trim().is_empty()) else { return cover };
    let words = Text::new(&line.text, palette).font_size(20.0).line_height(26.0).render()
        .font_family(FontFamily::SansBold).animate_enter(format!("mobile-cover-line:{index}"));
    let foot = Element::column(vec![Element::spacer().flex(1.0), words]).padding(16.0).size(None, Some(side * 0.45))
        // Rounded as the cover: a gradient is not cut by its parent's corners, only by its own.
        .vertical_gradient(vec![Color::rgba(0, 0, 0, 0), COVER_SCRIM]).radius(8.0);
    Element::panel(Element::stack(vec![cover, Element::column(vec![Element::spacer().flex(1.0), foot])]))
        .radius(8.0).clip_children().size(Some(side), Some(side))
}

/// The lyrics card's own buttons: round, on a faint glass of their own.
fn card_button(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    controls::action(id, glyph, label, 18.0, palette).background(Color::rgba(255, 255, 255, 22))
}

fn lyric_preview(state: &State, palette: ComponentPalette) -> Element {
    // Translucent over the song's colour, as a pane of glass rather than a grey slab.
    let card = ComponentPalette { surface: Color::rgba(255, 255, 255, 20), ..palette };
    let share = card_button("lyrics-share", Icon::Custom(include_str!("../../../assets/icons/share-2.svg")), &t!("common-share-song"), card)
        .dispatch_as("do:share:current");
    let mut parts = vec![Element::row(vec![
        Text::new(t!("mobile-lyrics"), card).font_size(16.0).render().font_family(FontFamily::SansBold).centered_y().flex(1.0),
        share, card_button("lyrics", Icon::Expand, &t!("mobile-show-lyrics"), card),
    ]).gap(8.0)];
    let lines = state.lines();
    if !lines.is_empty() {
        // Every line, in a window four lines tall that slides them up so the one being sung
        // rests second (the Host measures where it is); each keeps its own ink, so it
        // brightens and dims as it passes instead of switching.
        let sung = state.active_line().map_or(-1, |index| index as i64);
        let rows = lines.iter().enumerate().map(|(index, line)| {
            let ink = Color { a: (255.0 * lyric_focus(index as i64 - sung).0) as u8, ..card.text };
            Text::new(if line.text.trim().is_empty() { "♪" } else { &line.text }, card)
                .font_size(22.0).line_height(LYRIC_CARD_LINE).ink(ink).render()
                .font_family(FontFamily::SansBold).animate_foreground(format!("lyric-card-ink:{index}"))
                .anchor(format!("lyric-card:{index}"))
        }).collect();
        let slide = limn::Point::new(0.0, -state.lyric_card_offset(std::time::Instant::now()));
        let height = 4.0 * LYRIC_CARD_LINE + 3.0 * LYRIC_CARD_GAP;
        parts.push(Element::column(vec![Element::column(rows).gap(LYRIC_CARD_GAP).translate(slide)])
            .size(None, Some(height)).clip_children());
    } else if !state.plain_lyrics().is_empty() {
        parts.extend(state.plain_lyrics().iter().filter(|line| !line.is_empty()).take(4)
            .map(|line| Text::new(line, card).font_size(22.0).line_height(30.0).render()));
        let source = match &state.lyrics { Some((_, Lyrics::Plain { source, .. })) => source.as_str(), _ => "" };
        parts.push(Text::new(t!("mobile-unsynced-lyrics", source = source), card).font_size(14.0).muted().render());
    } else {
        let current = state.current().map(|track| track.id.as_str());
        let known = state.lyrics.as_ref().filter(|(id, _)| Some(id.as_str()) == current);
        let message = match known {
            Some((_, Lyrics::Loading)) => t!("mobile-lyrics-loading"),
            Some((_, Lyrics::Missing | Lyrics::Lines(_) | Lyrics::Plain { .. })) => t!("mobile-lyrics-missing"),
            Some((_, Lyrics::Failed { message, .. })) => message.as_str().into(),
            None => t!("mobile-lyrics-open"),
        };
        parts.push(Text::new(message, card).font_size(16.0).muted().render());
        if matches!(known, Some((_, Lyrics::Failed { .. } | Lyrics::Missing))) {
            parts.push(Button::new("retry-lyrics", t!("common-retry"), card).variant(ButtonVariant::Outline)
                .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render());
        }
    }
    Element::panel(Element::column(parts).gap(14.0).padding(20.0)).background(card.surface).radius(20.0)
}

fn artist_card(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let (artist, biography) = state.about_artist()?;
    let card = ComponentPalette { surface: SCHEME.raised, ..palette };
    let followed = state.is_followed(artist);
    let name = Element::clickable("about-artist", Text::new(&artist.name, card).font_size(20.0).truncate().render().centered_y())
        .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-view-artist", name = artist.name.as_str())))
        .size(None, Some(TAP)).flex(1.0);
    let follow = Button::new("about-follow", if followed { t!("common-following") } else { t!("common-follow") }, card)
        .variant(ButtonVariant::Outline).sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render();
    let mut copy = vec![
        Text::new(t!("mobile-about-artist"), card).font_size(16.0).render(),
        Element::row(vec![name, follow]).gap(12.0),
    ];
    if let Some(subscribers) = &artist.subscribers {
        copy.push(Text::new(t!("mobile-subscribers", subscribers = subscribers.as_str()), card).muted().render());
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
        .clip_children().size(Some(width), None))
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
    fn the_line_being_sung_stands_over_the_cover_once_and_only_while_it_is_sung() {
        let mut state = state();
        let painted = |frame: &Frame, text: &str| frame.commands.iter()
            .filter(|command| matches!(command, DrawCommand::Text { text: painted, .. } if painted == text)).count();
        assert_eq!(painted(&frame(&state), "Segunda línea real"), 0);
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![
            Line { at_ms: 5_000, text: "Primera línea real".into() },
            Line { at_ms: 25_000, text: "Segunda línea real".into() },
        ])));
        // Over the cover and in the card beneath.
        let sung = frame(&state);
        assert_eq!(painted(&sung, "Segunda línea real"), 2);
        // The scrim's foot keeps the cover's rounded corners instead of squaring them in black.
        let dark = |color: &Color| color.a > 100 && color.r == 0 && color.g == 0 && color.b == 0;
        let band = sung.commands.iter().find_map(|command| match command {
            DrawCommand::FillVerticalGradient { rect, bottom, .. } if dark(bottom) => Some(*rect),
            _ => None,
        }).unwrap();
        let foot = sung.commands.iter().filter_map(|command| match command {
            DrawCommand::FillRect { rect, color, .. } if dark(color) && rect.y >= band.y + band.height => Some(*rect),
            _ => None,
        }).max_by(|a, b| (a.y + a.height).total_cmp(&(b.y + b.height))).expect("the scrim's corners are painted rounded");
        assert!(foot.x >= band.x + 2.0, "the lowest row is inset by the corner: {foot:?}");
        state.lyrics = Some(("song".into(), Lyrics::Lines(vec![Line { at_ms: 25_000, text: "  ".into() }])));
        assert_eq!(painted(&frame(&state), "♪"), 1, "a silent line is the card's note alone; the cover stays bare");
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
    fn glass_player_fits_one_screen_with_its_foot_and_toggles_the_lyrics_in_place() {
        let mut state = state();
        state.mobile_ui.native_surfaces = true;
        state.mobile_ui.glass = true;
        let ids = ["like", "seek", "shuffle", "previous", "play-pause", "next", "repeat", "lyrics", "mobile-connections", "sleep"];
        for size in [Size::new(320.0, 568.0), Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            for page in [Page::Playing, Page::Lyrics] {
                state.page = page.clone();
                let mut tree = UiTree::new(crate::view::view(&state, size));
                tree.set_reduced_motion(true);
                let shown = tree.layout(size);
                let last = if page == Page::Playing { "queue" } else { "do:share:current" };
                for id in ids.iter().copied().chain([last]) {
                    let nodes: Vec<_> = shown.access.iter().filter(|node| node.id == id).collect();
                    assert_eq!(nodes.len(), 1, "one {id} on {page:?} at {size:?}");
                    let rect = nodes[0].rect;
                    assert!(rect.width >= 43.99 && rect.height >= 43.99, "{id}: {rect:?}");
                    assert!(rect.y >= 0.0 && rect.y + rect.height <= size.height + 0.01, "{id} on screen, not scrolled to: {rect:?}");
                }
                assert!(!shown.access.iter().any(|node| node.id == "about-artist"), "no Spotify cards under the player");
            }
        }
        state.page = Page::Playing;
        state.update(&UiAction::Clicked { id: "lyrics".into() });
        assert_eq!(state.page, Page::Lyrics);
        state.update(&UiAction::Clicked { id: "lyrics".into() });
        assert_eq!(state.page, Page::Playing, "the lit button puts the cover back");

        let later = std::time::Instant::now() + std::time::Duration::from_secs(3);
        assert_eq!(state.cover_rest(later), 1.0, "whole while it plays");
        let paused = PlayerView { playing: false, ..state.player.clone() };
        state.show(paused);
        assert!((state.cover_rest(later) - 0.82).abs() < 0.01, "stepped back while paused");
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
