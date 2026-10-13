//! The window, described from the state. Rebuilt on every frame.

use dnm_core::{Repeat, Track};
use limn::{
    ButtonVariant, Color, Component, ComponentPalette, ControlSize, Element, Empty, FontFamily, Icon, IconButton,
    ListRow, ScrollArea, SearchField, Size, Slider, Spinner, Theme,
};

use crate::state::{clock, State};

/// dnmusic's palette: near-black surfaces told apart by luminance, white ink.
pub const THEME: Theme = Theme {
    background: Color::rgb(18, 18, 18),
    foreground: Color::rgb(255, 255, 255),
    accent: Color::rgb(255, 255, 255),
};

const SIDEBAR_WIDTH: f32 = 260.0;
const PLAYER_HEIGHT: f32 = 76.0;
const VOLUME_WIDTH: f32 = 120.0;

pub fn view(state: &State, window: Size) -> Element {
    let palette = THEME.palette();
    let body = Element::row(vec![sidebar(state, palette), results(state, palette)]).flex(1.0);
    Element::panel(Element::column(vec![body, player(state, palette)]))
        .background(THEME.background)
        .size(Some(window.width), Some(window.height))
}

fn sidebar(state: &State, palette: ComponentPalette) -> Element {
    Element::panel(
        Element::column(vec![
            Element::text("dnmusic").font_family(FontFamily::Sans).font_size(18.0).foreground(palette.text),
            SearchField::new("search", &state.query, "Buscar canciones y artistas", palette)
                .width(SIDEBAR_WIDTH - 24.0)
                .render(),
        ])
        .gap(16.0),
    )
    .padding(12.0)
    .background(palette.surface)
    .size(Some(SIDEBAR_WIDTH), None)
}

fn results(state: &State, palette: ComponentPalette) -> Element {
    let centered = |element: Element| {
        Element::column(vec![Element::spacer().flex(1.0), element.centered_x(), Element::spacer().flex(1.0)]).flex(1.0)
    };
    if state.searching {
        return centered(Spinner::new("searching", palette.muted).size(24.0).render());
    }
    if let Some(error) = &state.search_error {
        return centered(Empty::new("No se pudo buscar", error, palette).icon(Icon::WarningTriangle).render());
    }
    if state.submitted.is_none() {
        return centered(
            Empty::new("Buscá algo para escuchar", "Escribí una canción o un artista y apretá Enter.", palette)
                .icon(Icon::Search)
                .render(),
        );
    }
    if state.results.is_empty() {
        return centered(Empty::new("Sin resultados", "Probá con otras palabras.", palette).icon(Icon::MusicNote).render());
    }
    let current = state.current().map(|track| track.id.as_str());
    let rows = state
        .results
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let row = ListRow::new(format!("track:{index}"), &track.title, palette)
                .leading(cover(state, track, 40.0, palette))
                .subtitle(&track.artist)
                .title_fade(true);
            let row = if track.duration_ms > 0 { row.meta(clock(track.duration_ms as f64 / 1000.0)) } else { row };
            if current == Some(track.id.as_str()) { row.filled() } else { row }.render()
        })
        .collect();
    ScrollArea::new("results", Element::column(rows).gap(2.0).padding(12.0)).render().flex(1.0)
}

fn cover(state: &State, track: &Track, side: f32, palette: ComponentPalette) -> Element {
    let radius = limn::tokens().radius.control;
    match track.artwork_url.as_ref().and_then(|url| state.artwork.get(url)) {
        Some(image) => Element::raster_image(image.clone()).size(Some(side), Some(side)).radius(radius),
        None => Element::panel(Element::icon(Icon::MusicNote).foreground(palette.muted).centered())
            .size(Some(side), Some(side))
            .background(palette.selected)
            .radius(radius),
    }
}

fn player(state: &State, palette: ComponentPalette) -> Element {
    let current = state.current();
    let has_track = current.is_some();
    let transport = |id: &str, icon: Icon, hint: &str| {
        IconButton::new(id, icon, palette)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Regular)
            .tooltip(hint)
            .enabled(has_track)
    };
    let now_playing = match current {
        Some(track) => Element::row(vec![
            cover(state, track, 48.0, palette),
            Element::column(vec![
                Element::faded_text(&track.title, 24.0).font_family(FontFamily::Sans).font_size(14.0).foreground(palette.text),
                Element::faded_text(state.player.error.as_deref().unwrap_or(&track.artist), 24.0)
                    .font_family(FontFamily::Sans)
                    .font_size(12.0)
                    .foreground(palette.muted),
            ])
            .gap(4.0)
            .flex(1.0)
            .centered_y(),
        ])
        .gap(12.0),
        None => Element::spacer(),
    };
    let repeat_icon = if state.player.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat };
    let buttons = Element::row(vec![
        Element::spacer().flex(1.0),
        transport("shuffle", Icon::Shuffle, "Aleatorio").active(state.player.shuffle).render(),
        transport("previous", Icon::SkipBack, "Anterior").render(),
        transport("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, if state.player.playing { "Pausar" } else { "Reproducir" })
            .render(),
        transport("next", Icon::SkipForward, "Siguiente").render(),
        transport("repeat", repeat_icon, "Repetir").active(state.player.repeat != Repeat::Off).render(),
        Element::spacer().flex(1.0),
    ])
    .gap(4.0);
    let time = |seconds: f64| {
        Element::text(clock(seconds)).font_family(FontFamily::Monospace).font_size(11.0).foreground(palette.muted).centered_y()
    };
    let seek = Element::row(vec![
        time(f64::from(state.progress()) * state.player.duration),
        Slider::new("seek", state.progress(), palette).enabled(has_track && state.player.duration > 0.0).render().flex(1.0).centered_y(),
        time(state.player.duration),
    ])
    .gap(10.0);
    let volume = Element::row(vec![
        Element::spacer().flex(1.0),
        IconButton::new("mute", if state.volume > 0.0 { Icon::Volume } else { Icon::VolumeMuted }, palette)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Compact)
            .tooltip(if state.volume > 0.0 { "Silenciar" } else { "Activar sonido" })
            .render()
            .centered_y(),
        Slider::new("volume", state.volume, palette).width(VOLUME_WIDTH).render().centered_y(),
    ])
    .gap(6.0);
    Element::panel(
        Element::row(vec![
            now_playing.flex(1.0).centered_y(),
            Element::column(vec![buttons, seek]).gap(2.0).flex(2.0).centered_y(),
            volume.flex(1.0).centered_y(),
        ])
        .gap(24.0),
    )
    .padding(12.0)
    .background(palette.surface)
    .size(None, Some(PLAYER_HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Event;
    use crate::player::PlayerView;
    use limn::{HitKind, UiTree};

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("Title {id}"),
            artist: "Artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: None,
            duration_ms: 200_000,
        }
    }

    fn asked(query: &str) -> State {
        State { query: query.into(), ..State::default() }
    }

    fn ids(state: &State, size: Size) -> Vec<String> {
        let mut tree = UiTree::new(view(state, size));
        // A disabled control keeps a hover region for its tooltip; these are the ones that answer a press.
        tree.layout(size).regions.iter().filter(|region| matches!(region.kind, HitKind::Button | HitKind::TextInput)).map(|region| region.id.clone()).collect()
    }

    #[test]
    fn every_state_lays_out_at_narrow_and_wide_windows() {
        let mut searching = asked("q");
        searching.search();
        let mut failed = asked("q");
        failed.search();
        failed.apply(Event::SearchFailed { query: "q".into(), message: "sin red".into() });
        let mut empty = asked("q");
        empty.search();
        empty.apply(Event::SearchResults { query: "q".into(), tracks: Vec::new() });
        for state in [State::default(), searching, failed, empty] {
            for width in [720.0, 1180.0, 2200.0] {
                let found = ids(&state, Size::new(width, 700.0));
                assert!(found.iter().any(|id| id == "search"), "the search field is always there at {width}");
                assert!(found.iter().any(|id| id == "volume"));
                assert!(!found.iter().any(|id| id == "seek"), "nothing to seek without a track");
            }
        }
    }

    #[test]
    fn results_are_clickable_rows_and_playing_one_enables_the_transport() {
        let mut state = asked("q");
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), tracks: vec![track("a"), track("b"), track("c")] });
        let size = Size::new(1180.0, 700.0);
        let before = ids(&state, size);
        for id in ["track:0", "track:1", "track:2"] {
            assert!(before.iter().any(|found| found == id), "{id} is a row");
        }
        assert!(!before.iter().any(|id| id == "play-pause"), "transport is passive with nothing loaded");
        state.show(PlayerView { current: Some(track("b")), playing: true, duration: 200.0, ..PlayerView::default() });
        let after = ids(&state, size);
        for id in ["play-pause", "next", "previous", "shuffle", "repeat", "seek", "mute", "volume"] {
            assert!(after.iter().any(|found| found == id), "{id} is live while a track plays");
        }
    }
}
