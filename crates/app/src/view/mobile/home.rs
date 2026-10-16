use super::*;

pub(super) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let empty = state.collection.recent.is_empty() && state.collection.kept.is_empty()
        && state.collection.heard.is_empty() && state.saved.is_empty();
    if !empty { return super::super::home(state, width, palette); }
    let intro = Element::column(vec![
        Text::new(t!("mobile-home-title"), palette).font_size(36.0).render(),
        Text::new(t!("mobile-home-intro"), palette).font_size(16.0).muted().render(),
        Button::new("mobile-discover", t!("mobile-home-discover"), palette).icon(Icon::Search)
            .variant(ButtonVariant::Default).sizing(limn::ButtonSizing { height: 48.0, ..ControlSize::Spacious.button_sizing() }).round().render(),
    ]).gap(20.0).padding(24.0);
    let panel = Element::panel(intro).background(SCHEME.raised).radius(16.0);
    ScrollArea::new("mobile-home", Element::column(vec![panel,
        heading(&t!("mobile-home-curiosity"), palette), search::browse(4, width - 32.0, 2, 98.0, palette),
        Element::row(vec![Element::icon(Icon::Heart).foreground(palette.muted).centered_y(),
            Text::new(t!("mobile-home-favorites"), palette).muted().render().flex(1.0)]).gap(12.0).padding(8.0),
    ]).gap(24.0).padding(16.0)).render().flex(1.0)
}
