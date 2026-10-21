//! Replay drawn as Spotify's Wrapped is: cards in a handful of flat colours, names and figures
//! in a display face, numbers ranked large, bands of rings cut by a card's edge. Any stretch
//! can be looked at this way, not only the year: the month, the year or every play kept.
//! Side by side on a desktop; one under another, as stories, on a phone.

use super::*;
use crate::replay::{Period, Ranked, Summary};
use limn::{ChartSeries, LineChart};
use limn::Point;

/// Wrapped's palette.
const COAL: Color = Color::rgb(0x27, 0x27, 0x27);
const CREAM: Color = Color::rgb(0xEF, 0xEF, 0xEA);
const YELLOW: Color = Color::rgb(0xDA, 0xB5, 0x03);
const LAVENDER: Color = Color::rgb(0x96, 0x91, 0xFF);
const RED: Color = Color::rgb(0xFF, 0x48, 0x34);
const GREEN: Color = Color::rgb(0x95, 0xC9, 0x0D);

/// The widest the cards stand together, as wide as two stories side by side read well.
const WIDEST: f32 = 1200.0;
/// The width from which the button to watch the story stands beside the page's name.
const NAME_BESIDE: f32 = 620.0;
/// The width from which two cards share a row.
const PAIRED: f32 = 760.0;
const CARD_RADIUS: f32 = 12.0;
/// Around what a card holds.
const CARD_PAD: f32 = 24.0;
/// Room over a story's step for its bars and the way out.
const STORY_TOP: f32 = 64.0;

/// What was listened to, summed up, over the stretch chosen: how long, the artist heard most,
/// the songs and artists ranked, all of it on one card to keep, when in the day, and the run
/// of days, the busiest one and the song played again and again.
pub(super) fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let summary = state.replay();
    let inner = (width - 24.0).clamp(240.0, WIDEST);
    let paired = !state.mobile && inner >= PAIRED;
    let mut sections = Vec::new();
    // A phone's header already names it.
    let watch = || card_button("replay-story", t!("common-replay-story"), true).icon(Icon::Custom(glyph::REPLAY)).render();
    if !state.mobile {
        let named = Element::column(vec![
            display(t!("common-replay"), if inner >= PAIRED { 64.0 } else { 40.0 }, palette.text),
            Text::new(t!("desktop-profile-only-you"), palette).muted().render(),
        ]).gap(8.0).flex(1.0);
        // Beside the name where there is room for both; under it where there is not.
        sections.push(match summary.plays > 0 {
            true if inner >= NAME_BESIDE => Element::row(vec![named, watch().centered_y()]).gap(16.0),
            true => Element::column(vec![named, Element::row(vec![watch()])]).gap(12.0),
            false => named,
        });
    }
    sections.push(periods(state, palette));
    if state.mobile && summary.plays > 0 { sections.push(watch()); }
    if summary.plays == 0 {
        sections.push(centered(Empty::new(t!("common-replay-empty"), t!("common-replay-empty-detail"), palette).icon(Icon::Custom(glyph::REPLAY)).render()));
        return page_scroll("replay", Element::column(sections).gap(16.0).padding(12.0));
    }
    let half = if paired { (inner - 16.0) / 2.0 } else { inner };
    let pair = |left: Element, right: Option<Element>| match right {
        Some(right) if paired => Element::row(vec![left.flex(1.0), right.flex(1.0)]).gap(16.0),
        Some(right) => Element::column(vec![left, right]).gap(16.0),
        None => left,
    };
    sections.push(minutes(&summary, inner, None));
    let top = summary.top_artists.first().map(|artist| top_artist(state, artist, half, palette, None));
    sections.push(pair(songs(state, &summary, half, palette, None), Some(artists(state, &summary, half, palette, None))));
    if let Some(top) = top {
        sections.push(pair(top, Some(tops(state, &summary, half, palette, None))));
    }
    sections.push(hours(&summary, inner, "replay-hours", palette, None));
    let facts = facts(state, &summary, palette, None);
    if !facts.is_empty() {
        let row = if paired { Element::row(facts.into_iter().map(|fact| fact.flex(1.0)).collect()).gap(16.0) } else { Element::column(facts).gap(16.0) };
        sections.push(row);
    }
    page_scroll("replay", Element::column(vec![Element::column(sections).gap(16.0).size(Some(inner), None).centered_x()]).padding(12.0))
}

/// The stretch chosen as Spotify's filters: pills, the one chosen filled light.
fn periods(state: &State, palette: ComponentPalette) -> Element {
    let chips = Period::ALL.into_iter().map(|period| {
        let label = match period { Period::Month => t!("common-replay-month"), Period::Year => t!("common-replay-year"), Period::All => t!("common-replay-all") };
        let (id, chosen) = (format!("replay-period:{}", period.slug()), period == state.replay_period);
        if state.mobile { return mobile::chip(id, &label, chosen, limn::Role::Button, palette); }
        Button::new(id, label, palette).variant(if chosen { ButtonVariant::Default } else { ButtonVariant::Secondary })
            .sizing(ControlSize::Regular.button_sizing()).round().render()
    }).collect();
    Element::row(chips).gap(8.0).wrap().line_gap(8.0)
}

/// Words in the display face.
fn display(words: impl Into<String>, size: f32, ink: Color) -> Element {
    Element::text(words.into()).font_family(FontFamily::Display).font_size(size).foreground(ink)
}

/// A name in the display face that ends where the room does.
fn display_cut(words: impl Into<String>, size: f32, ink: Color) -> Element {
    Element::truncated_text(words.into()).font_family(FontFamily::Display).font_size(size).foreground(ink)
}

/// Plain words, in the text's face.
fn plain(words: impl Into<String>, size: f32, ink: Color) -> Element {
    Element::text(words.into()).font_family(FontFamily::Sans).font_size(size).foreground(ink)
}

/// Words on a block of colour, as Wrapped sets its headlines and names.
fn block(words: impl Into<String>, size: f32, ink: Color, fill: Color) -> Element {
    let pad = (size * 0.3).round();
    // The display face stands taller than the text's lines: the block is sized to it.
    Element::panel(Element::row(vec![
        Element::spacer().size(Some(pad), None),
        display_cut(words, size, ink).line_height(size * 1.45).centered_y(),
        Element::spacer().size(Some(pad), None),
    ])).background(fill).size(None, Some((size * 1.45).round()))
}

/// A card of one colour, its corners round, what reaches past its edge cut off. `behind` lies
/// under what it holds, where it was placed. In a story, `at` its time in, the screen is the
/// card: nothing is cut at its edge, so its rings run on to the screen's.
fn card(fill: Color, behind: Vec<Element>, content: Element, at: Option<f32>) -> Element {
    let mut layers = behind;
    layers.push(content.padding(CARD_PAD));
    let card = Element::panel(Element::stack(layers)).background(fill).radius(CARD_RADIUS);
    if at.is_some() { card } else { card.clip_children() }
}

/// Bands round one centre, Wrapped's rings: `count` of them `band` wide with as much between,
/// the outermost `side` across, the one at `accent` in its own colour.
fn rings(side: f32, count: usize, band: f32, ink: Color, accent: Option<(usize, Color)>) -> Element {
    let bands = (0..count).filter_map(|index| {
        let across = side - index as f32 * band * 4.0;
        (across > band * 2.0).then(|| {
            let color = match accent { Some((at, color)) if at == index => color, _ => ink };
            Element::panel(Element::spacer()).border(color, band).radius(across / 2.0).size(Some(across), Some(across)).centered()
        })
    }).collect();
    Element::stack(bands).size(Some(side), Some(side))
}

/// Something laid in a card's corner and moved by `by`, out past its edge, taking no room.
fn corner(element: Element, top: bool, left: bool, by: Point) -> Element {
    let row = if left { Element::row(vec![element, Element::spacer().flex(1.0)]) } else { Element::row(vec![Element::spacer().flex(1.0), element]) };
    let column = if top { Element::column(vec![row, Element::spacer().flex(1.0)]) } else { Element::column(vec![Element::spacer().flex(1.0), row]) };
    column.translate(by)
}

/// A picture in a frame of `frame`'s colour, round when `round`.
fn framed(state: &State, artwork: &Option<String>, side: f32, frame: Color, round: bool, palette: ComponentPalette) -> Element {
    let edge = (side * 0.02).clamp(2.0, 4.0);
    let inside = side - 2.0 * edge;
    let picture = cover(state, artwork, inside, palette).radius(if round { inside / 2.0 } else { 0.0 });
    Element::panel(picture.centered()).background(frame).radius(if round { side / 2.0 } else { 0.0 }).size(Some(side), Some(side))
}

/// Whether the `index`th of a list has come in yet, `at` seconds into a story's step; on the
/// page, always.
fn arrived(at: Option<f32>, index: usize) -> bool {
    at.is_none_or(|at| at >= 0.35 + 0.18 * index as f32)
}

/// What comes in with a story's step fades and settles in under `id`; on the page it stands.
fn entering(element: Element, at: Option<f32>, id: String) -> Element {
    if at.is_some() { element.animate_enter(id) } else { element }
}

/// How far along `at` seconds are of `seconds`, easing out as a count does when it lands.
fn landing(at: f32, seconds: f32) -> f32 {
    1.0 - (1.0 - (at / seconds).clamp(0.0, 1.0)).powi(3)
}

/// A count with its thousands apart, as the language writes them.
fn grouped(value: u64) -> String {
    let separator = if crate::i18n::current() == Language::Spanish { '.' } else { ',' };
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) { out.push(separator); }
        out.push(digit);
    }
    out
}

/// "My Minutes Listened": the minutes, huge, in lavender on coal, and what they come to.
fn minutes(summary: &Summary, width: f32, at: Option<f32>) -> Element {
    let size = (width * 0.22).clamp(56.0, 168.0);
    let days = summary.minutes / (24 * 60);
    let came = if days >= 1 { t!("common-replay-minutes-days", count = days) } else { t!("common-replay-minutes-hours", count = summary.minutes / 60) };
    let ring = (width * 0.42).clamp(180.0, 360.0);
    let mut words = vec![
        Element::spacer().size(None, Some(ring * 0.2)),
        display(t!("common-replay-minutes-title"), (size * 0.24).max(20.0), CREAM).center_content(),
        // In a story the count runs up to it.
        display(grouped(at.map_or(summary.minutes, |at| (summary.minutes as f32 * landing(at, 1.6)).round() as u64)), size, LAVENDER).center_content(),
    ];
    if summary.minutes >= 60 && at.is_none_or(|at| at >= 1.6) { words.push(entering(plain(came, 16.0, CREAM).center_content(), at, "story:minutes-came".into())); }
    words.push(Element::spacer().size(None, Some(ring * 0.2)));
    let behind = vec![
        corner(rings(ring, 4, ring / 18.0, CREAM, Some((1, LAVENDER))), true, false, Point::new(ring * 0.5, -ring * 0.6)),
        corner(rings(ring * 0.8, 3, ring / 18.0, CREAM, None), false, true, Point::new(-ring * 0.5, ring * 0.6)),
    ];
    card(COAL, behind, Element::column(words).gap(8.0).padding(16.0), at)
}

/// "My Top Songs": ranked large, each cover framed in cream, the strip of them unbroken.
fn songs(state: &State, summary: &Summary, width: f32, palette: ComponentPalette, at: Option<f32>) -> Element {
    let side = (width * 0.2).clamp(48.0, 104.0);
    let ring = (width * 0.5).clamp(160.0, 300.0);
    let mut rows = vec![Element::spacer().size(None, Some(ring * 0.3)), block(t!("common-replay-top-songs"), (width * 0.05).clamp(18.0, 26.0), COAL, CREAM).centered_x(), Element::spacer().size(None, Some(12.0))];
    rows.extend(summary.top_songs.iter().take(5).enumerate().map(|(index, song)| {
        let words = Element::column(vec![
            display_cut(&song.track.title, (side * 0.22).max(14.0), CREAM),
            plain(&song.track.artist, (side * 0.16).max(12.0), CREAM).font_family(FontFamily::SansMedium),
        ]).gap(2.0).flex(1.0).centered_y();
        if !arrived(at, index) { return Element::spacer().size(None, Some(side)); }
        entering(ranked(state, format!("replay-song:{index}"), index, &song.track.title, framed(state, &song.track.artwork_url, side, CREAM, false, palette), words, side, CREAM), at, format!("story:song:{index}"))
    }));
    let behind = vec![corner(rings(ring, 4, ring / 16.0, CREAM, Some((2, LAVENDER))), true, true, Point::new(-ring * 0.45, -ring * 0.62))];
    card(COAL, behind, Element::column(rows), at)
}

/// "My Top Artists": the names on blocks, the faces framed, on lavender.
fn artists(state: &State, summary: &Summary, width: f32, palette: ComponentPalette, at: Option<f32>) -> Element {
    let side = (width * 0.2).clamp(48.0, 104.0);
    let ring = (width * 0.5).clamp(160.0, 300.0);
    let mut rows = vec![Element::spacer().size(None, Some(ring * 0.3)), block(t!("common-replay-top-artists"), (width * 0.05).clamp(18.0, 26.0), CREAM, COAL).centered_x(), Element::spacer().size(None, Some(12.0))];
    rows.extend(summary.top_artists.iter().take(5).enumerate().map(|(index, artist)| {
        let name = Element::row(vec![block(&artist.track.artist, (side * 0.24).max(14.0), CREAM, COAL)]).flex(1.0).centered_y();
        if !arrived(at, index) { return Element::spacer().size(None, Some(side)); }
        entering(ranked(state, format!("replay-artist:{index}"), index, &artist.track.artist, framed(state, &artist.track.artwork_url, side, COAL, false, palette), name, side, COAL), at, format!("story:artist:{index}"))
    }));
    let behind = vec![corner(rings(ring, 4, ring / 16.0, COAL, Some((1, CREAM))), true, false, Point::new(ring * 0.45, -ring * 0.62))];
    card(LAVENDER, behind, Element::column(rows), at)
}

/// A row of a ranking: its place large, the picture, what it is; it plays when chosen.
#[allow(clippy::too_many_arguments)]
fn ranked(state: &State, id: String, index: usize, name: &str, picture: Element, words: Element, side: f32, ink: Color) -> Element {
    let place = display((index + 1).to_string(), (side * 0.36).max(18.0), ink).center_content().size(Some(side * 0.45), None).centered_y();
    let row = Element::row(vec![place, picture, words]).gap(if state.mobile { 12.0 } else { 16.0 });
    Element::clickable(id, row).accessible(limn::Access::new(limn::Role::Button).name(name.to_owned()))
}

/// "My Top Artist": the face round under rings stacked like records, the name in a frame,
/// and the minutes spent with them.
fn top_artist(state: &State, artist: &Ranked, width: f32, palette: ComponentPalette, at: Option<f32>) -> Element {
    let side = (width * 0.58).clamp(96.0, 340.0);
    // The rings rise behind the face, each a step higher than the one before.
    let mut stack: Vec<Element> = (1..=4).rev().map(|step| {
        Element::panel(Element::spacer()).border(CREAM, 2.0).radius(side / 2.0).size(Some(side), Some(side)).translate(Point::new(0.0, -(step as f32) * side * 0.09))
    }).collect();
    let face = framed(state, &artist.track.artwork_url, side, CREAM, true, palette);
    // In a story it swells from smaller into its place.
    stack.push(match at {
        Some(at) => face.animate_scale_about("story:top-artist", if at < 0.05 { 0.6 } else { 1.0 }, Point::new(0.5, 0.5)),
        None => face,
    });
    let face = Element::column(vec![Element::spacer().size(None, Some(side * 0.36)), Element::stack(stack).size(Some(side), Some(side)).centered_x()]);
    let name = Element::panel(Element::column(vec![
        display(t!("common-replay-top-artist"), 18.0, CREAM).center_content(),
        display_cut(&artist.track.artist, (width * 0.075).clamp(20.0, 40.0), CREAM).center_content(),
    ]).gap(8.0).padding((width * 0.045).clamp(12.0, 20.0))).background(COAL).border(CREAM, 4.0);
    let listened = Element::column(vec![
        plain(t!("common-replay-minutes", count = artist.minutes), 16.0, CREAM).center_content(),
        display(grouped(artist.minutes), (width * 0.14).clamp(32.0, 72.0), CREAM).center_content(),
        plain(t!("common-replay-plays-of", count = artist.plays), 16.0, CREAM).center_content(),
    ]).gap(4.0);
    let open = Element::clickable("replay-artist:0", Element::column(vec![face, name]).gap(0.0))
        .accessible(limn::Access::new(limn::Role::Button).name(artist.track.artist.clone()));
    let listened = if at.is_none_or(|at| at >= 0.9) { entering(listened, at, "story:top-artist-minutes".into()) } else { Element::spacer() };
    card(COAL, Vec::new(), Element::column(vec![open, listened]).gap((width * 0.05).clamp(12.0, 24.0)), at)
}

/// "The Tops": the card to keep, on cream: the artist heard most framed, the five artists and
/// songs, the minutes and the longest run of days.
fn tops(state: &State, summary: &Summary, width: f32, palette: ComponentPalette, at: Option<f32>) -> Element {
    let side = (width * 0.52).clamp(110.0, 320.0);
    let artwork = summary.top_artists.first().map(|artist| artist.track.artwork_url.clone()).unwrap_or_default();
    let list = |label: std::borrow::Cow<'static, str>, names: Vec<&str>| {
        let mut lines = vec![plain(label, 14.0, COAL)];
        lines.extend(names.into_iter().enumerate().map(|(index, name)| {
            Element::truncated_text(format!("{}  {name}", index + 1)).font_family(FontFamily::SansBold).font_size(if width < 360.0 { 14.0 } else { 16.0 }).foreground(COAL)
        }));
        Element::column(lines).gap(if width < 360.0 { 2.0 } else { 4.0 }).flex(1.0)
    };
    let artists = list(t!("common-replay-top-artists"), summary.top_artists.iter().take(5).map(|artist| artist.track.artist.as_str()).collect());
    let songs = list(t!("common-replay-top-songs"), summary.top_songs.iter().take(5).map(|song| song.track.title.as_str()).collect());
    let figure = |label: std::borrow::Cow<'static, str>, value: String| Element::column(vec![plain(label, 14.0, COAL), display(value, (width * 0.09).clamp(24.0, 44.0), COAL)]).gap(4.0).flex(1.0);
    let streak = if summary.streak > 1 { t!("common-replay-days", count = summary.streak).into_owned() } else { grouped(summary.plays as u64) };
    let streak_label = if summary.streak > 1 { t!("common-replay-streak-title") } else { t!("common-replay-plays", count = summary.plays) };
    let ring = side * 0.9;
    let behind = vec![
        corner(rings(ring, 4, ring / 16.0, YELLOW, None), true, true, Point::new(-ring * 0.5, -ring * 0.2)),
        corner(rings(ring, 4, ring / 16.0, COAL, None), true, false, Point::new(ring * 0.55, -ring * 0.1)),
    ];
    card(CREAM, behind, Element::column(vec![
        framed(state, &artwork, side, COAL, false, palette).centered_x(),
        Element::spacer().size(None, Some(8.0)),
        Element::row(vec![artists, songs]).gap(16.0),
        Element::row(vec![figure(t!("common-replay-minutes", count = summary.minutes), grouped(summary.minutes)), figure(streak_label, streak)]).gap(16.0),
    ]).gap((width * 0.04).clamp(10.0, 20.0)), at)
}

/// When in the day, on cream, as ECharts' stacked areas with a gradient: the artists heard
/// most each a band of their colour on the ones before, everyone else's on top, the lines
/// drawn in from the left; under the pointer the hour says each one's minutes.
fn hours(summary: &Summary, width: f32, id: &str, palette: ComponentPalette, at: Option<f32>) -> Element {
    const BANDS: [Color; 4] = [LAVENDER, RED, YELLOW, GREEN];
    let mut series: Vec<ChartSeries> = summary.hours_of.iter().zip(&summary.top_artists).zip(BANDS)
        .map(|((hours, artist), color)| ChartSeries::new(artist.track.artist.clone(), hours.map(|minutes| minutes as f64)).color(color))
        .collect();
    let rest: Vec<f64> = (0..24).map(|hour| summary.hours[hour].saturating_sub(summary.hours_of.iter().map(|hours| hours[hour]).sum()) as f64).collect();
    if rest.iter().any(|minutes| *minutes > 0.0) {
        series.push(ChartSeries::new(t!("common-replay-everyone-else").into_owned(), rest).color(BANDS[series.len().min(3)]));
    }
    let legend = Element::row(series.iter().map(|band| Element::row(vec![
        Element::spacer().size(Some(10.0), Some(10.0)).background(band.color.unwrap_or(COAL)).radius(5.0).centered_y(),
        Element::truncated_text(band.name.clone()).font_family(FontFamily::Sans).font_size(13.0).foreground(COAL).centered_y(),
    ]).gap(6.0)).collect()).gap(14.0).wrap().line_gap(6.0);
    let ink = ComponentPalette { surface: CREAM, text: COAL, muted: Color { a: 170, ..COAL }, border: Color { a: 60, ..COAL }, ..palette };
    let chart = LineChart::new(id, (0..24).map(|hour| format!("{hour}:00")), series, ink)
        .area(true).stacked(true).smooth(true).value_axis(false).height((width * 0.42).clamp(140.0, 260.0)).render();
    let mut parts = vec![Element::row(vec![block(t!("common-replay-when"), (width * 0.04).clamp(18.0, 26.0), YELLOW, COAL)])];
    if let Some(part) = crate::replay::part_of_day(&summary.hours) {
        parts.push(plain(crate::i18n::text(crate::i18n::current(), part, None), 16.0, COAL));
    }
    parts.extend([legend, chart]);
    card(CREAM, Vec::new(), Element::column(parts).gap(10.0), at)
}

/// A few things worth knowing, a card of colour each: the run of days, the busiest day, the
/// song played again and again in one.
fn facts(state: &State, summary: &Summary, palette: ComponentPalette, at: Option<f32>) -> Vec<Element> {
    let fact = |fill: Color, leading: Option<Element>, value: String, label: String| {
        let words = Element::column(vec![display_cut(value, 32.0, COAL), plain(label, 15.0, COAL)]).gap(4.0).flex(1.0).centered_y();
        let mut row = Vec::from_iter(leading.map(Element::centered_y));
        row.push(words);
        card(fill, Vec::new(), Element::row(row).gap(14.0), None)
    };
    let mut facts = Vec::new();
    if summary.streak > 1 {
        facts.push(fact(YELLOW, None, t!("common-replay-days", count = summary.streak).into_owned(), t!("common-replay-streak").into_owned()));
    }
    if let Some((day, minutes)) = summary.busiest_day {
        facts.push(fact(GREEN, None, day.format("%d/%m/%Y").to_string(), t!("common-replay-busiest", minutes = minutes).into_owned()));
    }
    if let Some((track, times)) = &summary.on_repeat {
        facts.push(fact(RED, Some(framed(state, &track.artwork_url, 64.0, COAL, false, palette)), track.title.clone(), t!("common-replay-on-repeat", times = *times).into_owned()));
    }
    facts.into_iter().enumerate().filter(|(index, _)| arrived(at, *index)).map(|(index, fact)| entering(fact, at, format!("story:fact:{index}"))).collect()
}

/// Replay told as a story over the window, as Wrapped is: a step at a time, each with a bar
/// along the top that fills while it is on screen. A tap or a click on its right goes on, on its
/// left back, as the arrows beside it do. A card a phone's shape on a desktop; the whole
/// screen on a phone.
pub(super) fn story(state: &State, window_tree: Element, window: Size) -> Element {
    use crate::replay::Step;
    let Some((index, since)) = state.replay_story else { return window_tree };
    let summary = state.replay();
    let steps = summary.story();
    let Some(&step) = steps.get(index) else { return window_tree };
    let at = since.elapsed().as_secs_f32();
    let palette = SCHEME.palette();
    let (width, height) = if state.mobile {
        (window.width, window.height)
    } else {
        let tall = (window.height - 64.0).clamp(480.0, 860.0);
        ((tall * 9.0 / 16.0).min(window.width - 160.0), tall)
    };
    let shown = Some(at);
    // On a phone the bars sit under the status bar and the island, not behind them.
    let notch = if state.mobile { state.mobile_ui.content_insets.top } else { 0.0 };
    // What it holds is sized to fit the screen under the bars, tall as well as wide; what still
    // runs past its foot scrolls rather than being cut.
    let room = height - STORY_TOP - notch - 16.0;
    let fit = width.min(room * 0.6);
    let (fill, content) = match step {
        Step::Intro => (COAL, intro(state, fit)),
        Step::Minutes => (COAL, minutes(&summary, fit, shown)),
        Step::TopArtist => (COAL, top_artist(state, &summary.top_artists[0], fit, palette, shown)),
        Step::Songs => (COAL, songs(state, &summary, fit, palette, shown)),
        Step::Artists => (LAVENDER, artists(state, &summary, fit, palette, shown)),
        Step::Hours => (CREAM, hours(&summary, fit, &format!("story-hours:{since:?}"), palette, shown)),
        Step::Facts => (COAL, Element::column(facts(state, &summary, palette, shown)).gap(12.0).padding(16.0)),
        Step::Tops => (CREAM, tops(state, &summary, fit, palette, shown)),
    };
    let ink = if fill == CREAM { COAL } else { CREAM };
    // The room under the bars is kept inside what scrolls, so the rings run on to the screen's top edge.
    let content = Element::column(vec![Element::spacer().size(None, Some(STORY_TOP + notch)), Element::spacer().flex(1.0), content, Element::spacer().flex(1.0)])
        .min_size(0.0, height - 16.0).animate_enter(format!("story-step:{index}:{since:?}"));
    let content = panel_scroll(&format!("story-scroll:{index}"), content).render().size(Some(width), Some(height));
    let screen = Element::panel(content).background(fill).size(Some(width), Some(height));
    // A bar a step: those gone full, this one filling, those to come empty, sharing the width.
    let bars = (0..steps.len()).map(|position| {
        let share = match position.cmp(&index) {
            std::cmp::Ordering::Less => 1.0,
            std::cmp::Ordering::Equal => (at / crate::state::STORY_STEP.as_secs_f32()).min(1.0),
            std::cmp::Ordering::Greater => 0.0,
        };
        let filled = Element::spacer().size(None, Some(3.0)).background(ink).radius(1.5);
        let inside = match share {
            share if share <= 0.0 => Element::spacer(),
            share if share >= 1.0 => filled.flex(1.0),
            share => Element::row(vec![filled.flex(share), Element::spacer().flex(1.0 - share)]),
        };
        Element::panel(Element::row(vec![inside.flex(1.0)])).background(Color { a: 70, ..ink }).radius(1.5).size(None, Some(3.0)).flex(1.0)
    }).collect::<Vec<_>>();
    let close = IconButton::new("story-close", Icon::Custom(glyph::CLOSE), ComponentPalette { text: ink, hover: Color { a: 30, ..ink }, selected: Color { a: 50, ..ink }, ..palette })
        .round().variant(ButtonVariant::Ghost).size(ControlSize::Regular).accessibility_label(t!("common-close")).render();
    let chrome = Element::column(vec![Element::spacer().size(None, Some(notch)), Element::row(bars).gap(4.0), Element::row(vec![Element::spacer().flex(1.0), close])]).gap(4.0).padding(16.0);
    let tap = |id: &'static str, label: std::borrow::Cow<'static, str>, share: f32| Element::clickable(id, Element::spacer().size(None, Some(height)))
        .accessible(limn::Access::new(limn::Role::Button).name(label)).flex(share);
    let taps = Element::row(vec![tap("story-back", t!("common-back"), 1.0), tap("story-next", t!("desktop-next"), 2.0)]).size(Some(width), Some(height));
    let card = Element::panel(Element::stack(vec![screen, taps, Element::column(vec![chrome, Element::spacer().flex(1.0)]).size(Some(width), Some(height))]))
        .radius(if state.mobile { 0.0 } else { 16.0 }).clip_children().size(Some(width), Some(height));
    let dialog = if state.mobile {
        card
    } else {
        // Beside it on a desktop, the way on and back.
        let arrow = |id: &str, glyph: &'static str, label: std::borrow::Cow<'static, str>, there: bool| {
            // Its own ID, apart from the side of the card that does the same.
            let button = IconButton::new(format!("{id}:arrow"), Icon::Custom(glyph), palette).dispatch_as(id).round().variant(ButtonVariant::Secondary).size(ControlSize::Regular).accessibility_label(label).render();
            if there { button } else { Element::spacer().size(Some(40.0), Some(40.0)) }
        };
        Element::row(vec![
            arrow("story-back", glyph::CHEVRON_LEFT, t!("common-back"), index > 0).centered_y(),
            card,
            arrow("story-next", glyph::CHEVRON_RIGHT, t!("desktop-next"), index + 1 < steps.len()).centered_y(),
        ]).gap(24.0)
    };
    let dialog = dialog.accessible(limn::Access::new(limn::Role::Dialog).id("replay-story").name(t!("common-replay")));
    Element::popover(window_tree, dialog, "story-close", PopoverPlacement::Center).popover_scrim(Color::rgba(0, 0, 0, 235))
}

/// The story's first words, as Wrapped's: that it was all heard, and over which stretch.
fn intro(state: &State, width: f32) -> Element {
    let size = (width * 0.1).clamp(28.0, 44.0);
    let stretch = match state.replay_period { Period::Month => t!("common-replay-month"), Period::Year => t!("common-replay-year"), Period::All => t!("common-replay-all") };
    let ring = width * 0.9;
    let behind = vec![corner(rings(ring, 6, ring / 24.0, CREAM, Some((2, LAVENDER))), false, true, Point::new(-ring * 0.3, ring * 0.45))];
    card(COAL, behind, Element::column(vec![
        display(t!("common-replay-intro"), size, CREAM).line_height(size * 1.2).center_content().animate_enter("story:intro"),
        plain(stretch, 16.0, CREAM).center_content(),
        Element::spacer().size(None, Some(ring * 0.5)),
    ]).gap(12.0), Some(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_is_grouped_by_thousands_as_the_language_writes_it() {
        let separator = if crate::i18n::current() == Language::Spanish { "." } else { "," };
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(53_623), format!("53{separator}623"));
        assert_eq!(grouped(1_234_567), format!("1{separator}234{separator}567"));
    }
}
