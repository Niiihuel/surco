//! The first run's steps over the whole window, as Spotify's sign-up: one question at a time
//! in the middle, a white pill to go on, and a picker of round faces for the artists.

use super::*;
use crate::state::onboarding::{Onboarding, Step, PICKS_FEWEST};
use surco_core::ArtistRef;

thread_local! {
    /// Whether the steps are drawn for a phone, for the frame being built.
    static PHONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The widest a step's column grows, and the picker's.
const COLUMN: f32 = 480.0;
const PICKER_COLUMN: f32 = 760.0;
/// A face in the picker at its smallest, and the space between faces.
const FACE_FEWEST: f32 = 120.0;
const FACE_GAP: f32 = 16.0;
/// The disc with a check on a face picked.
const PICKED: f32 = 28.0;
/// The white pill that goes on.
const PILL: f32 = 48.0;

pub(super) fn view(state: &State, onboarding: &Onboarding, window: Size) -> Element {
    let palette = SCHEME.palette();
    let phone = state.mobile || window.width < 600.0;
    PHONE.set(phone);
    let margin = if phone { 24.0 } else { 32.0 };
    let insets = if state.mobile { state.mobile_ui.content_insets } else { Default::default() };
    let most = if onboarding.step == Step::Artists { PICKER_COLUMN } else { COLUMN };
    let width = (window.width - insets.left - insets.right - 2.0 * margin).min(most).max(200.0);
    let height = (window.height - insets.top - insets.bottom - 2.0 * margin).max(200.0);

    let body = match onboarding.step {
        Step::Welcome => welcome(palette),
        Step::Bring => bring(state, width, palette),
        Step::Name => name(onboarding, width, palette),
        Step::Genres => genres(onboarding, palette),
        Step::Artists => artists(state, onboarding, width, palette),
        Step::Done => done(state, onboarding, palette),
    };
    // Each step comes in as the last leaves, as the steps of Spotify's.
    let body = body.animate_enter(format!("onboarding:{:?}", onboarding.step));
    let column = Element::column(vec![header(onboarding, palette), body.flex(1.0)]).size(Some(width), Some(height));
    // In the middle of the window, clear of a phone's notch and home bar.
    let middle = Element::column(vec![
        Element::spacer().size(None, Some(insets.top + margin)),
        Element::row(vec![Element::spacer().flex(1.0), column, Element::spacer().flex(1.0)]),
    ]);
    // Each step under a colour of its own that gives way to black, as a page under its cover's.
    let hue = match onboarding.step {
        Step::Welcome => 3,
        Step::Bring => 2,
        Step::Name => 5,
        Step::Genres => 0,
        Step::Artists => 4,
        Step::Done => 2,
    };
    Element::panel(middle).background(SCHEME.canvas).size(Some(window.width), Some(window.height))
        .top_background_fade(Color { a: 150, ..MIX_STRIPES[hue] }, window.height * 0.5)
}

/// The way back, where the steps are, and the way past one that can be left out.
fn header(onboarding: &Onboarding, palette: ComponentPalette) -> Element {
    let step = onboarding.step;
    let back = matches!(step, Step::Bring | Step::Name | Step::Artists) || (step == Step::Genres && !onboarding.taste_only);
    let mut row = vec![if back {
        header_button("onboarding-back", Icon::Custom(glyph::CHEVRON_LEFT), TooltipSpec::new(t!("common-back")), palette).render().centered_y()
    } else {
        Element::spacer().size(Some(32.0), None)
    }];
    row.push(Element::spacer().flex(1.0));
    // The dots of the steps a fresh start goes through.
    let steps: &[Step] = if onboarding.taste_only { &[Step::Genres, Step::Artists] } else { &[Step::Name, Step::Genres, Step::Artists] };
    if steps.contains(&step) {
        let dots = steps.iter().map(|each| {
            let ink = if *each == step { palette.text } else { Color { a: 90, ..palette.text } };
            Element::spacer().size(Some(8.0), Some(8.0)).background(ink).radius(4.0)
        }).collect();
        row.push(Element::row(dots).gap(8.0).centered_y());
    }
    row.push(Element::spacer().flex(1.0));
    // A word that lights under the pointer, as Spotify's.
    row.push(if matches!(step, Step::Genres | Step::Artists) {
        Element::button("onboarding-skip", t!("common-onboarding-skip")).font_family(FontFamily::SansBold).font_size(14.0)
            .foreground(palette.muted).hover_foreground(palette.text).pressed_foreground(palette.text).padding(8.0).centered_y()
    } else {
        Element::spacer().size(Some(32.0), None)
    });
    Element::row(row).size(None, Some(64.0))
}

/// A step's question and what it is for.
fn heading(title: &str, detail: Option<&str>, palette: ComponentPalette) -> Element {
    // A phone's column is narrow: its question a step smaller, so its words fit.
    let size = if PHONE.get() { 26.0 } else { 32.0 };
    let mut copy = vec![Text::new(title, palette).font_size(size).render().font_family(FontFamily::SansBold)];
    copy.extend(detail.map(|detail| Text::new(detail, palette).font_size(16.0).muted().render()));
    Element::column(copy).gap(8.0)
}

/// The first step's and the last's heading, in the middle as the rest of them is.
fn centered_heading(title: &str, detail: Option<&str>, palette: ComponentPalette) -> Element {
    let size = if PHONE.get() { 26.0 } else { 32.0 };
    let mut copy = vec![Text::new(title, palette).font_size(size).render().font_family(FontFamily::SansBold).center_content()];
    copy.extend(detail.map(|detail| Text::new(detail, palette).font_size(16.0).muted().render().center_content()));
    Element::column(copy).gap(8.0)
}

/// The white pill that goes on, as Spotify's; muted until there is something to go on with.
fn go_on(id: &str, label: &str, enabled: bool, width: f32, palette: ComponentPalette) -> Element {
    let white = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    Button::new(id, label, white).variant(ButtonVariant::Subtle).sizing(limn::ButtonSizing { height: PILL, font_size: 16.0, ..ControlSize::Spacious.button_sizing() })
        .round().enabled(enabled).width(width).render().center_content()
}

/// The width of the pill that goes on, and of the welcome's two.
const GO_ON: f32 = 200.0;
const WELCOME_PILL: f32 = 320.0;

fn welcome(palette: ComponentPalette) -> Element {
    let bring = Button::new("onboarding-bring", t!("common-onboarding-bring"), palette).variant(ButtonVariant::Outline)
        .sizing(limn::ButtonSizing { height: PILL, font_size: 16.0, ..ControlSize::Spacious.button_sizing() }).round().width(WELCOME_PILL).render().center_content();
    Element::column(vec![
        Element::spacer().flex(1.0),
        centered_heading(&t!("common-onboarding-welcome"), Some(&t!("common-onboarding-welcome-detail")), palette),
        Element::spacer().size(None, Some(24.0)),
        go_on("onboarding-fresh", &t!("common-onboarding-fresh"), true, WELCOME_PILL, palette).centered_x(),
        bring.centered_x(),
        Element::spacer().flex(2.0),
    ]).gap(12.0)
}

/// An account from another device: by the phone's camera, or by the address the other
/// device shows; then what of it comes.
fn bring(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut parts = vec![
        Element::spacer().size(None, Some(24.0)),
        heading(&t!("common-onboarding-bring-title"), Some(&t!("common-onboarding-bring-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
    ];
    let Some(handoff) = &state.handoff else { return Element::column(parts) };
    if state.mobile || cfg!(target_os = "ios") {
        parts.push(go_on("handoff-scan", &t!("common-scan-code"), true, width.min(WELCOME_PILL), palette));
        parts.push(Text::new(t!("common-onboarding-bring-type"), palette).muted().render());
    }
    let field = ComponentPalette { raised: CHOSEN, border: Color::rgb(124, 124, 124), ..palette };
    let address = TextField::new("handoff-address", &handoff.address, "192.168.1.20:48213/…", field).size(ControlSize::Spacious).render().size(None, Some(PILL)).flex(1.0);
    let look = Button::new("handoff-fetch", t!("desktop-handoff-look"), palette).variant(ButtonVariant::Outline)
        .sizing(limn::ButtonSizing { height: PILL, font_size: 16.0, ..ControlSize::Spacious.button_sizing() }).round().enabled(!handoff.fetching).render();
    parts.push(Element::row(vec![address, look]).gap(8.0));
    if handoff.fetching {
        parts.push(Element::row(vec![Spinner::new("onboarding-fetching", palette.muted).size(16.0).render(), Text::new(t!("desktop-handoff-fetching"), palette).muted().render()]).gap(8.0));
    }
    match &handoff.fetched {
        Some(Err(message)) => parts.push(Alert::new(t!("desktop-handoff"), message, palette).id("handoff-error").compact(true).render()),
        Some(Ok(bundle)) => {
            parts.push(Element::spacer().size(None, Some(8.0)));
            parts.push(Text::new(t!("desktop-handoff-came"), palette).muted().render());
            let rows = Element::column(came(bundle, handoff.parts));
            parts.push(ScrollArea::new("onboarding-came", rows).appearance(spotify_bar()).visibility(SPOTIFY_BAR_SHOWS).render().flex(1.0));
            parts.push(Element::row(vec![Element::spacer().flex(1.0), go_on("handoff-take", &t!("desktop-handoff-take"), true, GO_ON, palette)]).padding(16.0));
        }
        None => {}
    }
    Element::column(parts).gap(8.0)
}

fn name(onboarding: &Onboarding, width: f32, palette: ComponentPalette) -> Element {
    // A field of Spotify's sign-up: a darker fill and a grey ring.
    let field = ComponentPalette { raised: CHOSEN, border: Color::rgb(124, 124, 124), ..palette };
    let input = TextField::new("onboarding-name", &onboarding.name, t!("common-onboarding-name-placeholder"), field).size(ControlSize::Spacious).width(width).render().size(Some(width), Some(PILL));
    Element::column(vec![
        Element::spacer().size(None, Some(24.0)),
        heading(&t!("common-onboarding-name"), Some(&t!("common-onboarding-name-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
        input,
        Element::spacer().size(None, Some(16.0)),
        go_on("onboarding-next", &t!("common-onboarding-next"), !onboarding.name.trim().is_empty(), GO_ON, palette),
    ]).gap(8.0)
}

fn genres(onboarding: &Onboarding, palette: ComponentPalette) -> Element {
    // Chips as the home's: chosen, they turn white and carry a check.
    let chips = mobile::search::genres().map(|(label, query)| {
        let chosen = onboarding.genres.iter().any(|genre| genre == query);
        // The browse cards break a long genre over two lines; a chip keeps it on one.
        let label = label.replace('\n', " ");
        let chip = Button::new(format!("onboarding-genre:{query}"), label, palette)
            .variant(if chosen { ButtonVariant::Default } else { ButtonVariant::Secondary })
            .size(ControlSize::Regular).round();
        if chosen { chip.icon(Icon::Check) } else { chip }.render()
    }).collect();
    let chips = ScrollArea::new("onboarding-genres", Element::row(chips).gap(8.0).line_gap(8.0).wrap()).appearance(spotify_bar()).visibility(SPOTIFY_BAR_SHOWS).render().flex(1.0);
    let label = if onboarding.genres.is_empty() { t!("common-onboarding-skip") } else { t!("common-onboarding-next") };
    Element::column(vec![
        Element::spacer().size(None, Some(24.0)),
        heading(&t!("common-onboarding-genres"), Some(&t!("common-onboarding-genres-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
        chips,
        // Not the header's "onboarding-skip": a control's fill follows its id, so the two would share the pill's white.
        Element::row(vec![Element::spacer().flex(1.0), go_on("onboarding-next", &label, true, GO_ON, palette)]).padding(16.0),
    ]).gap(8.0)
}

fn artists(state: &State, onboarding: &Onboarding, width: f32, palette: ComponentPalette) -> Element {
    let search = SearchField::new("onboarding-search", &onboarding.query, t!("common-onboarding-search"), SCHEME.on_raised())
        .size(ControlSize::Spacious).width(width).render().radius(ControlSize::Spacious.height() / 2.0);
    let columns = (((width + FACE_GAP) / (FACE_FEWEST + FACE_GAP)) as usize).max(3);
    let side = (width - FACE_GAP * (columns - 1) as f32) / columns as f32;
    let shown = onboarding.shown();
    let tiles: Vec<Element> = if shown.is_empty() && onboarding.loading {
        let pulse = ComponentPalette { raised: palette.selected, ..palette };
        (0..columns * 2).map(|_| limn::Skeleton::block("onboarding-coming", pulse, side, side).radius(side / 2.0).render()).collect()
    } else {
        shown.iter().map(|artist| face(state, onboarding, artist, side, palette)).collect()
    };
    let mut rows = Vec::new();
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        let mut row: Vec<Element> = tiles.by_ref().take(columns).collect();
        while row.len() < columns { row.push(Element::spacer().size(Some(side), None)); }
        rows.push(Element::row(row).gap(FACE_GAP));
    }
    let grid = ScrollArea::new("onboarding-artists", Element::column(rows).gap(24.0).padding(4.0)).appearance(spotify_bar()).visibility(SPOTIFY_BAR_SHOWS).render().flex(1.0);

    // What is picked so far, and the way on once there are enough.
    let count = onboarding.picked.len();
    let faces: Vec<Element> = onboarding.picked.iter().rev().take(5).map(|artist| cover(state, &artist.artwork_url, 32.0, palette).radius(16.0)).collect();
    let said = if count < PICKS_FEWEST { t!("common-onboarding-left", count = PICKS_FEWEST - count) } else { t!("common-onboarding-chosen", count = count) };
    let footer = Element::row(vec![
        Element::row(faces).gap(4.0).centered_y(),
        Text::new(said, palette).muted().render().centered_y(),
        Element::spacer().flex(1.0),
        go_on("onboarding-next", &t!("common-onboarding-ready"), count >= PICKS_FEWEST, GO_ON, palette).centered_y(),
    ]).gap(12.0).size(None, Some(80.0));
    Element::column(vec![
        Element::spacer().size(None, Some(8.0)),
        heading(&t!("common-onboarding-artists"), Some(&t!("common-onboarding-artists-detail")), palette),
        Element::spacer().size(None, Some(8.0)),
        search,
        Element::spacer().size(None, Some(8.0)),
        grid,
        footer,
    ]).gap(8.0)
}

/// An artist of the picker: their face, round, and their name under it. Picked, a white ring
/// goes round the face and a disc with a check comes onto its corner.
fn face(state: &State, onboarding: &Onboarding, artist: &ArtistRef, side: f32, palette: ComponentPalette) -> Element {
    let picked = onboarding.picks(artist);
    let inner = side - 8.0;
    let picture = cover(state, &artist.artwork_url, inner, palette).radius(inner / 2.0);
    let ring = Element::panel(picture).padding(2.0).radius(side / 2.0).size(Some(side - 4.0), Some(side - 4.0));
    let ring = if picked { ring.border(palette.text, 2.0) } else { ring };
    let mut layers = vec![Element::panel(ring.centered()).size(Some(side), Some(side))];
    if picked {
        let check = Element::icon(Icon::Custom(glyph::ADDED)).large_icon(PICKED).foreground(palette.text).size(Some(PICKED), Some(PICKED))
            .animate_enter(format!("picked:{}", artist.id));
        layers.push(Element::column(vec![Element::spacer().flex(1.0), Element::row(vec![Element::spacer().flex(1.0), check])]).padding(2.0));
    }
    // In the middle of the face's width, cut short where it would run past it.
    let name = Text::new(&artist.name, palette).font_size(14.0).truncate().render().font_family(FontFamily::SansBold).shrink_to_fit();
    let name = Element::row(vec![Element::spacer().flex(1.0), name, Element::spacer().flex(1.0)]).size(Some(side), None);
    let tile = Element::column(vec![Element::stack(layers).size(Some(side), Some(side)), name]).gap(8.0).size(Some(side), None);
    let said = format!("{}, {}", artist.name, if picked { t!("common-following") } else { t!("common-follow") });
    Element::clickable(format!("onboarding-artist:{}", artist.id), tile).accessible(limn::Access::new(limn::Role::Button).name(said))
}

fn done(state: &State, onboarding: &Onboarding, palette: ComponentPalette) -> Element {
    let faces: Vec<Element> = onboarding.picked.iter().take(7).map(|artist| cover(state, &artist.artwork_url, 64.0, palette).radius(32.0)).collect();
    Element::column(vec![
        Element::spacer().flex(1.0),
        centered_heading(&t!("common-onboarding-done", name = onboarding.name.trim()), Some(&t!("common-onboarding-done-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
        Element::row(faces).gap(8.0).centered_x(),
        Element::spacer().size(None, Some(16.0)),
        go_on("onboarding-done", &t!("common-onboarding-home"), true, GO_ON, palette).centered_x(),
        Element::spacer().flex(2.0),
    ]).gap(8.0)
}
