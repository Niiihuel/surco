//! Replay drawn as Spotify's Wrapped is: cards in a handful of flat colours, names and figures
//! in a display face, numbers ranked large, bands of rings cut by a card's edge. Any stretch
//! can be looked at this way, not only the year: the month, the year or every play kept.
//! Side by side on a desktop; one under another, as stories, on a phone.

use super::*;
use crate::replay::{Period, Ranked, Summary};
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
/// The width from which two cards share a row.
const PAIRED: f32 = 760.0;
const CARD_RADIUS: f32 = 12.0;
/// Around what a card holds.
const CARD_PAD: f32 = 24.0;
/// How tall the hours stand at their most.
const BARS: f32 = 140.0;

/// What was listened to, summed up, over the stretch chosen: how long, the artist heard most,
/// the songs and artists ranked, all of it on one card to keep, when in the day, and the run
/// of days, the busiest one and the song played again and again.
pub(super) fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let summary = state.replay();
    let inner = (width - 24.0).clamp(240.0, WIDEST);
    let paired = !state.mobile && inner >= PAIRED;
    let mut sections = Vec::new();
    // A phone's header already names it.
    if !state.mobile {
        sections.push(Element::column(vec![
            display(t!("common-replay"), if inner >= PAIRED { 64.0 } else { 44.0 }, palette.text),
            Text::new(t!("desktop-profile-only-you"), palette).muted().render(),
        ]).gap(8.0));
    }
    sections.push(periods(state, palette));
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
    sections.push(minutes(&summary, inner));
    let top = summary.top_artists.first().map(|artist| top_artist(state, artist, half, palette));
    sections.push(pair(songs(state, &summary, half, palette), Some(artists(state, &summary, half, palette))));
    if let Some(top) = top {
        sections.push(pair(top, Some(tops(state, &summary, half, palette))));
    }
    sections.push(hours(&summary, inner));
    let facts = facts(state, &summary, palette);
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
/// under what it holds, where it was placed.
fn card(fill: Color, behind: Vec<Element>, content: Element) -> Element {
    let mut layers = behind;
    layers.push(content.padding(CARD_PAD));
    Element::panel(Element::stack(layers)).background(fill).radius(CARD_RADIUS).clip_children()
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
fn minutes(summary: &Summary, width: f32) -> Element {
    let size = (width * 0.2).clamp(72.0, 168.0);
    let days = summary.minutes / (24 * 60);
    let came = if days >= 1 { t!("common-replay-minutes-days", count = days) } else { t!("common-replay-minutes-hours", count = summary.minutes / 60) };
    let ring = (width * 0.42).clamp(180.0, 360.0);
    let mut words = vec![
        Element::spacer().size(None, Some(ring * 0.2)),
        display(t!("common-replay-minutes-title"), (size * 0.24).max(20.0), CREAM).center_content(),
        display(grouped(summary.minutes), size, LAVENDER).center_content(),
    ];
    if summary.minutes >= 60 { words.push(plain(came, 16.0, CREAM).center_content()); }
    words.push(Element::spacer().size(None, Some(ring * 0.2)));
    let behind = vec![
        corner(rings(ring, 4, ring / 18.0, CREAM, Some((1, LAVENDER))), true, false, Point::new(ring * 0.5, -ring * 0.6)),
        corner(rings(ring * 0.8, 3, ring / 18.0, CREAM, None), false, true, Point::new(-ring * 0.5, ring * 0.6)),
    ];
    card(COAL, behind, Element::column(words).gap(8.0).padding(16.0))
}

/// "My Top Songs": ranked large, each cover framed in cream, the strip of them unbroken.
fn songs(state: &State, summary: &Summary, width: f32, palette: ComponentPalette) -> Element {
    let side = (width * 0.2).clamp(64.0, 104.0);
    let ring = (width * 0.5).clamp(160.0, 300.0);
    let mut rows = vec![Element::spacer().size(None, Some(ring * 0.3)), block(t!("common-replay-top-songs"), (width * 0.05).clamp(18.0, 26.0), COAL, CREAM).centered_x(), Element::spacer().size(None, Some(12.0))];
    rows.extend(summary.top_songs.iter().take(5).enumerate().map(|(index, song)| {
        let words = Element::column(vec![
            display_cut(&song.track.title, (side * 0.22).max(16.0), CREAM),
            plain(&song.track.artist, (side * 0.16).max(13.0), CREAM).font_family(FontFamily::SansMedium),
        ]).gap(2.0).flex(1.0).centered_y();
        ranked(state, format!("replay-song:{index}"), index, &song.track.title, framed(state, &song.track.artwork_url, side, CREAM, false, palette), words, side, CREAM)
    }));
    let behind = vec![corner(rings(ring, 4, ring / 16.0, CREAM, Some((2, LAVENDER))), true, true, Point::new(-ring * 0.45, -ring * 0.62))];
    card(COAL, behind, Element::column(rows))
}

/// "My Top Artists": the names on blocks, the faces framed, on lavender.
fn artists(state: &State, summary: &Summary, width: f32, palette: ComponentPalette) -> Element {
    let side = (width * 0.2).clamp(64.0, 104.0);
    let ring = (width * 0.5).clamp(160.0, 300.0);
    let mut rows = vec![Element::spacer().size(None, Some(ring * 0.3)), block(t!("common-replay-top-artists"), (width * 0.05).clamp(18.0, 26.0), CREAM, COAL).centered_x(), Element::spacer().size(None, Some(12.0))];
    rows.extend(summary.top_artists.iter().take(5).enumerate().map(|(index, artist)| {
        let name = Element::row(vec![block(&artist.track.artist, (side * 0.24).max(16.0), CREAM, COAL)]).flex(1.0).centered_y();
        ranked(state, format!("replay-artist:{index}"), index, &artist.track.artist, framed(state, &artist.track.artwork_url, side, COAL, false, palette), name, side, COAL)
    }));
    let behind = vec![corner(rings(ring, 4, ring / 16.0, COAL, Some((1, CREAM))), true, false, Point::new(ring * 0.45, -ring * 0.62))];
    card(LAVENDER, behind, Element::column(rows))
}

/// A row of a ranking: its place large, the picture, what it is; it plays when chosen.
#[allow(clippy::too_many_arguments)]
fn ranked(state: &State, id: String, index: usize, name: &str, picture: Element, words: Element, side: f32, ink: Color) -> Element {
    let place = display((index + 1).to_string(), (side * 0.36).max(22.0), ink).center_content().size(Some(side * 0.45), None).centered_y();
    let row = Element::row(vec![place, picture, words]).gap(if state.mobile { 12.0 } else { 16.0 });
    Element::clickable(id, row).accessible(limn::Access::new(limn::Role::Button).name(name.to_owned()))
}

/// "My Top Artist": the face round under rings stacked like records, the name in a frame,
/// and the minutes spent with them.
fn top_artist(state: &State, artist: &Ranked, width: f32, palette: ComponentPalette) -> Element {
    let side = (width * 0.58).clamp(180.0, 340.0);
    // The rings rise behind the face, each a step higher than the one before.
    let mut stack: Vec<Element> = (1..=4).rev().map(|step| {
        Element::panel(Element::spacer()).border(CREAM, 2.0).radius(side / 2.0).size(Some(side), Some(side)).translate(Point::new(0.0, -(step as f32) * side * 0.09))
    }).collect();
    stack.push(framed(state, &artist.track.artwork_url, side, CREAM, true, palette));
    let face = Element::column(vec![Element::spacer().size(None, Some(side * 0.36)), Element::stack(stack).size(Some(side), Some(side)).centered_x()]);
    let name = Element::panel(Element::column(vec![
        display(t!("common-replay-top-artist"), 18.0, CREAM).center_content(),
        display_cut(&artist.track.artist, (width * 0.075).clamp(24.0, 40.0), CREAM).center_content(),
    ]).gap(8.0).padding(20.0)).background(COAL).border(CREAM, 4.0);
    let listened = Element::column(vec![
        plain(t!("common-replay-minutes", count = artist.minutes), 16.0, CREAM).center_content(),
        display(grouped(artist.minutes), (width * 0.14).clamp(44.0, 72.0), CREAM).center_content(),
        plain(t!("common-replay-plays-of", count = artist.plays), 16.0, CREAM).center_content(),
    ]).gap(4.0);
    let open = Element::clickable("replay-artist:0", Element::column(vec![face, name]).gap(0.0))
        .accessible(limn::Access::new(limn::Role::Button).name(artist.track.artist.clone()));
    card(COAL, Vec::new(), Element::column(vec![open, listened]).gap(24.0))
}

/// "The Tops": the card to keep, on cream: the artist heard most framed, the five artists and
/// songs, the minutes and the longest run of days.
fn tops(state: &State, summary: &Summary, width: f32, palette: ComponentPalette) -> Element {
    let side = (width * 0.62).clamp(160.0, 340.0);
    let artwork = summary.top_artists.first().map(|artist| artist.track.artwork_url.clone()).unwrap_or_default();
    let list = |label: std::borrow::Cow<'static, str>, names: Vec<&str>| {
        let mut lines = vec![plain(label, 14.0, COAL)];
        lines.extend(names.into_iter().enumerate().map(|(index, name)| {
            Element::truncated_text(format!("{}  {name}", index + 1)).font_family(FontFamily::SansBold).font_size(16.0).foreground(COAL)
        }));
        Element::column(lines).gap(4.0).flex(1.0)
    };
    let artists = list(t!("common-replay-top-artists"), summary.top_artists.iter().take(5).map(|artist| artist.track.artist.as_str()).collect());
    let songs = list(t!("common-replay-top-songs"), summary.top_songs.iter().take(5).map(|song| song.track.title.as_str()).collect());
    let figure = |label: std::borrow::Cow<'static, str>, value: String| Element::column(vec![plain(label, 14.0, COAL), display(value, (width * 0.09).clamp(28.0, 44.0), COAL)]).gap(4.0).flex(1.0);
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
    ]).gap(20.0))
}

/// When in the day, on cream: a bar an hour, the busiest in red.
fn hours(summary: &Summary, width: f32) -> Element {
    let most = summary.hours.iter().copied().max().unwrap_or(0).max(1);
    let bars = summary.hours.iter().map(|minutes| {
        let tall = (BARS * *minutes as f32 / most as f32).max(4.0);
        let bar = Element::spacer().size(None, Some(tall)).background(if *minutes == most { RED } else { COAL });
        Element::column(vec![Element::spacer().flex(1.0), bar]).size(None, Some(BARS)).flex(1.0)
    }).collect();
    let marks = Element::row([0, 6, 12, 18].into_iter().map(|hour| plain(format!("{hour}:00"), 13.0, COAL).flex(1.0)).collect());
    let mut parts = vec![Element::row(vec![block(t!("common-replay-when"), (width * 0.04).clamp(18.0, 26.0), YELLOW, COAL)])];
    if let Some(part) = crate::replay::part_of_day(&summary.hours) {
        parts.push(plain(crate::i18n::text(crate::i18n::current(), part, None), 16.0, COAL));
    }
    parts.extend([Element::spacer().size(None, Some(8.0)), Element::row(bars).gap(if width >= PAIRED { 6.0 } else { 3.0 }), marks]);
    card(CREAM, Vec::new(), Element::column(parts).gap(8.0))
}

/// A few things worth knowing, a card of colour each: the run of days, the busiest day, the
/// song played again and again in one.
fn facts(state: &State, summary: &Summary, palette: ComponentPalette) -> Vec<Element> {
    let fact = |fill: Color, leading: Option<Element>, value: String, label: String| {
        let words = Element::column(vec![display_cut(value, 32.0, COAL), plain(label, 15.0, COAL)]).gap(4.0).flex(1.0).centered_y();
        let mut row = Vec::from_iter(leading.map(Element::centered_y));
        row.push(words);
        card(fill, Vec::new(), Element::row(row).gap(14.0))
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
    facts
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
