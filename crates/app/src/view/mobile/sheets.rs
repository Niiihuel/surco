//! Controlled touch surfaces over the same track commands used by desktop.

use super::*;
use limn::{BottomSheet, Separator};

#[cfg(test)]
#[path = "library_tests.rs"]
mod library_tests;

pub(super) fn render(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let context = state.mobile_ui.context.as_ref();
    let body = context.map_or_else(Element::spacer, |context| {
        let mut items = vec![track_header(state, &context.track, palette), Separator::horizontal(palette).render()];
        let mut actions = track_actions(state, &context.row, &context.track);
        if context.row.starts_with("upcoming:") {
            actions.insert(0, Some(Action { id: format!("do:unqueue:{}", context.row), label: "Quitar de la cola", icon: Icon::Minus, detail: None }));
        }
        items.extend(actions.into_iter().map(|action| match action {
            Some(action) => {
                let row = ListRow::new(action.id, action.label, palette)
                    .sizing(ListRowSizing { height: Some(52.0), two_line_height: Some(64.0), title_size: 15.0, leading_width: 24.0, ..TRACK_ROW })
                    .leading(Element::icon(action.icon).foreground(palette.text).size(Some(24.0), Some(24.0)));
                match action.detail { Some(detail) => row.subtitle(detail), None => row }.render()
            }
            None => Separator::horizontal(palette).margin(4.0).render(),
        }));
        Element::column(items).gap(4.0)
    });
    let base = BottomSheet::new(base, context.is_some(), "Opciones de la canción", "", "mobile-context-close", SCHEME.menu())
        .max_height(680.0).body(body).render();
    connections(state, library_sheet(state, base, palette), palette)
}

fn library_sheet(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let Some(context) = &state.mobile_ui.library else { return base };
    let pinned = state.collection.pinned.contains(&context.key);
    let row = |action: &str, label: &str, glyph| ListRow::new(format!("library-action:{action}"), label, palette)
        .sizing(ListRowSizing { height: Some(52.0), title_size: 15.0, ..TRACK_ROW })
        .leading(Element::icon(glyph).foreground(palette.text).size(Some(24.0), Some(24.0))).render();
    let header = Element::row(vec![cover(state, &context.artwork_url, 56.0, palette),
        Text::new(&context.title, palette).font_size(18.0).truncate().render().centered_y().flex(1.0),
    ]).gap(16.0).padding(8.0);
    let kept = context.entry.as_ref().is_none_or(|entry| state.card_entry_kept(entry));
    let mut body = vec![header, Separator::horizontal(palette).render(), row("open", "Abrir", Icon::ArrowRight)];
    if kept { body.push(row("pin", if pinned { "Desfijar" } else { "Fijar arriba" }, Icon::Pin)); }
    if context.entry.is_some() && !kept {
        body.push(row("keep", if context.key.starts_with("artist:") { "Seguir artista" } else { "Guardar en tu biblioteca" }, Icon::Add));
    } else if context.key.starts_with("album:") || context.key.starts_with("artist:") {
        body.push(row("remove", if context.key.starts_with("artist:") { "Dejar de seguir" } else { "Quitar de tu biblioteca" }, Icon::Minus));
    }
    BottomSheet::new(base, true, "Opciones de biblioteca", "", "mobile-library-close", SCHEME.menu())
        .max_height(480.0).body(Element::column(body).gap(8.0))
        .return_focus(format!("library-more:{}", context.key)).render()
}

fn track_header(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    Element::row(vec![cover(state, &track.artwork_url, 56.0, palette).radius(6.0),
        Element::column(vec![Text::new(&track.title, palette).font_size(17.0).truncate().render(),
            Text::new(&track.artist, palette).font_size(14.0).muted().truncate().render()]).gap(6.0).centered_y().flex(1.0),
    ]).gap(16.0).padding(8.0)
}

fn connections(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let name = state.mobile_ui.route.as_ref().map_or("Salida del sistema", |(name, _)| name.as_str());
    let route = Element::panel(Element::row(vec![
        Element::icon(Icon::Custom(AUDIO_ROUTE)).foreground(palette.text).size(Some(32.0), Some(32.0)).centered_y(),
        Element::column(vec![Text::new("ESCUCHANDO EN", palette).role(TextRole::Caption).muted().render(),
            Text::new(name, palette).font_size(22.0).truncate().render()]).gap(8.0).flex(1.0),
        Element::icon(Icon::Check).foreground(palette.text).centered_y(),
    ]).gap(16.0).padding(20.0)).background(SCHEME.raised).radius(12.0);
    // UIKit owns the visible route button and its tap. This slot only reserves
    // geometry; no simulated click or private picker presentation is used.
    let picker = Element::panel(if cfg!(target_os = "ios") { Element::spacer() } else { Element::icon(Icon::Custom(AUDIO_ROUTE)).foreground(palette.text).centered() })
        .size(Some(TAP), Some(TAP))
        .accessible(limn::Access::new(limn::Role::Group).id("ios-route-picker").name("Selector de destinos de audio"));
    let choose = Element::row(vec![Element::column(vec![
        Text::new("AirPlay o Bluetooth", palette).font_size(17.0).render(),
        Text::new(if state.mobile_ui.native_routes { "Abrí el selector de audio" } else { "Disponible en el iPhone" }, palette).muted().render(),
    ]).gap(6.0).centered_y().flex(1.0), picker]).gap(12.0).padding(12.0);
    let guide = Element::column(vec![
        heading("¿Un dispositivo nuevo?", palette),
        Text::new("Vinculá tus auriculares o parlantes en Ajustes > Bluetooth del iPhone. Después elegilos desde el selector de audio.", palette).muted().render(),
    ]).gap(12.0).padding(8.0);
    let volume = Element::column(vec![Text::new("Volumen de Surco", palette).muted().render(),
        Slider::new("volume", state.volume, palette).render().size(None, Some(TAP)),
    ]).gap(8.0).padding(8.0);
    let body = Element::column(vec![route, choose, Separator::horizontal(palette).render(), guide, volume]).gap(16.0);
    let done = Button::new("mobile-connections-close", "Listo", palette).variant(ButtonVariant::Default)
        .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render();
    BottomSheet::new(base, state.mobile_ui.connections, "Escuchar en", "Elegí dónde sigue la música.", "mobile-connections-close", SCHEME.menu())
        .max_height(600.0).body(body).footer(done).return_focus("mobile-connections").render()
}
