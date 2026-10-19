//! The first run's steps over the whole window, as Spotify's sign-up: one question at a time
//! in the middle, a white pill to go on, and a picker of round faces for the artists.

use super::*;
use crate::state::onboarding::{Onboarding, Step, PICKS_FEWEST};
use surco_core::ArtistRef;

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
    let margin = if phone { 16.0 } else { 32.0 };
    let most = if onboarding.step == Step::Artists { PICKER_COLUMN } else { COLUMN };
    let width = (window.width - 2.0 * margin).min(most).max(200.0);

    let body = match onboarding.step {
        Step::Welcome => welcome(palette),
        Step::Name => name(onboarding, width, palette),
        Step::Genres => genres(onboarding, palette),
        Step::Artists => artists(state, onboarding, width, palette),
        Step::Done => done(state, onboarding, palette),
    };
    // Each step comes in as the last leaves, as the steps of Spotify's.
    let body = body.animate_enter(format!("onboarding:{:?}", onboarding.step));
    let column = Element::column(vec![header(onboarding, palette), body.flex(1.0)]).size(Some(width), None).flex(1.0);
    let top = if state.mobile { state.mobile_ui.content_insets.top } else { 0.0 };
    Element::panel(Element::row(vec![Element::spacer().flex(1.0), column, Element::spacer().flex(1.0)]).padding(margin))
        .background(SCHEME.canvas)
        .size(Some(window.width), Some(window.height))
        .padding(top)
}

/// The way back, where the steps are, and the way past one that can be left out.
fn header(onboarding: &Onboarding, palette: ComponentPalette) -> Element {
    let step = onboarding.step;
    let back = matches!(step, Step::Name | Step::Artists) || (step == Step::Genres && !onboarding.taste_only);
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
    row.push(if matches!(step, Step::Genres | Step::Artists) {
        Button::new("onboarding-skip", t!("common-onboarding-skip"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).round().render().centered_y()
    } else {
        Element::spacer().size(Some(32.0), None)
    });
    Element::row(row).size(None, Some(64.0))
}

/// A step's question and what it is for.
fn heading(title: &str, detail: Option<&str>, palette: ComponentPalette) -> Element {
    let mut copy = vec![Text::new(title, palette).font_size(32.0).render().font_family(FontFamily::SansBold)];
    copy.extend(detail.map(|detail| Text::new(detail, palette).font_size(16.0).muted().render()));
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
        heading(&t!("common-onboarding-welcome"), Some(&t!("common-onboarding-welcome-detail")), palette),
        Element::spacer().size(None, Some(24.0)),
        go_on("onboarding-fresh", &t!("common-onboarding-fresh"), true, WELCOME_PILL, palette),
        bring,
        Element::spacer().flex(2.0),
    ]).gap(12.0)
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
        let chip = Button::new(format!("onboarding-genre:{query}"), label, palette)
            .variant(if chosen { ButtonVariant::Default } else { ButtonVariant::Secondary })
            .size(ControlSize::Regular).round();
        if chosen { chip.icon(Icon::Check) } else { chip }.render()
    }).collect();
    let chips = ScrollArea::new("onboarding-genres", Element::row(chips).gap(8.0).line_gap(8.0).wrap()).appearance(spotify_bar()).visibility(SPOTIFY_BAR_SHOWS).render().flex(1.0);
    let label = if onboarding.genres.is_empty() { t!("common-onboarding-skip") } else { t!("common-onboarding-next") };
    let id = if onboarding.genres.is_empty() { "onboarding-skip" } else { "onboarding-next" };
    Element::column(vec![
        Element::spacer().size(None, Some(24.0)),
        heading(&t!("common-onboarding-genres"), Some(&t!("common-onboarding-genres-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
        chips,
        Element::row(vec![Element::spacer().flex(1.0), go_on(id, &label, true, GO_ON, palette)]).padding(16.0),
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
        heading(&t!("common-onboarding-done", name = onboarding.name.trim()), Some(&t!("common-onboarding-done-detail")), palette),
        Element::spacer().size(None, Some(16.0)),
        Element::row(faces).gap(8.0),
        Element::spacer().size(None, Some(16.0)),
        go_on("onboarding-done", &t!("common-onboarding-home"), true, GO_ON, palette),
        Element::spacer().flex(2.0),
    ]).gap(8.0)
}
