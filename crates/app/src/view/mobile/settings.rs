//! The settings as Spotify lays them out: a list of their parts, each a row with its glyph,
//! what it holds and a chevron, and a page for each part of flat rows under bold headings,
//! the parts of it apart by a hairline. Monochrome, like the app's mark: white for what is on.
//! The desktop lays out its own rows with these same pieces.

use super::*;
use crate::state::SettingsPart;
use limn::Separator;

pub(in crate::view) const GUTTER: f32 = 16.0;
/// The least height of a row, above the 44-point touch minimum.
const ROW_HEIGHT: f32 = 52.0;

/// The sizes of the settings' words and rows: the phone's are touch-sized, the desktop's
/// those of the rest of its pages.
#[derive(Clone, Copy)]
struct Scale { title: f32, detail: f32, heading: f32, page: f32, row: f32, part: f32, glyph: f32 }
const TOUCH: Scale = Scale { title: 17.0, detail: 14.0, heading: 22.0, page: 28.0, row: ROW_HEIGHT, part: 56.0, glyph: 26.0 };
const DENSE: Scale = Scale { title: 15.0, detail: 13.0, heading: 18.0, page: 28.0, row: 44.0, part: 44.0, glyph: 22.0 };

thread_local! {
    static DENSE_PAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While it lives, the settings are drawn at the desktop's sizes.
pub(in crate::view) struct Dense;
impl Dense {
    pub(in crate::view) fn on() -> Self {
        DENSE_PAGE.set(true);
        Self
    }
}
impl Drop for Dense {
    fn drop(&mut self) { DENSE_PAGE.set(false); }
}

fn scale() -> Scale {
    if DENSE_PAGE.get() { DENSE } else { TOUCH }
}
/// A row's inside: 10 above and below, 16 at the sides once `cell` adds its 6.
const ROW_PAD: f32 = 10.0;
/// The room between a part's glyph and its words.
const ROW_GAP: f32 = 16.0;
/// What a row under the pointer or the finger shows: the rows themselves are on the page.
const PRESSED: f32 = 8.0;
/// A segment's height; the track adds 2 on each side, to a 44-point target.
const SEGMENT: f32 = 40.0;
/// The only hue: what cannot be undone.
const DESTRUCTIVE: Color = Color::rgb(255, 69, 58);

/// The name over an optional line about it, and what sits at the end.
pub(in crate::view) fn line(title: &str, detail: Option<&str>, trailing: Option<Element>, ink: Color) -> Element {
    let (palette, scale) = (SCHEME.palette(), scale());
    let mut copy = vec![Text::new(title, palette).font_size(scale.title).ink(ink).render()];
    if let Some(detail) = detail { copy.push(Text::new(detail, palette).font_size(scale.detail).muted().render()); }
    let mut children = vec![Element::column(copy).gap(4.0).centered_y().flex(1.0)];
    children.extend(trailing.map(Element::centered_y));
    Element::row(children).gap(ROW_GAP)
}

pub(in crate::view) fn cell(content: Element) -> Element {
    horizontal_inset(content.min_size(0.0, scale().row).padding(ROW_PAD), GUTTER - ROW_PAD, GUTTER - ROW_PAD)
}

pub(in crate::view) fn value(text: &str) -> Element {
    Text::new(text, SCHEME.palette()).font_size(15.0).muted().render()
}

pub(in crate::view) fn chevron() -> Element {
    let glyph = if DENSE_PAGE.get() { Icon::Custom(crate::view::glyph::CHEVRON_RIGHT) } else { Icon::ChevronRight };
    Element::icon(glyph).foreground(SCHEME.muted)
}

/// Jumping over what is not music in a video, as SponsorBlock's users marked it: a switch for
/// it, and one a category while it is on.
pub(in crate::view) fn sponsorblock(state: &State) -> Vec<Element> {
    let mask = state.settings.sponsorblock;
    let mut rows = vec![switch(&t!("common-sponsorblock-title"), Some(&t!("common-sponsorblock-detail")), "setting:sponsorblock", mask != 0)];
    if mask != 0 {
        rows.extend(crate::sponsorblock::CATEGORIES.iter().enumerate().map(|(bit, (name, key))|
            switch(&crate::i18n::text(crate::i18n::current(), key, None), None, &format!("setting:sponsorblock:{name}"), mask & (1 << bit) != 0)));
    }
    rows
}

/// Last.fm and ListenBrainz: who is signed in, and what waits to be sent; or the fields to sign
/// in with, the password never kept.
pub(in crate::view) fn scrobbling(state: &State, width: f32) -> Element {
    use crate::scrobble::Service;
    let palette = SCHEME.on_raised();
    let form = &state.scrobble_form;
    let white = Color::WHITE;
    let field = |id: &str, value: &str, placeholder: std::borrow::Cow<'static, str>, hidden: bool|
        TextField::new(id, value, "", palette).label(placeholder).password(hidden).width(width).render();
    let connect = |id: &str, busy: bool, ready: bool| super::super::card_button(id, if busy { t!("common-scrobble-connecting") } else { t!("common-scrobble-connect") }, true)
        .sizing(super::super::CARD_BUTTON_SMALL).enabled(ready && form.busy.is_none()).render();
    let signed = |id: &str, name: &str, user: &str, waiting: usize| {
        let detail = if waiting > 0 { t!("common-scrobble-signed-waiting", user = user, count = waiting) } else { t!("common-scrobble-signed", user = user) };
        action(id, name, Some(&detail), Some(value(&t!("common-scrobble-sign-out"))), true, true)
    };
    let mut rows = vec![match &state.scrobbling.listenbrainz {
        Some(account) => signed("scrobble:listenbrainz-out", "ListenBrainz", &account.user, account.waiting.len()),
        None => stacked(line("ListenBrainz", Some(&t!("common-scrobble-listenbrainz-detail")), None, white), Element::column(vec![
            field("scrobble-token", &form.token, t!("common-scrobble-token"), true),
            Element::row(vec![connect("scrobble:listenbrainz", form.busy == Some(Service::ListenBrainz), !form.token.trim().is_empty())]),
        ]).gap(8.0)),
    }];
    rows.push(match &state.scrobbling.lastfm {
        Some(account) => signed("scrobble:lastfm-out", "Last.fm", &account.user, account.waiting.len()),
        None => {
            let ready = [&form.key, &form.secret, &form.user, &form.password].iter().all(|field| !field.trim().is_empty());
            stacked(line("Last.fm", Some(&t!("common-scrobble-lastfm-detail")), None, white), Element::column(vec![
                field("scrobble-key", &form.key, t!("common-scrobble-key"), false),
                field("scrobble-secret", &form.secret, t!("common-scrobble-secret"), true),
                field("scrobble-user", &form.user, t!("common-scrobble-user"), false),
                field("scrobble-password", &form.password, t!("common-scrobble-password"), true),
                Element::row(vec![connect("scrobble:lastfm", form.busy == Some(Service::LastFm), ready)]),
            ]).gap(8.0))
        }
    });
    if let Some(why) = &form.error {
        rows.push(Alert::new(t!("common-scrobble-failed"), why, SCHEME.palette()).id("scrobble-error").compact(true).render());
    }
    group(&t!("common-scrobble"), rows, &t!("common-scrobble-footer"))
}

pub(in crate::view) fn switch(title: &str, detail: Option<&str>, id: &str, on: bool) -> Element {
    // Monochrome: a white track with a dark thumb when on, a white thumb on gray when off.
    let track = ComponentPalette { surface: if on { SCHEME.canvas } else { Color::WHITE }, selected: SCHEME.control.border, ..SCHEME.on_raised() };
    let control = Switch::new(id, on, track).ink(Color::WHITE).size(ControlSize::Spacious).accessibility_label(title).render();
    cell(line(title, detail, Some(control), Color::WHITE))
}

/// A whole row that does one thing; a destructive one is named in red. A disabled one keeps
/// its id, passive and muted.
pub(in crate::view) fn action(id: &str, title: &str, detail: Option<&str>, trailing: Option<Element>, enabled: bool, destructive: bool) -> Element {
    let ink = if !enabled { SCHEME.muted } else if destructive { DESTRUCTIVE } else { Color::WHITE };
    pressable(id, title, detail, cell(line(title, detail, trailing, ink)), enabled)
}

/// A row that answers the pointer and the finger over the page's own floor.
fn pressable(id: &str, title: &str, detail: Option<&str>, row: Element, enabled: bool) -> Element {
    let mut said = limn::Access::new(limn::Role::Button).name(title);
    if let Some(detail) = detail { said = said.description(detail); }
    if !enabled { return row.accessible(said.id(id).disabled(true)); }
    ComponentPalette { surface: SCHEME.canvas, hover: SCHEME.selected, selected: SCHEME.control.pressed, ..SCHEME.palette() }
        .bar_item_surface(Element::clickable(id, row).accessible(said)).radius(PRESSED)
}

/// A row whose control needs the page's width: its line, then the control under it.
pub(in crate::view) fn stacked(head: Element, control: Element) -> Element {
    cell(Element::column(vec![head, control]).gap(12.0))
}

/// A pill-shaped choice among a few: the chosen one white with dark words.
pub(in crate::view) fn segmented(group: &str, width: f32, tabs: Vec<Tab>) -> Element {
    let palette = ComponentPalette { surface: SCHEME.selected, selected: Color::WHITE, ..SCHEME.palette() };
    Tabs::new(tabs, palette).id(group).height(SEGMENT).accent(SCHEME.canvas).radius(SEGMENT / 2.0).fill_width(width).render()
}

/// A bold heading over its rows, flat on the page, and a note only when one helps.
pub(in crate::view) fn group(title: &str, rows: Vec<Element>, footer: &str) -> Element {
    let palette = SCHEME.palette();
    let rows = Element::column(rows).gap(4.0)
        .accessible(limn::Access::new(limn::Role::Group).id(format!("settings-group:{title}")).name(title));
    let caption = |text: Element| horizontal_inset(text, GUTTER, GUTTER);
    let heading = Text::new(title, palette).role(TextRole::Heading).font_size(scale().heading).render().font_family(FontFamily::SansBold);
    let mut parts = vec![caption(heading), rows];
    if !footer.is_empty() { parts.push(caption(Text::new(footer, palette).font_size(13.0).muted().render())); }
    Element::column(parts).gap(10.0)
}

/// A part's name, which titles its page.
pub(in crate::view) fn part_title(part: SettingsPart) -> std::borrow::Cow<'static, str> {
    match part {
        SettingsPart::Playback => t!("common-settings-playback"),
        SettingsPart::Audio => t!("common-settings-audio"),
        SettingsPart::Devices => t!("common-settings-devices"),
        SettingsPart::Display => t!("common-settings-display"),
        SettingsPart::Library => t!("common-settings-library"),
        SettingsPart::Connected => t!("common-settings-connected"),
        SettingsPart::About => t!("common-settings-about"),
        SettingsPart::Storage => t!("common-settings-storage"),
    }
}

/// What a part holds, said in its row of the list: a few of its settings between dots.
fn part_about(part: SettingsPart) -> std::borrow::Cow<'static, str> {
    match part {
        SettingsPart::Playback => t!("common-settings-playback-about"),
        SettingsPart::Audio => t!("common-settings-audio-about"),
        SettingsPart::Devices => t!("common-settings-devices-about"),
        SettingsPart::Display => t!("common-settings-display-about"),
        SettingsPart::Library => t!("common-settings-library-about"),
        SettingsPart::Connected => t!("common-settings-connected-about"),
        SettingsPart::About => t!("common-settings-about-about", version = env!("CARGO_PKG_VERSION")),
        SettingsPart::Storage => t!("common-settings-storage-about"),
    }
}

/// Accounts elsewhere: two points joined to a third.
const CONNECTED: &str = include_str!("../../../assets/icons/share-2.svg");

fn part_glyph(part: SettingsPart) -> Icon {
    // A desktop draws Surco's own glyphs; a phone keeps the platform's.
    if DENSE_PAGE.get() {
        use crate::view::glyph;
        return Icon::Custom(match part {
            SettingsPart::Playback => glyph::VOLUME_HIGH,
            SettingsPart::Audio => glyph::EQUALIZER,
            SettingsPart::Devices => glyph::DEVICE,
            SettingsPart::Display => glyph::DISPLAY,
            SettingsPart::Library => glyph::LIBRARY,
            SettingsPart::Connected => CONNECTED,
            SettingsPart::About => glyph::INFO,
            SettingsPart::Storage => glyph::DOWNLOAD,
        });
    }
    match part {
        SettingsPart::Playback => Icon::Volume,
        SettingsPart::Audio => Icon::ChartBar,
        SettingsPart::Devices => Icon::Custom(AUDIO_ROUTE),
        SettingsPart::Display => Icon::Palette,
        SettingsPart::Library => Icon::Playlist,
        SettingsPart::Connected => Icon::Custom(CONNECTED),
        SettingsPart::About => Icon::InfoCircle,
        SettingsPart::Storage => Icon::Custom(include_str!("../../../assets/icons/download.svg")),
    }
}

/// The settings page's rows: on the list of parts, a row for each; on a part's page, its
/// groups apart by hairlines. `sections` are every group with the part it belongs to. The
/// desktop titles the page itself; the phone's header does.
pub(in crate::view) fn arrange(state: &State, sections: Vec<(SettingsPart, Element)>, titled: bool, palette: ComponentPalette) -> Vec<Element> {
    let scale = scale();
    let title = |text: &str| horizontal_inset(Text::new(text, palette).role(TextRole::Heading).font_size(scale.page).render().font_family(FontFamily::SansBold), GUTTER, GUTTER);
    let Page::SettingsPart(shown) = state.page else {
        let rows = SettingsPart::ALL.into_iter().map(|part| {
            let (name, about) = (part_title(part), part_about(part));
            let glyph = Element::icon(part_glyph(part)).large_icon(scale.glyph).foreground(Color::WHITE).size(Some(scale.glyph), Some(scale.glyph));
            let row = Element::row(vec![glyph.centered_y(), line(&name, Some(&about), Some(chevron()), Color::WHITE).flex(1.0)]).gap(ROW_GAP);
            pressable(&format!("settings-part:{}", part.slug()), &name, Some(&about), cell(row.min_size(0.0, scale.part)), true)
        });
        let mut page: Vec<Element> = titled.then(|| title(&t!("common-settings"))).into_iter().collect();
        page.push(Element::column(rows.collect()).gap(4.0));
        return page;
    };
    let mut page: Vec<Element> = titled.then(|| title(&part_title(shown))).into_iter().collect();
    for (index, (_, group)) in sections.into_iter().filter(|(part, _)| *part == shown).enumerate() {
        if index > 0 { page.push(Separator::horizontal(palette).render()); }
        page.push(group);
    }
    page
}

pub(in crate::view) fn page(state: &State, width: f32) -> Element {
    // The card's inside, for controls that span it.
    let inner = (width - GUTTER * 4.0).max(0.0);
    let white = Color::WHITE;
    let seconds = state.settings.crossfade;
    let crossfade = stacked(
        line("Crossfade", None, Some(value(&if seconds == 0.0 { t!("mobile-crossfade-off").into_owned() } else { format!("{seconds:.0} s") })), white),
        Slider::new("setting:crossfade", seconds / CROSSFADE_MAX, SCHEME.on_raised()).steps(CROSSFADE_MAX as usize + 1).width(inner).render(),
    );
    let transitions = group(&t!("common-settings-transitions"), vec![crossfade], "");
    let mut controls = vec![switch(&t!("mobile-autoplay"), Some(&t!("desktop-autoplay-detail")), "setting:autoplay", state.settings.autoplay)];
    controls.extend(sponsorblock(state));
    let controls = group(&t!("common-settings-controls"), controls, &t!("common-sponsorblock-footer"));

    let mut connections = group(&t!("mobile-connections"), vec![
        action("setting:connections", &t!("mobile-listen-elsewhere"), None, Some(chevron()), true, false),
    ], "");
    if let Some(notice) = &state.output_notice {
        connections = Element::column(vec![connections, Alert::new(t!("mobile-audio-output"), notice, SCHEME.palette()).id("setting:output-notice").compact(true).render()]).gap(8.0);
    }

    let chosen = state.settings.quality;
    let kbps: u32 = match chosen { Quality::Low => 50, Quality::Normal => 70, Quality::High => 140 };
    let ceiling = t!("mobile-quality-ceiling", kbps = kbps);
    let quality = stacked(
        line(&t!("mobile-quality"), None, Some(value(&ceiling)), white),
        segmented("setting-quality", inner, Quality::ALL.into_iter()
            .map(|(quality, name, label)| Tab::new(format!("setting:quality:{name}"), label, 0.0).active(quality == chosen)).collect()),
    );
    let equalizer = super::super::equalizer::summary(state);
    let streaming = group(&t!("common-settings-streaming"), vec![quality], &t!("mobile-quality-note"));
    let sound = group(&t!("common-settings-sound"), vec![
        switch(&t!("mobile-normalize"), None, "setting:normalize", state.settings.normalize),
        switch(&t!("mobile-mono"), None, "setting:mono", state.settings.mono),
        switch(&t!("common-skip-silence"), Some(&t!("common-skip-silence-detail")), "setting:skip-silence", state.settings.skip_silence),
        action("equalizer", &t!("common-equalizer"), None,
            Some(Element::row(vec![value(&equalizer).centered_y(), chevron().centered_y()]).gap(6.0)), true, false),
    ], "");

    let display = group(&t!("mobile-display"), vec![
        switch(&t!("mobile-reduce-motion"), Some(&t!("desktop-reduce-motion-detail")), "setting:motion", state.settings.reduce_motion),
    ], "");
    let language = group(&t!("common-language"), vec![
        stacked(line(&t!("common-language"), None, None, white), segmented("setting-language", inner, Language::ALL.into_iter()
            .map(|language| Tab::new(format!("setting:language:{}", language.tag()), language.name(), 0.0).active(language == crate::i18n::current())).collect())),
    ], "");

    let sort = state.settings.library_sort;
    let view = state.settings.library_view;
    let library = group(&t!("common-settings-your-library"), vec![
        stacked(line(&t!("mobile-sort"), None, None, white), segmented("setting-library-sort", inner, vec![
            Tab::new("library-sort:added", t!("mobile-sort-recent"), 0.0).active(sort == Sort::Added),
            Tab::new("library-sort:alphabetical", "A–Z", 0.0).active(sort == Sort::Alphabetical),
            Tab::new("library-sort:creator", t!("common-creator"), 0.0).active(sort == Sort::Creator),
        ])),
        stacked(line(&t!("mobile-view"), None, None, white), segmented("setting-library-view", inner, vec![
            Tab::new("library-view:compact", t!("mobile-view-compact"), 0.0).active(view == LibraryView::Compact),
            Tab::new("library-view:list", t!("mobile-view-list"), 0.0).active(view == LibraryView::List),
            Tab::new("library-view:grid", t!("mobile-view-grid"), 0.0).active(view == LibraryView::Grid),
        ])),
    ], "");
    let bring = group(&t!("common-settings-bring"), vec![
        action("import-open", &t!("mobile-import-music"), None, Some(chevron()), true, false),
        action("account-handoff", &t!("desktop-handoff"), None, Some(chevron()), true, false),
        action("onboarding-taste", &t!("common-onboarding-taste"), None, Some(chevron()), true, false),
    ], "");

    let about = group(&t!("mobile-about"), vec![
        cell(line(&t!("mobile-version"), None, Some(value(env!("CARGO_PKG_VERSION"))), white)),
        action("patches", &t!("common-patches"), None, Some(chevron()), true, false),
        cell(line(&t!("mobile-licenses"), Some(&t!("mobile-licenses-detail")), None, white)),
    ], "");

    // What cannot be undone sits last, each behind a confirmation.
    let downloads = format!("{} · {}", songs(state.saved.len()), megabytes(state.saved_bytes));
    let erase = group(&t!("mobile-storage"), vec![
        action("downloads-clear", &t!("mobile-delete-downloads"), Some(&downloads), None, !state.saved.is_empty(), true),
        action("recent-clear", &t!("mobile-clear-recent"), Some(&t!("mobile-clear-recent-detail")), None, !state.collection.recent.is_empty(), true),
    ], "");

    use SettingsPart::*;
    let sections = vec![(Playback, transitions), (Playback, controls), (Devices, connections), (Audio, streaming), (Audio, sound),
        (Display, display), (Display, language), (Library, library), (Library, bring), (Connected, scrobbling(state, inner)), (About, about), (Storage, erase)];
    ScrollArea::new("settings", Element::column(arrange(state, sections, false, SCHEME.palette())).gap(24.0).padding(GUTTER)).render().flex(1.0)
}

/// The equaliser as a page of the same cards: the ready-made curves, the curve its filters
/// draw over a slider for each band, and the preamp with the reset.
pub(in crate::view) fn equalizer(state: &State, width: f32) -> Element {
    use super::super::equalizer as parts;
    let palette = SCHEME.palette();
    let inner = (width - GUTTER * 4.0).max(0.0);
    let heading = |title: &str| horizontal_inset(Text::new(title, palette).role(TextRole::Heading).font_size(17.0).render().font_family(FontFamily::SansBold), GUTTER, GUTTER);
    let presets = Element::column(vec![heading(&t!("mobile-eq-presets")), parts::presets(state, 44.0)]).gap(8.0);
    // Columns as wide as the card allows: about 34 points each on a 390-point phone.
    let response = Element::column(vec![parts::curve(state, 150.0, SCHEME.canvas), parts::bands(state, 170.0)]).gap(12.0).padding(6.0);
    let response = group(&t!("mobile-eq-bands"), vec![response.padding(4.0)], "");
    let preamp = stacked(
        line(&t!("mobile-eq-preamp"), None, Some(value(&parts::decibels(state.equalizer.preamp_db))), Color::WHITE),
        parts::preamp(state, Some(inner)).size(None, Some(44.0)),
    );
    let flat = state.equalizer == surco_audio::equalizer::EqualizerValues::FLAT;
    let reset = action("equalizer-reset", &t!("mobile-eq-reset"), Some(&t!("mobile-eq-reset-detail")), None, !flat, false);
    let preamp = group(&t!("mobile-eq-preamp"), vec![preamp, reset], &t!("mobile-eq-preamp-note"));
    ScrollArea::new("equalizer", Element::column(vec![presets, response, preamp]).gap(28.0).padding(GUTTER)).render().flex(1.0)
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

    /// The list of parts and then each part's page, as the phone shows them.
    fn every_page(state: &mut State, width: f32) -> Vec<Frame> {
        std::iter::once(Page::Settings).chain(SettingsPart::ALL.map(Page::SettingsPart)).map(|page| {
            state.page = page;
            frame(state, width)
        }).collect()
    }

    fn rect(frames: &[Frame], id: &str) -> Rect {
        frames.iter().find_map(|frame| frame.access.iter().find(|node| node.id == id)).unwrap_or_else(|| panic!("{id} is shown")).rect
    }

    #[test]
    fn every_setting_is_a_full_row_inside_the_page_at_phone_widths() {
        let mut state = State::default();
        state.mobile = true;
        for width in [320.0, 390.0, 430.0] {
            let frames = every_page(&mut state, width);
            for id in ROWS.into_iter().chain(["recent-clear", "setting:quality:low", "library-sort:added", "library-view:compact"]) {
                let target = rect(&frames, id);
                assert!(target.x >= GUTTER - 0.01 && target.x + target.width <= width - GUTTER + 0.01, "{id} keeps the 16-point margins at {width}: {target:?}");
            }
            for id in ["setting:connections", "equalizer", "import-open", "downloads-clear", "recent-clear", "settings-part:playback", "settings-part:storage"] {
                let row = rect(&frames, id);
                assert!(row.height >= 44.0, "{id} is a full touch row at {width}: {row:?}");
                assert!((row.width - (width - GUTTER * 2.0)).abs() < 0.01, "{id} spans the page at {width}: {row:?}");
            }
            for id in ["setting:quality:normal", "library-sort:alphabetical", "library-view:list"] {
                assert!(rect(&frames, id).height >= 39.99, "{id} is a comfortable segment at {width}: {:?}", rect(&frames, id));
            }
            for id in ["setting:autoplay", "setting:normalize", "setting:mono", "setting:motion"] {
                assert!(rect(&frames, id).width >= 44.0, "{id} is easy to toggle at {width}");
            }
            // Switches sit in their own rows: two in a row are a full row apart.
            assert!(rect(&frames, "setting:mono").y - rect(&frames, "setting:normalize").y >= ROW_HEIGHT);
        }
    }

    #[test]
    fn the_whole_phone_page_keeps_rows_inside_the_safe_area() {
        for (width, left, right) in [(320.0, 0.0, 0.0), (390.0, 0.0, 0.0), (430.0, 0.0, 0.0), (874.0, 62.0, 34.0)] {
            let mut state = State::default();
            state.mobile = true;
            state.mobile_ui.content_insets = crate::state::mobile::ContentInsets { top: 62.0, bottom: 34.0, left, right };
            let frames: Vec<Frame> = std::iter::once(Page::Settings).chain(SettingsPart::ALL.map(Page::SettingsPart)).map(|page| {
                state.page = page;
                let mut tree = UiTree::new(super::super::view(&state, Size::new(width, 6000.0)));
                tree.set_reduced_motion(true);
                tree.layout(Size::new(width, 6000.0))
            }).collect();
            for id in ROWS.into_iter().chain(["settings-part:playback"]) {
                let target = rect(&frames, id);
                assert!(target.x >= left + GUTTER - 0.01 && target.x + target.width <= width - right - GUTTER + 0.01, "{id} clears the insets at {width}: {target:?}");
            }
        }
    }

    #[test]
    fn the_list_of_parts_leads_to_flat_monochrome_pages_and_what_cannot_be_undone_comes_last() {
        let mut state = State::default();
        state.mobile = true;
        let frames = every_page(&mut state, 390.0);
        let parts: Vec<_> = frames[0].access.iter().filter(|node| node.id.starts_with("settings-part:")).map(|node| node.id.as_str()).collect();
        assert_eq!(parts.len(), 8);
        assert_eq!(parts.last(), Some(&"settings-part:storage"), "erasing is the last part");
        assert!(ROWS.iter().all(|id| !frames[0].access.iter().any(|node| node.id == *id)), "the list holds the parts, not their settings");
        let groups = frames[1..].iter().flat_map(|frame| frame.access.iter().filter(|node| node.id.starts_with("settings-group:"))).count();
        assert_eq!(groups, 12);
        // No hue but the destructive red, and no cards: the rows are on the page.
        for frame in &frames {
            for command in &frame.commands {
                if let DrawCommand::FillRect { color, .. } = command {
                    assert!(color.r == color.g && color.g == color.b, "a gray fill, not a tint: {color:?}");
                }
            }
        }
        // With nothing to erase, both stay listed but cannot be pressed.
        for id in ["downloads-clear", "recent-clear"] {
            let node = frames.last().unwrap().access.iter().find(|node| node.id == id).unwrap();
            assert!(node.disabled && !node.actionable, "{id} waits for something to erase");
        }
    }

    #[test]
    fn rows_dispatch_the_existing_settings_actions() {
        let mut state = State::default();
        state.mobile = true;
        state.saved.push(Track { id: "saved".into(), title: "Guardada".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 });
        for (page, ids) in [
            (Page::Settings, vec!["settings-part:audio", "settings-part:storage"]),
            (Page::SettingsPart(SettingsPart::Devices), vec!["setting:connections"]),
            (Page::SettingsPart(SettingsPart::Audio), vec!["equalizer", "setting:quality:low"]),
            (Page::SettingsPart(SettingsPart::Library), vec!["import-open", "library-view:grid"]),
            (Page::SettingsPart(SettingsPart::Storage), vec!["downloads-clear"]),
            (Page::SettingsPart(SettingsPart::Playback), vec!["setting:autoplay"]),
        ] {
            state.page = page;
            let mut tree = UiTree::new(page_of(&state));
            tree.set_reduced_motion(true);
            let frame = tree.layout(Size::new(390.0, 4000.0));
            for id in ids {
                let target = rect(std::slice::from_ref(&frame), id);
                let point = limn::Point::new(target.x + target.width / 2.0, target.y + target.height / 2.0);
                tree.handle_event(limn::UiEvent::PointerDown(point));
                assert_eq!(tree.handle_event(limn::UiEvent::PointerUp(point)), vec![limn::UiAction::Clicked { id: id.into() }]);
            }
        }
    }

    fn page_of(state: &State) -> Element {
        page(state, 390.0)
    }

    #[test]
    fn a_part_row_opens_its_page_and_back_returns_to_the_list() {
        let mut state = State::default();
        state.mobile = true;
        state.update(&limn::UiAction::Clicked { id: "settings".into() });
        state.update(&limn::UiAction::Clicked { id: "settings-part:about".into() });
        assert_eq!(state.page, Page::SettingsPart(SettingsPart::About));
        assert_eq!(super::super::title(&state), "Acerca de Surco");
        state.update(&limn::UiAction::Clicked { id: "back".into() });
        assert_eq!(state.page, Page::Settings);
    }
}
