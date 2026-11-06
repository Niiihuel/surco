//! A phone's library as BitChord's: what was listened to this year on cards across the top,
//! the downloads, then the lists and the artists kept, each shelf with a way to all of them.

use super::*;
use crate::replay::{summarize, Period, Summary};

/// BitChord's listening cards: 78% of the width, three tall to five wide, corners 22, their
/// words 20 in from the edge, on a plum shade that darkens downward.
const CARD_SHARE: f32 = 0.78;
const CARD_MOST: f32 = 340.0;
const CARD_RATIO: f32 = 0.63;
const CARD_RADIUS: f32 = 22.0;
const CARD_PAD: f32 = 20.0;
/// BitChord's card: nearly black with a breath of plum, one colour, on a soft shadow.
const CARD_GROUND: Color = Color::rgb(40, 34, 38);
const CARD_SHADOW: Color = Color::rgba(0, 0, 0, 110);
const CARD_EYEBROW: Color = Color::rgba(255, 255, 255, 200);
const CARD_QUIET: Color = Color::rgba(255, 255, 255, 170);
/// A row of the library: 55 tall, its glyph 24 in a 40 column, a hairline under its words.
const ROW_HEIGHT: f32 = 55.0;
const ROW_GLYPH: f32 = 40.0;
const ROW_RULE: Color = Color::rgba(255, 255, 255, 20);
/// The liked songs' tile, as BitChord's: pink into violet.
const LIKED_GROUND: [Color; 2] = [Color::rgb(236, 100, 196), Color::rgb(124, 82, 232)];

pub(in crate::view) fn content(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let now = chrono::Local::now().fixed_offset();
    let year = summarize(&state.collection.plays, Period::Year, now);
    let mut parts = Vec::new();
    if year.plays > 0 {
        parts.push(cards(state, &year, now, width, palette));
    }
    parts.push(rows(palette));
    parts.push(playlists(state, palette));
    if let Some(artists) = artists(state, palette) { parts.push(artists); }
    Element::column(parts).gap(24.0).padding(16.0)
}

/// "Your listening experience": the minutes of the year, its song and its artist, each a card
/// that opens Replay.
fn cards(state: &State, year: &Summary, now: chrono::DateTime<chrono::FixedOffset>, width: f32, palette: ComponentPalette) -> Element {
    use chrono::Datelike;
    let across = (width * CARD_SHARE).min(CARD_MOST).round();
    let tall = (across * CARD_RATIO).round();
    let name = state.profile.shown().to_uppercase();
    let plays = t!("mobile-plays-year", count = year.plays, plays = super::super::wrapped::grouped(year.plays as u64), year = now.year()).into_owned();
    // The first play kept, as "member since": Surco has no account to have joined.
    let since = state.collection.plays.iter().map(|play| play.at).min()
        .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
        .map(|at| at.with_timezone(&chrono::Local).format("%m/%y").to_string());
    // BitChord's small capitals stand apart: limn sets no tracking, so a hair space does.
    let caps = |text: &str, size: f32, ink: Color| Text::new(spaced(&text.to_uppercase()), palette).font_size(size).ink(ink).truncate().render()
        .font_family(FontFamily::SansBold);
    // The account's name and the date in a typewriter's bold, as BitChord's: open enough as it is.
    let typed = |text: String, size: f32| Text::new(text, palette).font_size(size).truncate().render().font_family(FontFamily::MonospaceBold);
    let card = |id: &str, eyebrow: &str, big: String, under: String, foot: (String, String), corner: Option<Element>| {
        let top = caps(eyebrow, 11.0, CARD_EYEBROW);
        let figure = Element::column(vec![
            Text::new(big, palette).font_size(30.0).truncate().render().font_family(FontFamily::SansBold),
            caps(&under, 11.5, CARD_QUIET),
        ]).gap(0.0);
        let foot = Element::row(vec![
            Element::column(vec![
                typed(foot.0, 15.0),
                Text::new(foot.1, palette).font_size(13.0).ink(CARD_QUIET).truncate().render(),
            ]).gap(2.0).flex(1.0),
            corner.unwrap_or_else(Element::spacer),
        ]).gap(10.0);
        let body = Element::column(vec![top, Element::spacer().flex(1.0), figure, Element::spacer().size(None, Some(18.0)), foot]).padding(CARD_PAD);
        let face = Element::panel(body).background(CARD_GROUND).radius(CARD_RADIUS).border(THUMB_RIM, 0.5)
            .shadow(CARD_SHADOW, limn::Point::new(0.0, 8.0), 24.0, 0.0).size(Some(across), Some(tall)).clip_children();
        Element::clickable(id, face).dispatch_as("replay").accessible(limn::Access::new(limn::Role::Button).name(eyebrow)).radius(CARD_RADIUS)
    };
    // Flush with the card's right edge, as BitChord's "member since".
    let flush = |line: Element| Element::row(vec![Element::spacer().flex(1.0), line]);
    let joined = since.map(|since| Element::column(vec![
        flush(caps(&t!("mobile-listening-since"), 8.5, CARD_QUIET)),
        flush(typed(since, 15.0)),
    ]).gap(2.0));
    let mut tiles = vec![card("library-card:minutes", &t!("mobile-listening-experience"), super::super::wrapped::grouped(year.minutes),
        t!("mobile-minutes-listened").into_owned(), (name.clone(), plays.clone()), joined)];
    if let Some(top) = year.top_songs.first() {
        tiles.push(card("library-card:song", &t!("mobile-top-song"), top.track.title.clone(),
            t!("common-replay-plays-of", count = top.plays).into_owned(), (name.clone(), top.track.artist.clone()), None));
    }
    if let Some(top) = year.top_artists.first() {
        tiles.push(card("library-card:artist", &t!("mobile-top-artist"), top.track.artist.clone(),
            t!("common-replay-plays-of", count = top.plays).into_owned(), (name, plays), None));
    }
    Element::horizontal_scroll_2d("library-cards", Element::row(tiles).gap(16.0)).drag_to_pan().scrollbar_visibility(SCROLL_INDICATOR)
}

/// `text` with a hair space between its letters, BitChord's open tracking.
fn spaced(text: &str) -> String {
    let mut letters = text.chars();
    let mut out: String = letters.next().into_iter().collect();
    for letter in letters {
        out.push('\u{200A}');
        out.push(letter);
    }
    out
}

/// BitChord's rows under the cards. Surco keeps no local folders or cache to list: its
/// downloads alone.
fn rows(palette: ComponentPalette) -> Element {
    let row = |id: &str, glyph: Icon, label: &str| {
        let copy = Element::column(vec![
            Element::row(vec![
                Text::new(label, palette).font_size(17.0).truncate().render().flex(1.0).centered_y(),
                Element::icon(Icon::ChevronRight).foreground(palette.muted).size(Some(16.0), Some(16.0)).centered_y(),
            ]).gap(8.0).flex(1.0),
            Element::panel(Element::spacer()).background(ROW_RULE).size(None, Some(0.5)),
        ]);
        let glyph = Element::icon(glyph).large_icon(24.0).foreground(palette.text).centered().size(Some(ROW_GLYPH), Some(ROW_HEIGHT));
        Element::clickable(id, Element::row(vec![glyph, copy.flex(1.0)]).gap(8.0)).accessible(limn::Access::new(limn::Role::Button).name(label))
            .size(None, Some(ROW_HEIGHT))
    };
    Element::column(vec![row("downloads", Icon::Custom(DOWNLOAD), &t!("common-downloads"))])
}

/// The lists kept, after a new one, an import and the liked songs, as BitChord's first three.
fn playlists(state: &State, palette: ComponentPalette) -> Element {
    let made = |id: &str, title: &str, about: Option<String>, face: Element| {
        let tile = MediaTile::new(id, title, palette).width(TILE_WIDTH).artwork(face);
        match about { Some(about) => tile.subtitle(about), None => tile }.render()
    };
    let glyph = |glyph: Icon| Element::panel(Element::icon(glyph).large_icon(40.0).foreground(palette.text).centered()).background(SCHEME.raised);
    let liked = Element::panel(Element::icon(Icon::HeartFill).large_icon(40.0).foreground(Color::WHITE).centered())
        .vertical_gradient(LIKED_GROUND.to_vec());
    let mut tiles = vec![
        made("playlist-new", &t!("common-create-playlist"), None, glyph(Icon::Add)),
        made("import-open", &t!("desktop-import-music"), Some(t!("mobile-import-from-link").into_owned()), glyph(Icon::Custom(DOWNLOAD))),
        made("liked", &t!("common-liked-songs"), Some(t!("common-songs", count = state.collection.liked.len()).into_owned()), liked),
    ];
    tiles.extend(state.collection.playlists.iter().enumerate().map(|(index, playlist)| {
        tile(state, format!("playlist:{index}"), &playlist.name, Some(t!("common-songs", count = playlist.tracks.len()).into_owned()), &playlist.cover().cloned(), false, palette)
    }));
    home::see_all_shelf("library-playlists", &t!("desktop-playlists"), "library-all:lists", tiles, palette)
}

/// The artists kept, round, as BitChord's; none, no shelf.
fn artists(state: &State, palette: ComponentPalette) -> Option<Element> {
    let tiles: Vec<Element> = state.collection.artists().map(|(index, artist)| {
        tile(state, format!("library-kept:{index}"), &artist.name, Some(t!("common-artist").into_owned()), &artist.artwork_url, true, palette)
            .dispatch_as(format!("kept:{index}"))
    }).collect();
    (!tiles.is_empty()).then(|| home::see_all_shelf("library-artists", &t!("common-artists"), "library-all:artists", tiles, palette))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Filter, Page};
    use limn::{UiAction, UiTree};

    fn track(id: &str) -> Track {
        Track { id: id.into(), title: format!("Song {id}"), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 }
    }

    #[test]
    fn the_library_tab_is_bitchords_its_cards_rows_and_shelves_and_see_all_lists_them_all() {
        let mut state = State::default();
        state.mobile = true;
        state.update(&UiAction::Clicked { id: "mobile-library".into() });
        assert_eq!(state.page, Page::Collection, "the tab opens the overview");
        let now = chrono::Local::now().timestamp();
        state.collection.plays = (0..3).map(|at| crate::replay::Play { track: track("a"), at: now - 3600 * at, seconds: 180 }).collect();
        let mut tree = UiTree::new(content(&state, 390.0, SCHEME.palette()));
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(390.0, 1400.0));
        let ids: Vec<&str> = frame.regions.iter().map(|region| region.id.as_str()).collect();
        for id in ["library-card:minutes", "library-card:song", "downloads", "playlist-new", "import-open", "liked", "library-all:lists"] {
            assert!(ids.contains(&id), "{id} in {ids:?}");
        }
        assert!(!ids.contains(&"library-all:artists"), "no artists kept, no shelf of them");
        state.update(&UiAction::Clicked { id: "library-all:lists".into() });
        assert_eq!((state.page.clone(), state.filter), (Page::Library, Filter::Lists), "see all is the whole list of that kind");
    }
}
