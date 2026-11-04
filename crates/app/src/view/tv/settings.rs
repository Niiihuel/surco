//! The TV's settings as Apple's grouped lists: one centred column under a large title, each
//! group's rows on a rounded card, what the focused row is for as its group's footnote.
//! Values are written out and change in place on select.

use std::borrow::Cow;

use super::*;
use crate::state::{Quality, CROSSFADE_MAX};

/// A row: what select does, its name, its value written out, and what the left says of it.
struct Row {
    id: &'static str,
    title: Cow<'static, str>,
    value: Cow<'static, str>,
    about: Cow<'static, str>,
}

fn on_off(on: bool) -> Cow<'static, str> {
    if on { t!("common-on") } else { t!("common-off") }
}

fn groups(state: &State) -> Vec<(Cow<'static, str>, Vec<Row>)> {
    let settings = &state.settings;
    let row = |id, title, value, about| Row { id, title, value, about };
    let crossfade = if settings.crossfade == 0.0 { t!("common-off") } else { format!("{:.0} s", settings.crossfade).into() };
    let quality = Quality::ALL.iter().find(|(quality, ..)| *quality == settings.quality).map_or("", |(.., label)| label.text());
    vec![
        (t!("common-settings-playback"), vec![
            row("setting:autoplay", t!("mobile-autoplay"), on_off(settings.autoplay), t!("desktop-autoplay-detail")),
            row(CROSSFADE, t!("desktop-crossfade"), crossfade, t!("desktop-crossfade-detail")),
            row("setting:skip-silence", t!("common-skip-silence"), on_off(settings.skip_silence), t!("common-skip-silence-detail")),
        ]),
        (t!("common-settings-sound"), vec![
            row(QUALITY, t!("mobile-quality"), quality.into(), t!("mobile-quality-note")),
            row("setting:normalize", t!("desktop-normalize"), on_off(settings.normalize), t!("desktop-normalize-detail")),
            row("setting:mono", t!("desktop-mono"), on_off(settings.mono), t!("desktop-mono-detail")),
        ]),
        (t!("mobile-display"), vec![
            row("setting:motion", t!("mobile-reduce-motion"), on_off(settings.reduce_motion), t!("desktop-reduce-motion-detail")),
            row(LANGUAGE, t!("common-language"), crate::i18n::current().name().into(), t!("common-language-detail")),
        ]),
        (t!("desktop-handoff"), vec![
            row(crate::state::tv::RECEIVE, t!("common-tv-handoff-row"), Cow::Borrowed(""), t!("common-tv-handoff-row-detail")),
        ]),
        (t!("mobile-about"), vec![
            row("tv-version", t!("mobile-version"), env!("CARGO_PKG_VERSION").into(), Cow::Borrowed("")),
        ]),
    ]
}

/// The rows that step their value along on select.
pub const CROSSFADE: &str = "tv-setting:crossfade";
pub const QUALITY: &str = "tv-setting:quality";
pub const LANGUAGE: &str = "tv-setting:language";
/// How far each select moves the crossfade, in seconds.
pub const CROSSFADE_STEP: f32 = CROSSFADE_MAX / 4.0;

pub(super) fn page(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let inset = |element: Element| horizontal_inset(element, TEXT_INSET, TEXT_INSET);
    let mut items = vec![
        Element::spacer().size(None, Some(BAR)),
        inset(Text::new(t!("common-settings"), palette).font_size(30.0).line_height(36.0).render().font_family(FontFamily::SansBold)),
        Element::spacer().size(None, Some(22.0)),
    ];
    for (title, rows) in groups(state) {
        let focused = rows.iter().position(|row| lit(state, row.id));
        let mut card = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            // As Apple's, no hairline runs beside the row the focus lifts.
            if index > 0 && focused != Some(index) && focused != Some(index - 1) {
                card.push(horizontal_inset(Element::panel(Element::spacer()).background(HAIRLINE).size(None, Some(1.0)), TEXT_INSET, 0.0));
            } else if index > 0 {
                card.push(Element::spacer().size(None, Some(1.0)));
            }
            card.push(cell(state, row, palette));
        }
        // The footnote keeps its room whether it speaks or not, so nothing moves as the focus does.
        let note = focused.map(|index| rows[index].about.clone()).filter(|about| !about.is_empty()).unwrap_or_default();
        items.extend([
            inset(Text::new(title.to_uppercase(), palette).font_size(12.0).render().font_family(FontFamily::SansSemibold).foreground(SECONDARY)),
            Element::spacer().size(None, Some(7.0)),
            Element::panel(Element::column(card)).background(CARD).radius(CARD_RADIUS),
            Element::spacer().size(None, Some(8.0)),
            {
                let note = inset(Text::new(note, palette).font_size(13.0).line_height(18.0).render().foreground(SECONDARY)).size(None, Some(NOTE));
                if focused.is_some() { note.anchor(NOTE_ANCHOR) } else { note }
            },
            Element::spacer().size(None, Some(14.0)),
        ]);
    }
    let width = COLUMN.min(window.width - 2.0 * SIDE);
    let column = Element::column(items).size(Some(width), None).centered_x();
    ScrollArea::new("tv-settings", column).visibility(UNSEEN_BAR).render().flex(1.0)
}

/// Bringing the account from a phone, as BitChord's TV signs in with one: a code for the
/// phone to read, the steps beside it, and once the account came, what of it to bring in.
pub(super) fn handoff(state: &State, handoff: &crate::state::Handoff, window: Size, palette: ComponentPalette) -> Element {
    let inset = |element: Element| horizontal_inset(element, TEXT_INSET, TEXT_INSET);
    let words = |text: Cow<'static, str>, size: f32| Text::new(text, palette).font_size(size).line_height(size * 1.35).render();
    let mut items = vec![
        Element::spacer().size(None, Some(BAR)),
        inset(Text::new(t!("common-tv-handoff-title"), palette).font_size(30.0).line_height(36.0).render().font_family(FontFamily::SansBold)),
        Element::spacer().size(None, Some(22.0)),
    ];
    let button = |id: &str, label: Cow<'static, str>, main: bool| {
        let on = lit(state, id);
        let (fill, ink) = if on { (PLATTER, PLATTER_INK) } else if main { (Color::rgba(255, 255, 255, 56), palette.text) } else { (CARD, palette.text) };
        let label_text = Text::new(&*label, palette).font_size(15.0).render().font_family(FontFamily::SansMedium).foreground(ink).centered();
        Element::clickable(id, Element::panel(horizontal_inset(label_text, 24.0, 24.0)).background(fill).round().size(None, Some(44.0)))
            .accessible(limn::Access::new(limn::Role::Button).name(&*label)).focus_ring(false).round()
            .animate_scale_about(format!("tv-lift:{id}"), if on { 1.06 } else { 1.0 }, limn::Point::new(0.5, 0.5))
    };
    let mut actions = Vec::new();
    match &handoff.fetched {
        Some(Ok(bundle)) => {
            let parts = handoff.parts;
            let on = |on: bool| if on { t!("common-on") } else { t!("common-off") };
            let mut rows = Vec::new();
            let mut part = |shown: bool, name: &'static str, title: Cow<'static, str>, about: String, taken: bool| {
                if shown { rows.push(Row { id: name, title, value: on(taken), about: about.into() }); }
            };
            part(bundle.profile.is_some(), "handoff-part:profile", t!("desktop-handoff-profile"), bundle.profile.as_ref().map(|profile| profile.shown().to_owned()).unwrap_or_default(), parts.profile);
            part(!bundle.playlists.is_empty(), "handoff-part:playlists", t!("desktop-playlists"), t!("desktop-profile-playlists", count = bundle.playlists.len()).into_owned(), parts.playlists);
            part(!bundle.liked.is_empty(), "handoff-part:liked", t!("common-liked-songs"), songs(bundle.liked.len()), parts.liked);
            part(!bundle.kept.is_empty(), "handoff-part:library", t!("desktop-handoff-library"), t!("desktop-handoff-library-detail").into_owned(), parts.library);
            part(bundle.settings.is_some(), "handoff-part:settings", t!("common-settings"), t!("desktop-handoff-settings-detail").into_owned(), parts.settings);
            part(bundle.equalizer.is_some(), "handoff-part:equalizer", t!("common-equalizer"), String::new(), parts.equalizer);
            part(!bundle.heard.is_empty(), "handoff-part:history", t!("desktop-handoff-history"), songs(bundle.heard.len()), parts.history);
            let card: Vec<Element> = rows.iter().enumerate().flat_map(|(index, row)| {
                let line = (index > 0).then(|| horizontal_inset(Element::panel(Element::spacer()).background(HAIRLINE).size(None, Some(1.0)), TEXT_INSET, 0.0));
                line.into_iter().chain(std::iter::once(cell(state, row, palette)))
            }).collect();
            items.extend([
                inset(Text::new(t!("desktop-handoff-came").to_uppercase(), palette).font_size(12.0).render().font_family(FontFamily::SansSemibold).foreground(SECONDARY)),
                Element::spacer().size(None, Some(7.0)),
                Element::panel(Element::column(card)).background(CARD).radius(CARD_RADIUS),
                Element::spacer().size(None, Some(8.0)),
                inset(words(t!("desktop-handoff-replace-detail"), 13.0).foreground(SECONDARY)),
            ]);
            actions.extend([button("handoff-take", t!("desktop-handoff-take"), true), button("handoff-close", t!("common-cancel"), false)]);
        }
        Some(Err(message)) => {
            items.push(inset(words(message.clone().into(), 15.0).foreground(SECONDARY)));
            actions.extend([button(crate::state::tv::RECEIVE, t!("common-retry"), true), button("handoff-close", t!("common-cancel"), false)]);
        }
        None => {
            // The code on white, as a camera reads it best; pulsing until it is made.
            let tile = match &handoff.offer {
                Some(Ok((_, Some(qr)))) => Element::panel(Element::raster_image(qr.clone()).size(Some(QR - 16.0), Some(QR - 16.0)).centered()).background(Color::WHITE).radius(CARD_RADIUS),
                _ => limn::Skeleton::block("handoff-coming", ComponentPalette { raised: palette.selected, ..palette }, QR, QR).radius(CARD_RADIUS).render(),
            }.size(Some(QR), Some(QR));
            let step = |number: usize, text: Cow<'static, str>| Element::row(vec![
                Element::panel(Text::new(number.to_string(), palette).font_size(13.0).render().font_family(FontFamily::SansSemibold).centered()).background(CARD).round().size(Some(26.0), Some(26.0)),
                words(text, 15.0).foreground(SECONDARY).flex(1.0),
            ]).gap(14.0);
            let steps = match &handoff.offer {
                Some(Err(message)) => vec![words(message.clone().into(), 15.0).foreground(SECONDARY)],
                _ => vec![step(1, t!("common-tv-handoff-step-network")), step(2, t!("common-tv-handoff-step-scan")), step(3, t!("common-tv-handoff-step-send"))],
            };
            items.push(Element::row(vec![tile, Element::column(steps).gap(18.0).centered_y().flex(1.0)]).gap(32.0));
            if matches!(handoff.offer, Some(Err(_))) { actions.push(button(crate::state::tv::RECEIVE, t!("common-retry"), true)); }
            actions.push(button("handoff-close", t!("common-cancel"), false));
        }
    }
    items.extend([Element::spacer().size(None, Some(26.0)), inset(Element::row(actions).gap(14.0))]);
    let width = COLUMN.min(window.width - 2.0 * SIDE);
    ScrollArea::new("tv-handoff", Element::column(items).size(Some(width), None).centered_x()).visibility(UNSEEN_BAR).render().flex(1.0)
}

/// The side of the code's white tile.
const QR: f32 = 200.0;

/// A row on its group's card: its name and its value, the white platter under the focus.
fn cell(state: &State, row: &Row, palette: ComponentPalette) -> Element {
    let on = lit(state, row.id);
    let (ink, soft) = if on { (PLATTER_INK, Color::rgb(80, 80, 86)) } else { (palette.text, SECONDARY) };
    let line = Element::row(vec![
        Text::new(&*row.title, palette).font_size(15.0).truncate().render().font_family(FontFamily::SansMedium).foreground(ink).centered_y().flex(1.0),
        Text::new(&*row.value, palette).font_size(15.0).render().foreground(soft).centered_y(),
    ]).gap(16.0);
    let body = Element::panel(horizontal_inset(line, TEXT_INSET, TEXT_INSET)).background(if on { PLATTER } else { Color::TRANSPARENT }).radius(CARD_RADIUS - 2.0);
    let body = if on { body.shadow(LIFT_SHADOW, limn::Point::new(0.0, 8.0), 20.0, 0.0) } else { body };
    Element::clickable(row.id, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{}, {}", row.title, row.value)).description(&*row.about))
        .focus_ring(false).animate_scale_about(format!("tv-lift:{}", row.id), if on { 1.03 } else { 1.0 }, limn::Point::new(0.5, 0.5))
        .size(None, Some(48.0))
}

/// The column's width at most, the cards' corners and floor, and where their words start.
const COLUMN: f32 = 600.0;
const CARD_RADIUS: f32 = 14.0;
const CARD: Color = Color::rgba(255, 255, 255, 18);
const TEXT_INSET: f32 = 16.0;
/// Two lines of a footnote, and the name of the one that speaks, to bring it into sight.
const NOTE: f32 = 36.0;
pub const NOTE_ANCHOR: &str = "tv-settings-note";

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{UiAction, UiTree};

    #[test]
    fn select_steps_a_value_along_and_the_focused_rows_group_notes_what_it_is_for() {
        let mut state = State::default();
        state.tv = true;
        state.update(&UiAction::Clicked { id: "settings".into() });
        let steps: Vec<f32> = (0..5).map(|_| { state.update(&UiAction::Clicked { id: CROSSFADE.into() }); state.settings.crossfade }).collect();
        assert_eq!(steps, [3.0, 6.0, 9.0, 12.0, 0.0], "round to off again");
        let quality = state.settings.quality;
        state.update(&UiAction::Clicked { id: QUALITY.into() });
        assert_ne!(state.settings.quality, quality);
        state.tv_focused = Some("setting:mono".into());
        let size = Size::new(960.0, 540.0);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.layout(size);
        // As the app does when the focus moves, the footnote is brought into sight.
        tree.reveal(NOTE_ANCHOR);
        let frame = tree.layout(size);
        assert!(frame.access.iter().any(|node| node.id == "setting:mono"));
        let painted: Vec<&str> = frame.commands.iter().filter_map(|command| match command {
            limn::DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }).collect();
        let detail = t!("desktop-mono-detail");
        let first = detail.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        assert!(painted.iter().any(|text| text.starts_with(&first)), "its group's footnote says what mono is for: {painted:?}");
    }
}
