//! The phone's settings as iOS inset-grouped cards in monochrome, like the app's mark: white
//! glyphs and words on gray cards, bold section headings, white for what is on.

use super::*;
use limn::{CornerRadii, Separator};

const GUTTER: f32 = 16.0;
/// The least height of a row, above the 44-point touch minimum.
const ROW_HEIGHT: f32 = 52.0;
/// A row's inside: 10 above and below, 16 at the sides once `cell` adds its 6.
const ROW_PAD: f32 = 10.0;
const GLYPH: f32 = 24.0;
const ROW_GAP: f32 = 14.0;
const CARD: Color = CHOSEN;
const CARD_RADIUS: f32 = 16.0;
/// Hairlines start under the words, past the glyph, as in iOS.
const SEPARATOR_INSET: f32 = GUTTER + GLYPH + ROW_GAP;
/// A segment's height; the track adds 2 on each side, to a 44-point target.
const SEGMENT: f32 = 40.0;
/// The only hue: what cannot be undone.
const DESTRUCTIVE: Color = Color::rgb(255, 69, 58);

/// The glyph, the name over an optional line about it, and what sits at the end.
fn line(glyph: Icon, title: &str, detail: Option<&str>, trailing: Option<Element>, ink: Color) -> Element {
    let palette = SCHEME.palette();
    let mut copy = vec![Text::new(title, palette).font_size(16.0).ink(ink).render()];
    if let Some(detail) = detail { copy.push(Text::new(detail, palette).font_size(13.0).muted().render()); }
    let glyph = Element::icon(glyph).large_icon(GLYPH).foreground(ink).size(Some(GLYPH), Some(GLYPH));
    let mut children = vec![glyph.centered_y(), Element::column(copy).gap(3.0).centered_y().flex(1.0)];
    children.extend(trailing.map(Element::centered_y));
    Element::row(children).gap(ROW_GAP)
}

fn cell(content: Element) -> Element {
    horizontal_inset(content.min_size(0.0, ROW_HEIGHT).padding(ROW_PAD), GUTTER - ROW_PAD, GUTTER - ROW_PAD)
}

fn value(text: &str) -> Element {
    Text::new(text, SCHEME.palette()).font_size(15.0).muted().render()
}

fn chevron() -> Element {
    Element::icon(Icon::ChevronRight).foreground(SCHEME.muted)
}

fn switch(glyph: Icon, title: &str, id: &str, on: bool) -> Element {
    // Monochrome: a white track with a dark thumb when on, a white thumb on gray when off.
    let track = ComponentPalette { surface: if on { SCHEME.canvas } else { Color::WHITE }, selected: SCHEME.control.border, ..SCHEME.on_raised() };
    let control = Switch::new(id, on, track).ink(Color::WHITE).size(ControlSize::Spacious).accessibility_label(title).render();
    cell(line(glyph, title, None, Some(control), Color::WHITE))
}

/// A whole row that does one thing; a destructive one is named in red. A disabled one keeps
/// its id, passive and muted.
fn action(id: &str, glyph: Icon, title: &str, detail: Option<&str>, trailing: Option<Element>, enabled: bool, destructive: bool) -> Element {
    let ink = if !enabled { SCHEME.muted } else if destructive { DESTRUCTIVE } else { Color::WHITE };
    let row = cell(line(glyph, title, detail, trailing, ink));
    let mut said = limn::Access::new(limn::Role::Button).name(title);
    if let Some(detail) = detail { said = said.description(detail); }
    if !enabled { return row.accessible(said.id(id).disabled(true)); }
    // Square here: the group rounds its first and last rows to the card's corners.
    ComponentPalette { surface: CARD, hover: SCHEME.selected, selected: SCHEME.control.pressed, ..SCHEME.palette() }
        .bar_item_surface(Element::clickable(id, row).accessible(said))
}

/// A row whose control needs the card's width: its line, then the control under it.
fn stacked(head: Element, control: Element) -> Element {
    cell(Element::column(vec![head, control]).gap(12.0))
}

/// A pill-shaped choice among a few: the chosen one white with dark words.
fn segmented(group: &str, width: f32, tabs: Vec<Tab>) -> Element {
    let palette = ComponentPalette { surface: SCHEME.selected, selected: Color::WHITE, ..SCHEME.palette() };
    let mut strip = Tabs::new(tabs, palette).id(group).height(SEGMENT).accent(SCHEME.canvas).fill_width(width).render();
    // shortcut: Tabs take their corners from limn's global radius tokens; round them here until Tabs has a radius.
    if let limn::ElementKind::TabStrip { selection: Some(selection), .. } = &mut strip.kind { selection.radius = SEGMENT / 2.0; }
    strip.radius(SEGMENT / 2.0 + 2.0)
}

/// A bold heading, a rounded card of rows with inset hairlines, and a note only when one helps.
fn group(title: &str, rows: Vec<Element>, footer: &str) -> Element {
    let palette = SCHEME.palette();
    let last = rows.len().saturating_sub(1);
    let mut cells = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 { cells.push(horizontal_inset(Separator::horizontal(palette).render(), SEPARATOR_INSET, 0.0)); }
        let (top, bottom) = (if index == 0 { CARD_RADIUS } else { 0.0 }, if index == last { CARD_RADIUS } else { 0.0 });
        cells.push(row.corner_radii(CornerRadii { top_left: top, top_right: top, bottom_right: bottom, bottom_left: bottom }));
    }
    let card = Element::panel(Element::column(cells)).background(CARD).radius(CARD_RADIUS).clip_children()
        .accessible(limn::Access::new(limn::Role::Group).id(format!("settings-group:{title}")).name(title));
    let caption = |text: Element| horizontal_inset(text, GUTTER, GUTTER);
    let heading = Text::new(title, palette).role(TextRole::Heading).font_size(17.0).render().font_family(FontFamily::SansBold);
    let mut parts = vec![caption(heading), card];
    if !footer.is_empty() { parts.push(caption(Text::new(footer, palette).font_size(13.0).muted().render())); }
    Element::column(parts).gap(8.0)
}

pub(in crate::view) fn page(state: &State, width: f32) -> Element {
    // The card's inside, for controls that span it.
    let inner = (width - GUTTER * 4.0).max(0.0);
    let white = Color::WHITE;
    let seconds = state.settings.crossfade;
    let crossfade = stacked(
        line(Icon::Shuffle, "Crossfade", None, Some(value(&if seconds == 0.0 { "No".to_owned() } else { format!("{seconds:.0} s") })), white),
        Slider::new("setting:crossfade", seconds / CROSSFADE_MAX, SCHEME.on_raised()).steps(CROSSFADE_MAX as usize + 1).width(inner).render(),
    );
    let playback = group("Reproducción", vec![
        switch(Icon::Infinity, "Reproducción automática", "setting:autoplay", state.settings.autoplay),
        crossfade,
    ], "");

    let mut connections = group("Conexiones y dispositivos", vec![
        action("setting:connections", Icon::Custom(AUDIO_ROUTE), "Escuchar en otro dispositivo", None, Some(chevron()), true, false),
    ], "");
    if let Some(notice) = &state.output_notice {
        connections = Element::column(vec![connections, Alert::new("Salida de audio", notice, SCHEME.palette()).id("setting:output-notice").compact(true).render()]).gap(8.0);
    }

    let chosen = state.settings.quality;
    let ceiling = match chosen { Quality::Low => "Hasta 50 kbps", Quality::Normal => "Hasta 70 kbps", Quality::High => "Hasta 140 kbps" };
    let quality = stacked(
        line(Icon::ChartBar, "Calidad", None, Some(value(ceiling)), white),
        segmented("setting-quality", inner, Quality::ALL.into_iter()
            .map(|(quality, name, label)| Tab::new(format!("setting:quality:{name}"), label, 0.0).active(quality == chosen)).collect()),
    );
    let equalizer = if state.equalizer == surco_audio::equalizer::EqualizerValues::FLAT { "Plano" } else { "Personalizado" };
    let audio = group("Calidad de audio", vec![
        quality,
        switch(Icon::Volume, "Normalizar el volumen", "setting:normalize", state.settings.normalize),
        switch(Icon::Headphones, "Audio mono", "setting:mono", state.settings.mono),
        action("equalizer", Icon::Sliders, "Ecualizador", None,
            Some(Element::row(vec![value(equalizer).centered_y(), chevron().centered_y()]).gap(6.0)), true, false),
    ], "La calidad cambia desde la próxima canción.");

    let display = group("Visualización", vec![
        switch(Icon::Bolt, "Reducir movimiento", "setting:motion", state.settings.reduce_motion),
    ], "");

    let sort = state.settings.library_sort;
    let view = state.settings.library_view;
    let library = group("Biblioteca", vec![
        stacked(line(Icon::Sort, "Orden", None, None, white), segmented("setting-library-sort", inner, vec![
            Tab::new("library-sort:added", "Recientes", 0.0).active(sort == Sort::Added),
            Tab::new("library-sort:alphabetical", "A–Z", 0.0).active(sort == Sort::Alphabetical),
            Tab::new("library-sort:creator", "Creador", 0.0).active(sort == Sort::Creator),
        ])),
        stacked(line(Icon::Grid, "Vista", None, None, white), segmented("setting-library-view", inner, vec![
            Tab::new("library-view:compact", "Compacta", 0.0).active(view == LibraryView::Compact),
            Tab::new("library-view:list", "Lista", 0.0).active(view == LibraryView::List),
            Tab::new("library-view:grid", "Cuadrícula", 0.0).active(view == LibraryView::Grid),
        ])),
        action("import-open", Icon::Playlist, "Importar música de otras apps", None, Some(chevron()), true, false),
    ], "");

    let about = group("Acerca de", vec![
        cell(line(Icon::InfoCircle, "Versión", None, Some(value(env!("CARGO_PKG_VERSION"))), white)),
        cell(line(Icon::Book, "Licencias de terceros", Some("zuno, Spell UI, Lucide, Bootstrap Icons y ytmapi-rs"), None, white)),
    ], "");

    // What cannot be undone sits last, each behind a confirmation.
    let downloads = format!("{} · {}", songs(state.saved.len()), megabytes(state.saved_bytes));
    let erase = group("Almacenamiento y privacidad", vec![
        action("downloads-clear", Icon::Trash, "Eliminar todas las descargas", Some(&downloads), None, !state.saved.is_empty(), true),
        action("recent-clear", Icon::History, "Borrar abiertos recientemente", Some("Álbumes y artistas de Inicio"), None, !state.collection.recent.is_empty(), true),
    ], "");

    let groups = vec![playback, connections, audio, display, library, about, erase];
    ScrollArea::new("settings", Element::column(groups).gap(28.0).padding(GUTTER)).render().flex(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{DrawCommand, Frame, Rect, UiTree};

    const ROWS: [&str; 12] = ["setting:autoplay", "setting:crossfade", "setting:connections", "setting:quality:high", "setting:normalize",
        "setting:mono", "equalizer", "setting:motion", "library-sort:creator", "library-view:grid", "import-open", "downloads-clear"];

    fn frame(state: &State, width: f32) -> Frame {
        let mut tree = UiTree::new(page(state, width));
        tree.set_reduced_motion(true);
        tree.layout(Size::new(width, 4000.0))
    }

    fn rect(frame: &Frame, id: &str) -> Rect {
        frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("{id} is shown")).rect
    }

    #[test]
    fn every_setting_is_a_full_row_inside_the_page_at_phone_widths() {
        let mut state = State::default();
        state.mobile = true;
        for width in [320.0, 390.0, 430.0] {
            let frame = frame(&state, width);
            for id in ROWS.into_iter().chain(["recent-clear", "setting:quality:low", "library-sort:added", "library-view:compact"]) {
                let target = rect(&frame, id);
                assert!(target.x >= GUTTER - 0.01 && target.x + target.width <= width - GUTTER + 0.01, "{id} keeps the 16-point margins at {width}: {target:?}");
            }
            for id in ["setting:connections", "equalizer", "import-open", "downloads-clear", "recent-clear"] {
                let row = rect(&frame, id);
                assert!(row.height >= 44.0, "{id} is a full touch row at {width}: {row:?}");
                assert!((row.width - (width - GUTTER * 2.0)).abs() < 0.01, "{id} spans its card at {width}: {row:?}");
            }
            for id in ["setting:quality:normal", "library-sort:alphabetical", "library-view:list"] {
                assert!(rect(&frame, id).height >= 39.99, "{id} is a comfortable segment at {width}: {:?}", rect(&frame, id));
            }
            for id in ["setting:autoplay", "setting:normalize", "setting:mono", "setting:motion"] {
                assert!(rect(&frame, id).width >= 44.0, "{id} is easy to toggle at {width}");
            }
            // Switches sit in their own rows: two in a row are a full row apart.
            assert!(rect(&frame, "setting:mono").y - rect(&frame, "setting:normalize").y >= ROW_HEIGHT);
        }
    }

    #[test]
    fn the_whole_phone_page_keeps_rows_inside_the_safe_area() {
        for (width, left, right) in [(320.0, 0.0, 0.0), (390.0, 0.0, 0.0), (430.0, 0.0, 0.0), (874.0, 62.0, 34.0)] {
            let mut state = State::default();
            state.mobile = true;
            state.page = Page::Settings;
            state.mobile_ui.content_insets = crate::state::mobile::ContentInsets { top: 62.0, bottom: 34.0, left, right };
            let mut tree = UiTree::new(super::super::view(&state, Size::new(width, 6000.0)));
            tree.set_reduced_motion(true);
            let frame = tree.layout(Size::new(width, 6000.0));
            for id in ROWS {
                let target = rect(&frame, id);
                assert!(target.x >= left + GUTTER - 0.01 && target.x + target.width <= width - right - GUTTER + 0.01, "{id} clears the insets at {width}: {target:?}");
            }
        }
    }

    #[test]
    fn groups_are_monochrome_rounded_cards_and_destructive_actions_come_last() {
        let mut state = State::default();
        state.mobile = true;
        let frame = frame(&state, 390.0);
        let groups: Vec<_> = frame.access.iter().filter(|node| node.id.starts_with("settings-group:")).collect();
        assert_eq!(groups.len(), 7);
        for group in &groups {
            assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::FillRect { rect, color, radius }
                if *color == CARD && *radius == CARD_RADIUS && (rect.y - group.rect.y).abs() < 0.01)), "{} is a rounded card", group.id);
        }
        // No hue but the destructive red: the page matches the monochrome mark.
        for command in &frame.commands {
            if let DrawCommand::FillRect { color, .. } = command {
                assert!(color.r == color.g && color.g == color.b, "a gray fill, not a card tint: {color:?}");
            }
        }
        assert_eq!(groups.last().unwrap().id, "settings-group:Almacenamiento y privacidad");
        let last_safe = ROWS.iter().filter(|id| !id.ends_with("-clear")).map(|id| rect(&frame, id).y).fold(0.0, f32::max);
        assert!(rect(&frame, "downloads-clear").y > last_safe && rect(&frame, "recent-clear").y > last_safe);
        // With nothing to erase, both stay listed but cannot be pressed.
        for id in ["downloads-clear", "recent-clear"] {
            let node = frame.access.iter().find(|node| node.id == id).unwrap();
            assert!(node.disabled && !node.actionable, "{id} waits for something to erase");
        }
    }

    #[test]
    fn rows_dispatch_the_existing_settings_actions() {
        let mut state = State::default();
        state.mobile = true;
        state.saved.push(Track { id: "saved".into(), title: "Guardada".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 });
        let mut tree = UiTree::new(page(&state, 390.0));
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(390.0, 4000.0));
        for id in ["setting:connections", "equalizer", "import-open", "downloads-clear", "setting:autoplay", "setting:quality:low", "library-view:grid"] {
            let target = rect(&frame, id);
            let point = limn::Point::new(target.x + target.width / 2.0, target.y + target.height / 2.0);
            tree.handle_event(limn::UiEvent::PointerDown(point));
            assert_eq!(tree.handle_event(limn::UiEvent::PointerUp(point)), vec![limn::UiAction::Clicked { id: id.into() }]);
        }
    }
}
